# Fresh Installation and Container Lifecycle Runbook

This runbook guides operators through deploying fresh Rubix single-node Kubernetes clusters
across supported platforms, including universal host installation, minimal manual installation,
and containerized deployment via Docker Engine.

---

## 1. Prerequisites & Preflight Inspection

Before installing Rubix on a Linux host, run preflight checks using `rubixctl`:

```sh
# Execute host preflight checks
rubixctl check
```

### Preflight Verification Scope
Preflight evaluation inspects:
- **Operating System & Architecture:** Confirms Linux kernel, architecture (`amd64`, `arm64`, `armv7`, or `riscv64`), and C library (`glibc` or `musl`).
- **Required Kernel Capabilities:** Validates cgroups (v1 or v2 unified hierarchy), overlay filesystem support (`overlay` or `fuse-overlayfs`), and packet filtering (`iptables` or `nftables`).
- **Port Availability:** Tests required control plane and service ports:
  - `6443/tcp`: Kubernetes API Server
  - `2379/tcp`: Kine etcd-compatible loopback listener
  - `10443/tcp`: NodeSetter admission webhook
  - `9105/tcp`: Operational metrics HTTP endpoint (when enabled)
  - `2376/tcp`: D2K Docker API endpoint (when enabled)
- **Filesystem Permissions:** Validates write access to `/var/lib/kubesolo` (default data directory) and `/etc/kubesolo` (configuration directory).

### Alpine Linux / OpenRC Prerequisites
On Alpine Linux, missing network tools and cgroups can be automatically configured with:
```sh
rubixctl check --install-prereqs
```
This flag authorizes only explicit Alpine package additions (`iproute2`, `iptables`) and enabling the `cgroups` OpenRC service. On non-Alpine hosts, `--install-prereqs` is a safe no-op.

---

## 2. Universal Host Installation

Universal installation automates artifact downloading, binary placement, configuration initialization,
and service registration under the host's native init system.

### Running Universal Installation
```sh
# Install default version under systemd
sudo rubixctl install --version 1.35.7

# Install on an OpenRC system with custom data path
sudo rubixctl install --version 1.35.7 --init openrc --path /mnt/fast-storage/kubesolo

# Install specifying custom network interface IP
sudo rubixctl install --node-ip 192.168.1.50
```

### Supported Init Systems & Service Unit Placement

`rubixctl` generates and installs native service definitions based on the detected init system:

| Init System | Unit File Path | Lifecycle Control Commands |
| --- | --- | --- |
| **systemd** | `/etc/systemd/system/kubesolo.service` | `systemctl daemon-reload && systemctl enable --now kubesolo` |
| **OpenRC** | `/etc/init.d/kubesolo` | `rc-update add kubesolo default && rc-service kubesolo start` |
| **SysVinit** | `/etc/init.d/kubesolo` | `update-rc.d kubesolo defaults && /etc/init.d/kubesolo start` |
| **Upstart** | `/etc/init/kubesolo.conf` | `initctl reload-configuration && start kubesolo` |
| **runit** | `/etc/sv/kubesolo/run` | `ln -s /etc/sv/kubesolo /var/service/` |
| **s6** | `/etc/s6/services/kubesolo/run` | `s6-svc -u /etc/s6/services/kubesolo` |

### Installed Layout
Universal install stages the following filesystem structure:
- `/usr/local/bin/kubesolo`: Supervised node daemon (`rubix-kube`).
- `/usr/local/bin/rubixctl`: Management CLI.
- `/etc/kubesolo/config.yaml`: Canonical cluster configuration (`0600` permissions).
- `/var/lib/kubesolo/`: Base directory containing PKI, datastore, runtime, and volume storage.

---

## 3. Minimal Host Installation

In resource-constrained, embedded, or custom environments where system package managers and init
system daemons are unavailable or restricted, Rubix can be deployed manually.

### Step 1: Download and Extract Candidate Archive
Download the matching architecture archive cell (e.g. `rubix-kube-v1.35.7-linux-arm64.tar.gz`):
```sh
tar -xzf rubix-kube-v1.35.7-linux-arm64.tar.gz -C /usr/local/bin/
chmod 0755 /usr/local/bin/kubesolo /usr/local/bin/rubixctl
```

