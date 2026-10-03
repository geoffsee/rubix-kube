# Offline Air-Gap Deployment Runbook

This runbook describes deploying Rubix in air-gapped, isolated, or egress-denied environments
where outbound internet connectivity is unavailable or forbidden by policy.

---

## 1. Air-Gap Architecture & Delivery Model

In standard online mode, Rubix bundles CoreDNS and the Pause sandbox image, and fetches optional
addon images (LocalPath, Portainer, D2K) as needed. In contrast, **offline delivery** bundles all
required and supported optional container images directly inside the distribution package or
offline bundle archive.

### Architectural Invariants in Air-Gap Mode:
1. **Zero Egress Required:** No generation inputs, binaries, or container images are fetched over the network during installation or startup.
2. **Strict Offline Enforcement:** If an image is not bundled in the offline payload, Rubix fails closed with a clear diagnostic rather than timing out attempting network pulls.
3. **Digest & Integrity Validation:** Bundled OCI archives and binaries are verified against cryptographic SHA-256 manifests (`bundle.manifest` and `asset-inventory.json`) before unpack.
4. **Target Cell Specificity:** Packages are pre-compiled and bundled for specific combinations of architecture, C runtime, and delivery mode.

---

## 2. Supported Target Matrix (16 Archive Cells)

Rubix publishes exactly 16 distinct Linux node distribution archive cells, covering every combination
of 4 hardware architectures, 2 C standard libraries, and 2 delivery variants:

| Cell ID | Architecture | C Library | Delivery Variant | Filename Pattern | Bundled Images |
| --- | --- | --- | --- | --- | --- |
| **Cell 01** | `amd64` | `glibc` | Online | `rubix-kube-v{ver}-amd64.tar.gz` | CoreDNS, Pause |
| **Cell 02** | `amd64` | `glibc` | **Offline** | `rubix-kube-v{ver}-amd64-offline.tar.gz` | CoreDNS, Pause, LocalPath, Helper, Portainer, D2K |
| **Cell 03** | `amd64` | `musl` | Online | `rubix-kube-v{ver}-amd64-musl.tar.gz` | CoreDNS, Pause |
| **Cell 04** | `amd64` | `musl` | **Offline** | `rubix-kube-v{ver}-amd64-musl-offline.tar.gz` | CoreDNS, Pause, LocalPath, Helper, Portainer, D2K |
| **Cell 05** | `arm64` | `glibc` | Online | `rubix-kube-v{ver}-arm64.tar.gz` | CoreDNS, Pause |
| **Cell 06** | `arm64` | `glibc` | **Offline** | `rubix-kube-v{ver}-arm64-offline.tar.gz` | CoreDNS, Pause, LocalPath, Helper, Portainer, D2K |
| **Cell 07** | `arm64` | `musl` | Online | `rubix-kube-v{ver}-arm64-musl.tar.gz` | CoreDNS, Pause |
| **Cell 08** | `arm64` | `musl` | **Offline** | `rubix-kube-v{ver}-arm64-musl-offline.tar.gz` | CoreDNS, Pause, LocalPath, Helper, Portainer, D2K |
| **Cell 09** | `armv7` | `glibc` | Online | `rubix-kube-v{ver}-arm.tar.gz` | CoreDNS, Pause |
| **Cell 10** | `armv7` | `glibc` | **Offline** | `rubix-kube-v{ver}-arm-offline.tar.gz` | CoreDNS, Pause, LocalPath, Helper, Portainer |
| **Cell 11** | `armv7` | `musl` | Online | `rubix-kube-v{ver}-arm-musl.tar.gz` | CoreDNS, Pause |
| **Cell 12** | `armv7` | `musl` | **Offline** | `rubix-kube-v{ver}-arm-musl-offline.tar.gz` | CoreDNS, Pause, LocalPath, Helper, Portainer |
| **Cell 13** | `riscv64` | `glibc` | Online | `rubix-kube-v{ver}-riscv64.tar.gz` | CoreDNS, Pause |
| **Cell 14** | `riscv64` | `glibc` | **Offline** | `rubix-kube-v{ver}-riscv64-offline.tar.gz` | CoreDNS, Pause, LocalPath, Helper |
| **Cell 15** | `riscv64` | `musl` | Online | `rubix-kube-v{ver}-riscv64-musl.tar.gz` | CoreDNS, Pause |
| **Cell 16** | `riscv64` | `musl` | **Offline** | `rubix-kube-v{ver}-riscv64-musl-offline.tar.gz` | CoreDNS, Pause, LocalPath, Helper |

