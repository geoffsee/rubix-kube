# Third-Party Attribution and License Inventory

This document records the third-party software components, retained executables,
container images, and library dependencies incorporated into or supervised by Rubix Kube.
It fulfills the attribution requirements of [E30.03](https://github.com/geoffsee/rubix-kube/issues/126)
and [Roadmap #263](https://github.com/geoffsee/rubix-kube/issues/263) Completion Criteria 10 and 11.

## Baseline Heritage

Rubix Kube originates from the Portainer KubeSolo project baseline at commit
[`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`](https://github.com/portainer/kubesolo/commit/2ef1c4787989f11f868f81bb84ae2afd4a49a81d).
Portainer KubeSolo is licensed under the Apache License, Version 2.0 (Apache-2.0).
Copyright © Portainer.io.

---

## Retained Upstream Components

Rubix Kube supervises retained official upstream binaries and container images rather than
reimplementing Kubernetes, container runtimes, or datastore engines in native Rust.
The following 17 retained components constitute the supervised product boundary defined in
the [Component Boundary ADR](../../experiments/component-boundary/ADR.md) and
the [Acceptance Matrix](acceptance-matrix.md).

| Component | Upstream Reference | License (SPDX) | Copyright Holders / Authors | Upstream Repository |
| --- | --- | --- | --- | --- |
| `kube-apiserver` | Official v1.35.7 | Apache-2.0 | The Kubernetes Authors | https://github.com/kubernetes/kubernetes |
| `kube-controller-manager` | Official v1.35.7 | Apache-2.0 | The Kubernetes Authors | https://github.com/kubernetes/kubernetes |
| `kubelet` | Official v1.35.7 | Apache-2.0 | The Kubernetes Authors | https://github.com/kubernetes/kubernetes |
| `kube-proxy` | Official v1.35.7 | Apache-2.0 | The Kubernetes Authors | https://github.com/kubernetes/kubernetes |
| `kine` | v0.16.3 | Apache-2.0 | Rancher Labs, Inc. / SUSE | https://github.com/k3s-io/kine |
| `sqlite` (embedded) | Embedded in Kine | Blessing / Public Domain | D. Richard Hipp / SQLite Authors | https://sqlite.org |
| `containerd` | Official v2.2.5 | Apache-2.0 | The containerd Authors | https://github.com/containerd/containerd |
| `containerd-shim-runc-v2` | v2.2.5 | Apache-2.0 | The containerd Authors | https://github.com/containerd/containerd |
| `crun` | 1.26 | GPL-2.0-or-later / LGPL-2.1-or-later | Red Hat, Inc. and contributors | https://github.com/containers/crun |
| `cni-plugins` (bridge, host-local, portmap, loopback) | v1.9.0 | Apache-2.0 | The CNI Authors | https://github.com/containernetworking/plugins |
| `containerd-fuse-overlayfs-grpc` | v2.1.7 | Apache-2.0 | The containerd Authors | https://github.com/containerd/fuse-overlayfs-snapshotter |
| `coredns` | docker.io/coredns/coredns:1.14.4 | Apache-2.0 | The CoreDNS Authors | https://github.com/coredns/coredns |
| `pause` | docker.io/portainer/pause:latest | Apache-2.0 | Portainer.io / Kubernetes Authors | https://github.com/kubernetes/kubernetes |
| `local-path-provisioner` | docker.io/rancher/local-path-provisioner:v0.0.36 | Apache-2.0 | Rancher Labs, Inc. | https://github.com/rancher/local-path-provisioner |
| `busybox` (helper pod) | busybox (pinned digest) | GPL-2.0-only | Erik Andersen, Rob Landley, Denys Vlasenko, et al. | https://busybox.net |
| `portainer-agent` | docker.io/portainer/agent:lts | Zlib | Portainer.io | https://github.com/portainer/agent |
| `d2k` | docker.io/portainer/d2k:1.2.3 | Apache-2.0 | Portainer.io | https://github.com/portainer/d2k |

### Detailed Retained Component Descriptions

#### 1. Kubernetes Control Plane & Node Executables (`kube-apiserver`, `kube-controller-manager`, `kubelet`, `kube-proxy`)
- **Version**: v1.35.7
- **License**: Apache-2.0
- **Copyright**: © The Kubernetes Authors
- **Role**: Official, unmodified upstream Kubernetes executables supervised by Rubix.
  Configuration, PKI, credentials, and lifecycle are managed by Rust crates (`rubix-apiserver`, `rubix-controller`, `rubix-kubelet`, `rubix-proxy`).

#### 2. Kine Datastore Adapter & SQLite Engine
- **Version**: Kine v0.16.3; SQLite embedded engine
- **License**: Apache-2.0 (Kine); Public Domain / SQLite Blessing (SQLite)
- **Copyright**: © Rancher Labs, Inc. / SUSE; D. Richard Hipp
- **Role**: Provides etcd v3 API emulation backed by SQLite. Rubix supervises Kine over dedicated loopback mTLS with isolated CA credentials (`rubix-datastore`, `rubix-pki`).

#### 3. Containerd & Runtime Shims
- **Version**: containerd v2.2.5, `containerd-shim-runc-v2` v2.2.5
- **License**: Apache-2.0
- **Copyright**: © The containerd Authors
- **Role**: Managed container runtime providing CRI v1 socket. Supervised and configured by `rubix-containerd`.

#### 4. OCI Runtime (`crun`)
- **Version**: 1.26
- **License**: GPL-2.0-or-later / LGPL-2.1-or-later
- **Copyright**: © Red Hat, Inc. and contributors
- **Role**: OCI container runtime invoked by containerd shim. Target architecture builds provided across amd64, arm64, ARMv7, and riscv64.

#### 5. CNI Plugins
- **Version**: v1.9.0
- **License**: Apache-2.0
- **Copyright**: © The CNI Authors
- **Role**: Standard network plugins (`bridge`, `host-local`, `portmap`, `loopback`) executed for pod network namespace creation. Configured idempotently by `rubix-network`.

#### 6. FUSE-OverlayFS Snapshotter
- **Version**: v2.1.7 (`da57796c7d0a2b608abf173651cf148706e33337`)
- **License**: Apache-2.0
- **Copyright**: © The containerd Authors
- **Role**: Supervised standalone proxy plugin executable providing user-space snapshotting for rootless and constrained container environments.

#### 7. CoreDNS Workload Image
- **Version**: `1.14.4` (`docker.io/coredns/coredns:1.14.4`)
- **License**: Apache-2.0
- **Copyright**: © The CoreDNS Authors
- **Role**: Cluster internal DNS server. Managed via `rubix-dns` Corefile generation and deployment reconciliation.

#### 8. Pause CRI Sandbox Image
- **Version**: `docker.io/portainer/pause:latest`
- **License**: Apache-2.0
- **Copyright**: © Portainer.io / The Kubernetes Authors
- **Role**: Pod infrastructure container holding network and IPC namespaces.

#### 9. Local-Path Provisioner & Helper Pod
- **Version**: Provisioner `docker.io/rancher/local-path-provisioner:v0.0.36`; Helper `busybox`
- **License**: Apache-2.0 (provisioner); GPL-2.0-only (busybox)
- **Copyright**: © Rancher Labs, Inc.; Erik Andersen et al.
- **Role**: Single-node persistent volume dynamic provisioner and volume initialization helper. Supervised and configured by `rubix-storage`.

#### 10. Portainer Edge Agent (Optional Addon)
- **Version**: `docker.io/portainer/agent:lts`
- **License**: Zlib
- **Copyright**: © Portainer.io
- **Role**: Optional management agent bootstrapped by `rubix-portainer` while preserving pre-existing objects.

#### 11. D2K Docker-to-Kubernetes Proxy (Optional Addon)
- **Version**: `docker.io/portainer/d2k:1.2.3`
- **License**: Apache-2.0
- **Copyright**: © Portainer.io
- **Role**: Translates Docker Engine API HTTP calls to Kubernetes API requests. Authenticated via dedicated TLS client certificates.

---

## Workspace Rust Dependencies

All third-party Rust crates used in the Rubix Kube workspace are locked in `Cargo.lock`
and vetted against the license policy defined in `deny.toml`.
Permitted license identifiers include:
- `Apache-2.0`
- `Apache-2.0 WITH LLVM-exception`
- `BSD-2-Clause`
- `BSD-3-Clause`
- `ISC`
- `MIT`
- `MPL-2.0`
- `Unicode-3.0`
- `Zlib`

### Key Rust Libraries
- **Asynchronous Runtime**: `tokio` (MIT), `futures-core` (MIT/Apache-2.0)
- **Serialization / Data Formats**: `serde` (MIT/Apache-2.0), `serde_json` (MIT/Apache-2.0), `toml` (MIT/Apache-2.0), `saphyr-parser` (local patched, MIT/Apache-2.0)
- **Cryptography & PKI**: `rcgen` (MIT/Apache-2.0), `aws-lc-rs` (Apache-2.0/ISC), `sha2` (MIT/Apache-2.0), `x509-parser` (MIT/Apache-2.0)
- **Kubernetes Client Types**: `k8s-openapi` (Apache-2.0)
- **System & OS Interfaces**: `rustix` (Apache-2.0 WITH LLVM-exception / Apache-2.0 / MIT), `libc` (MIT/Apache-2.0)
- **Archives & Compression**: `tar` (MIT/Apache-2.0), `flate2` (MIT/Apache-2.0), `zstd` (MIT), `zip` (MIT)