### Step 2: Create Canonical Configuration File
Create `/etc/kubesolo/config.yaml` with mode `0600`:
```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
path: /var/lib/kubesolo
logging:
  debug: false
network:
  nodeIP: ""         # Auto-detect primary host IP
  mtu: 0             # Auto-detect interface MTU (minimum 1200, default 1500)
  disableIPv6: false
  loadBalancer:
    enabled: true
runtime:
  endpoint: ""       # Empty string selects managed containerd
kubernetes:
  nodeName: ""       # Empty string normalizes host hostname
storage:
  localPath:
    enabled: true
metrics:
  enabled: true
  bindAddress: "127.0.0.1:9105"
```

### Step 3: Launch in Desired Execution Mode

#### Mode A: Interactive Foreground Execution
```sh
sudo /usr/local/bin/kubesolo --config /etc/kubesolo/config.yaml
```
In foreground mode, structured JSONL logs stream to stdout/stderr. Sending `SIGINT` or `SIGTERM`
initiates cooperative graceful shutdown with a 30-second bounded drain.

#### Mode B: Daemon Execution
```sh
sudo nohup /usr/local/bin/kubesolo --config /etc/kubesolo/config.yaml > /var/log/kubesolo.log 2>&1 &
echo $! | sudo tee /var/run/kubesolo.pid
```

---

## 4. Named Container Lifecycle (Docker Engine Mode)

Rubix supports running single-node clusters inside Docker containers on Linux, macOS (Docker Desktop),
and Windows (WSL2 with Docker Engine). Multiple named clusters can coexist independently on a single host.

### Container Architecture & Isolation
Each container cluster is provisioned with:
- **Dedicated Bridge Network:** `rubix-net-<name>` with MTU matched to the host Docker daemon.
- **Dedicated Persistent Volume:** `rubix-data-<name>` mounted at `/var/lib/kubesolo`.
- **Dedicated Container Instance:** `rubix-<name>`.
- **Isolated Loopback API Port:** Assigned dynamically or mapped explicitly.

### Creating Named Container Clusters
```sh
# Create a development cluster with default port allocation
rubixctl container create --name dev-01

# Create a cluster with explicit workload port mappings
rubixctl container create --name web-cluster \
  --port 8080:80 \
  --port 8443:443 \
  --port 127.0.0.1:9090:9090

# Create a cluster specifying container image tag
rubixctl container create --name edge-sim --image rubix-node:v1.35.7
```

### Port Mapping Security Rules
- **Host Binding Defaults:** Mappings without an explicit host IP (e.g. `--port 8080:80`) bind exclusively to loopback (`127.0.0.1:8080:80`) to avoid unintentional LAN exposure.
- **Reserved Port Protection:** Operators cannot bind host ports to internal control plane ports `6443` (API Server) or `2376` (D2K) directly; these are routed via authenticated endpoints.

### Managing Named Container Lifecycles
```sh
# Inspect container cluster status and exposed ports
rubixctl container status --name dev-01

# Restart container cluster (preserves persistent volume)
rubixctl container restart --name dev-01

# Stop running container cluster
rubixctl container stop --name dev-01

# Remove container instance (volume data is retained by default)
rubixctl container remove --name dev-01

# Remove container and purge persistent data
rubixctl container remove --name dev-01 --purge
```

---

## 5. Client Access Setup (`kubeconfig`)

Once the cluster is running (either host or container mode), export client credentials:

```sh
# Fetch admin kubeconfig and merge into current user's default config
rubixctl kubeconfig fetch --merge

# Export to a standalone kubeconfig file
rubixctl kubeconfig fetch --output ~/.kube/rubix-dev-01.kubeconfig

# Verify cluster connectivity
kubectl --kubeconfig ~/.kube/rubix-dev-01.kubeconfig get nodes
kubectl --kubeconfig ~/.kube/rubix-dev-01.kubeconfig get pods -A
```

Both YAML and JSON formatted kubeconfig structures are fully accommodated by `rubixctl` and
underlying client libraries.