> [!NOTE]
> Architecture Limitations: Portainer Agent is not published for `riscv64`. D2K is built exclusively for 64-bit platforms (`amd64`, `arm64`) and is intentionally disabled on `armv7` and `riscv64`.

---

## 3. Bundled Image Reference & Manifest Verification

Offline archives contain pre-exported OCI tarballs for the following authoritative versions:

| Image Identity | Version / Tag | Architecture Support | Function |
| --- | --- | --- | --- |
| `docker.io/coredns/coredns` | `1.14.4` | All 4 architectures | Internal cluster DNS service |
| `docker.io/portainer/pause` | `latest` | All 4 architectures | CRI sandbox container |
| `docker.io/rancher/local-path-provisioner` | `v0.0.36` | All 4 architectures | Persistent volume local-path provisioner |
| `docker.io/library/busybox` | `latest` (pinned) | All 4 architectures | LocalPath volume setup & teardown helper |
| `docker.io/portainer/agent` | `lts` | `amd64`, `arm64`, `armv7` | Portainer Edge Agent (optional) |
| `docker.io/portainer/d2k` | `1.2.3` | `amd64`, `arm64` | Docker-to-Kubernetes API translator (optional) |

### Verifying Offline Bundles Prior to Transfer
On a staging machine with network access, verify checksums:
```sh
# Verify sha256 checksums of the downloaded offline bundle
sha256sum -c rubix-kube-v1.35.7-amd64-offline.tar.gz.sha256

# Verify the internal bundle manifest
tar -ztvf rubix-kube-v1.35.7-amd64-offline.tar.gz | grep bundle.manifest
```

---

## 4. Bare-Metal & VM Host Air-Gap Installation

### Step 1: Transfer Bundle to Air-Gapped Host
Copy the archive file and `rubixctl` binary to the target host via secure media (e.g. USB drive, bastion jump host, or private repository).

### Step 2: Run Offline Installation via `rubixctl`
```sh
sudo rubixctl install \
  --bundle /var/tmp/rubix-kube-v1.35.7-amd64-offline.tar.gz \
  --init systemd
```

`rubixctl` executes:
1. **Archive Integrity Inspection:** Validates `bundle.manifest` against embedded files.
2. **Atomic Binary Extraction:** Installs `kubesolo` and `rubixctl` with `0755` permissions.
3. **Payload Materialization:** Stages bundled images under `/var/lib/kubesolo/containerd/images/` with `0644` permissions.
4. **Pre-loading Images:** Automatically imports the bundled tar archives into containerd's `k8s.io` namespace.
5. **Service Activation:** Registers and starts `kubesolo.service`.

### Step 3: Verification
Inspect containerd images to ensure all required images are present in the `k8s.io` namespace:
```sh
sudo ctr -n k8s.io images list
```

---

## 5. Container-Mode Air-Gap Installation (Docker Engine)

When deploying Rubix as a container on an air-gapped Docker Engine (Linux, macOS, or WSL2):

```sh
# Deploy container cluster using offline bundle
rubixctl container create \
  --name airgap-cluster \
  --bundle /var/tmp/rubix-kube-v1.35.7-amd64-offline.tar.gz \
  --port 8080:80
```

### Safety and Validation Enforcements in Container Mode:
- **`--pull=never` Enforcement:** Docker container creation is strictly enforced with `--pull=never`. If the image cannot be loaded from the archive, deployment fails immediately.
- **Single Archive Tag Constraint:** The offline bundle must expose exactly one repo tag for the Rubix container image. Archives containing ambiguous or conflicting tags are rejected before any Docker resources are allocated.
- **Config Platform & Layer DiffID Verification:** Rubix inspects the image manifest and verifies layer DiffIDs before streaming bytes to Docker Engine, ensuring image integrity even on hosts with untrusted storage.

---

## 6. Private Mirror Registry Configuration (Optional)

If your air-gapped network operates an internal mirror registry (e.g. Harbor, Nexus, Quay), configure containerd to route image pulls to the local mirror without altering Kubernetes deployment manifests:

Edit `/var/lib/kubesolo/containerd/config.toml` or create `/etc/kubesolo/hosts.toml`:
```toml
[host."http://mirror.internal.local:5000"]
  capabilities = ["pull", "resolve"]
  skip_verify = false
  ca = "/etc/kubesolo/certs/mirror-ca.crt"
```

Restart Rubix to apply containerd registry configuration:
```sh
sudo systemctl restart kubesolo
```
