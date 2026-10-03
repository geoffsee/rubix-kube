# Networking & Local Storage Provisioning Runbook

This runbook covers configuring host and pod networking, CNI plugins, packet filtering / egress
masquerade rules, and persistent local storage provisioner behavior in Rubix.

---

## 1. Network Topology & Addressing Architecture

Rubix establishes an isolated, single-node Kubernetes network fabric with fixed CIDR blocks:

| Network Surface | Subnet / Address | Purpose |
| --- | --- | --- |
| **Pod Network (Pod CIDR)** | `10.42.0.0/16` | Assigned to Kubernetes pods on the node bridge. |
| **Service Network (ClusterIP)** | `10.43.0.0/16` | Virtual IP addresses allocated to Kubernetes Services. |
| **Kubernetes API ClusterIP** | `10.43.0.1` | In-cluster service endpoint for the Kubernetes API Server. |
| **CoreDNS ClusterIP** | `10.43.0.10` | Internal cluster DNS resolver IP across namespaces. |
| **Host Node IP** | Auto-detected / Configured | Host primary network interface IP address. |

### MTU (Maximum Transmission Unit) Resolution
- **Auto-detection:** Rubix queries the MTU of the default gateway interface.
- **Constraints:** Minimum valid MTU is `1200` (e.g. for WireGuard/VPN encapsulation); maximum is `9000` (jumbo frames). Default is `1500`.
- **Manual Override:** Specify `network.mtu` in `/etc/kubesolo/config.yaml`:
  ```yaml
  network:
    mtu: 1420
  ```

---

## 2. CNI Plugin Configuration & Bridge Setup

Rubix uses the standard CNI bridge plugin stack to provide pod network interfaces and IPAM.

### Required CNI Plugins
The following standard plugins must be present in the CNI bin directory (managed `/var/lib/kubesolo/containerd/cni/plugins` or host `/opt/cni/bin`):
- `bridge`: Configures the virtual bridge (`cbr0` or `rubix-br0`) and veth pairs.
- `host-local`: Manages local IPv4 address allocation out of `10.42.0.0/16`.
- `portmap`: Implements hostPort port mappings via iptables/nftables.
- `loopback`: Configures the container `lo` interface.

### Owned CNI Conflist (`10-bridge.conflist`)
Rubix generates and manages `/etc/cni/net.d/10-bridge.conflist` (or instance-scoped path in managed mode):
```json
{
  "cniVersion": "0.3.1",
  "name": "rubix-bridge",
  "plugins": [
    {
      "type": "bridge",
      "bridge": "cbr0",
      "isGateway": true,
      "isDefaultGateway": true,
      "ipMasq": false,
      "mtu": 1500,
      "ipam": {
        "type": "host-local",
        "subnet": "10.42.0.0/16",
        "routes": [
          { "dst": "0.0.0.0/0" }
        ]
      }
    },
    {
      "type": "portmap",
      "capabilities": { "portMappings": true }
    }
  ]
}
```

> [!NOTE]
> In external runtime mode, Rubix writes only its owned `10-bridge.conflist`. It will never remove or
> overwrite pre-existing third-party CNI conflist files.

---

## 3. Pod Egress & Firewall Masquerade Rules

To allow pods on `10.42.0.0/16` to reach external networks, Rubix manages outbound Network Address
Translation (SNAT / Masquerade).

### Egress Traffic Evaluation Rules:
1. **Pod-to-External Traffic:** Source in `10.42.0.0/16`, destination outside `10.42.0.0/16`. **Action: Masquerade (SNAT).**
2. **Pod-to-Pod Traffic:** Source in `10.42.0.0/16`, destination inside `10.42.0.0/16`. **Action: Direct routing (No Masquerade).**
3. **Host Traffic:** Source outside `10.42.0.0/16`. **Action: Unmatched / Ignored.**

### Firewall Coexistence Policy (Zero Blanket Flushes)
Rubix is designed to safely coexist on hosts running Docker Engine, `ufw`, `firewalld`, or custom nftables rules:
- **Backend Detection:** Rubix probes for `nft` (Linux nftables) first, falling back to `iptables`.
- **nftables Mode:** Rules are placed exclusively in a dedicated table named `kubesolo-masq`.
- **iptables Mode:** Rules are tagged with the specific comment:
  `/* kubesolo: pod masquerade */`
- **Strict Ownership:** Rubix **never** executes blanket NAT flushes (`iptables -F` or `nft flush ruleset`). When resetting or stopping, only rules matching the table name or comment are cleaned.
- **Startup Ordering:** Egress rules are verified and established *before* Kubelet recovers persisted pods, ensuring existing workloads never experience network blackholes upon node restart.

### IPv6 Handling
On hosts where IPv6 is not configured or causes route leaks, set `network.disableIPv6: true`.
Rubix configures `/proc/sys/net/ipv6/conf/all/disable_ipv6` and strips IPv6 nameserver entries from
generated resolver configurations.

---

## 4. LocalPath Storage Provisioner Behavior

Rubix includes a built-in Kubernetes storage controller deploying the Rancher LocalPath provisioner
(`rancher.io/local-path`) to satisfy `PersistentVolumeClaims` (PVCs) using host disk storage.

### Core Storage Parameters
- **Default StorageClass:** `local-path` (set as the default cluster StorageClass).
- **Volume Binding Mode:** `WaitForFirstConsumer` (ensures volumes are only provisioned once a pod requesting the claim is scheduled).
- **Reclaim Policies Supported:**
  - `Delete` (Default): Directory on the host is deleted when the PVC is deleted.
  - `Retain`: Directory on the host is retained when the PVC is deleted.

### Host Storage Locations
By default, persistent data is stored under:
```
/var/lib/kubesolo/local-path-storage/
```
Each volume receives a dedicated directory named:
```
pvc-<pvc-uuid>_<namespace>_<pvc-name>/
```

### Configuring a Custom Shared Path
To place persistent volumes on a dedicated partition, SSD, or external mount:
```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
storage:
  localPath:
    enabled: true
    sharedPath: "/mnt/fast-nvme/k8s-volumes"
```

### Path Traversal Defense & Teardown Safety
When a PVC with `ReclaimPolicy: Delete` is removed, the provisioner executes a teardown routine using the helper pod. Rubix enforces strict path validation:
- Symlinks inside volume paths are never followed blindly.
- Volume paths must reside strictly within the designated storage root. Any attempt to specify paths containing relative traversals (`../`) or mount escapes is aborted with an error.

### Example PVC Manifest
```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: database-pvc
  namespace: default
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: local-path
  resources:
    requests:
      storage: 10Gi
```
Deploying a pod using `database-pvc` automatically provisions the host directory and mounts it inside the container.
