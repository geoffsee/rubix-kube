# rubix-proxy

`rubix-proxy` manages kube-proxy configuration generation, backend selection (`iptables` vs `nftables`), conntrack tuning adaptation, and supervised service lifecycle in Rubix.

## Features

- **v1alpha1 KubeProxyConfiguration Generation**: Full parity with Kubernetes `kubeproxy.config.k8s.io/v1alpha1`.
- **Automatic Backend Selection**: Dynamically detects kernel and userspace capabilities, seamlessly selecting `iptables` or `nftables` (Kubernetes 1.31+).
- **Container Mode Conntrack Zeroing**: In container mode, all six conntrack settings are set to zero (`maxPerCore = 0`, `min = 0`, `tcpEstablishedTimeout = 0s`, `tcpCloseWaitTimeout = 0s`, `udpTimeout = 0s`, `udpStreamTimeout = 0s`), enabling flawless operation on read-only `/proc/sys` filesystems.
- **Host Mode Upstream Defaults**: Preserves upstream Kubernetes host defaults when running on bare metal or virtual machine hosts.
- **Supervised Lifecycle & Health**: Fully integrated with `rubix-supervisor`, checking health and verifying SNAT pod egress masquerade readiness before signaling component ready.
- **Actionable Diagnostics**: Actionable operator guidance on missing backends, read-only `/proc/sys`, and credential failures.
