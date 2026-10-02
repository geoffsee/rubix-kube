# rubix-dns

CoreDNS deployment and reconciliation for KubeSolo clusters.

## Overview

`rubix-dns` provides cluster DNS and upstream resolver configuration by deploying and reconciling CoreDNS within `kube-system`. It generates and manages:
- **ServiceAccount**: `coredns` in `kube-system`
- **ClusterRole**: `system:coredns` with list/watch permissions for core Kubernetes discovery resources
- **ClusterRoleBinding**: `system:coredns` binding the service account to the cluster role
- **ConfigMap**: `coredns` containing the `Corefile`
- **Service**: `kube-dns` on cluster IP `10.43.0.10` exposing DNS on port 53 (UDP and TCP)
- **Deployment**: `coredns` running `docker.io/coredns/coredns:1.14.4` with health and readiness probes

## Corefile Generation

The Corefile is generated according to the cluster's network and execution configuration:
- **Container Mode (`container_mode = true`)**: Uses public upstream fallback resolvers (`1.1.1.1 8.8.8.8`) because containerized environments typically feature empty or loopback `/etc/resolv.conf`.
- **Host Mode (`container_mode = false`)**: Forwards upstream queries to the host's `/etc/resolv.conf`.
- **IPv4-Only Mode (`disable_ipv6 = true`)**: Omits `ip6.arpa` reverse-zone forwarding from the kubernetes plugin block (`in-addr.arpa` only), avoiding upstream reverse lookups for non-existent IPv6 subnets.
- **Dual-Stack Mode (`disable_ipv6 = false`)**: Includes both `in-addr.arpa` and `ip6.arpa` reverse-zone forwarding.
- **Custom Upstream Resolvers**: When explicit upstream resolvers are supplied, queries forward directly to those specified servers.
