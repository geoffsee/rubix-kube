# Rubix Operator Documentation & Runbooks

Welcome to the Rubix operator documentation. Rubix is a single-node Kubernetes distribution
delivering high performance, single-binary distribution management, and compatibility with
the KubeSolo architecture (`kubesolo.io/v1alpha1`).

This documentation suite provides operational procedures, architectural boundaries, deployment
runbooks, and disaster recovery playbooks for cluster operators.

## Runbook Map

The operational documentation is organized into focused runbooks under `docs/operator/`:

| Runbook | Scope and Topics Covered | Target Environments |
| --- | --- | --- |
| [Fresh Installation & Container Lifecycle](fresh-installs.md) | Universal host install (`rubixctl install`), minimal host install (manual binary/config), service unit management (systemd, OpenRC, SysVinit, Upstart, runit, s6), and named Docker container lifecycle (`rubixctl container`). | Bare-metal Linux, Virtual Machines, Docker Engine (Linux, macOS, WSL2). |
| [Offline Air-Gap Deployment](air-gap-deployment.md) | Air-gapped deployments with egress denied, 16 target archive cells, verified offline bundles (`bundle.manifest`), offline image archives (crane/OCI tars), and local containerd image loading. | Air-gapped datacenters, isolated edge devices, egress-denied hosts. |
| [External Container Runtime Integration](external-container-runtime.md) | Connecting to host-managed containerd and CRI-O runtimes, CRI socket URI configuration (`unix://`), cgroup driver matching (`systemd` vs `cgroupfs`), cgroup v1/v2 detection, and runtime ownership boundaries. | Pre-existing host containerd (v2.0.2 / v1.7.24) or CRI-O (v1.32.0 / v1.30.0). |
| [Networking & Local Storage Provisioning](networking-and-storage.md) | CNI plugin configuration (`bridge`, `host-local`, `portmap`, `loopback`), bridge network `10-bridge.conflist`, pod egress masquerade (`kubesolo-masq` nftables/iptables), and Rancher LocalPath storage provisioner (`WaitForFirstConsumer`, `Retain` / `Delete`). | Linux host networking, dual-stack/IPv4-only sysctls, persistent local storage. |
| [Operational Metrics & CPU Management](metrics-and-cpu-management.md) | HTTP metrics routes (`/metrics`, `/healthz`, `/livez`, `/readyz`), Prometheus text and OpenMetrics content negotiation, metric series reference, Kubelet CPU manager policies (`none` vs `static`), policy options, and state checkpoint invalidation. | Control plane observability, low-latency / real-time workloads. |
| [Migration & Operator Recovery Runbook](migration-and-recovery.md) | Step-by-step Go-to-Rust migration (`v1.1.8`..`v1.3.3`), CLI flag to `kubesolo.io/v1alpha1` YAML conversion, Kine SQLite snapshot/restore and WAL handling, dedicated loopback mTLS PKI preservation, recovery procedures, contractual downtime bounds, and platform limitations. | Existing KubeSolo cluster upgrades, disaster recovery, crash recovery. |

---

## Architectural Principles & Ownership Boundaries

Operators managing Rubix clusters must observe the following core architectural rules:

1. **Supervised Upstream Component Boundary:**
   Rubix supervises official upstream Kubernetes binaries (`kube-apiserver`, `kube-controller-manager`,
   `kubelet`, `kube-proxy`), upstream `kine` (v0.16.3), and upstream `containerd` (v2.2.5). Rubix owns
   configuration rendering, process supervision, identity and PKI generation, lifecycle orchestration,
   preflight checks, and admission mutation (`NodeSetter`). Rubix does **not** reimplement Kubernetes
   or container runtime internals in Rust.
2. **Strict Resource Ownership:**
   Rubix strictly owns files located under its configured base path (default `/var/lib/kubesolo`),
   configuration in `/etc/kubesolo/config.yaml`, service unit definitions, and its dedicated firewall
   table (`kubesolo-masq`). External runtimes (host containerd, CRI-O), foreign processes, unrelated Docker
   containers, and non-Rubix network rules are **never** mutated, stopped, or flushed.
3. **Dedicated Loopback mTLS Datastore Boundary:**
   Communication between `kube-apiserver` and `kine` takes place over loopback TCP using dedicated
   mutual TLS (mTLS). The datastore CA and client certificates are isolated from general cluster client
   PKI. Incompatible or unauthenticated connections are rejected fail-closed.
4. **State Retention Across Lifecycle Operations:**
   Standard reset and upgrade operations retain PKI keys, configuration backups, persistent volume data
   under `local-path-storage`, and offline container archives. Destruction of persistent data requires
   an explicit purge command (`rubixctl reset --purge`).

---

## Management CLI Quick Reference

The primary management binary is `rubixctl` (management operations) and `rubix-kube` (node supervisor):

```sh
# Preflight checks and host readiness
rubixctl check
rubixctl check --install-prereqs    # Authorizes missing Alpine package/cgroup installation

# Universal host installation
rubixctl install --version 1.35.7 --init systemd

# Offline bundle installation
rubixctl install --bundle /path/to/rubix-kube-v1.35.7-amd64-offline.tar.gz

# Container mode cluster management
rubixctl container create --name dev-cluster --port 8080:80
rubixctl container status --name dev-cluster
rubixctl container restart --name dev-cluster
rubixctl container stop --name dev-cluster
rubixctl container remove --name dev-cluster

# Client credentials and context export
rubixctl kubeconfig fetch --output ~/.kube/config --merge
rubixctl d2k fetch --output ~/.docker/contexts

# Node configuration management
rubixctl config validate /etc/kubesolo/config.yaml
rubixctl config get
rubixctl config set network.loadBalancer.enabled false

# Upgrades and recovery
rubixctl upgrade --target-version 1.35.7
rubixctl upgrade --recover          # Automated recovery from interrupted/failed upgrade

# Scoped cluster reset
rubixctl reset                      # Cleans runtime state; retains PKI, config, and PVs
rubixctl reset --purge              # Discards all cluster state including volumes
```
