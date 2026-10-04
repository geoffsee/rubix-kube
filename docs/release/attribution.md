# Draft Upstream License Attribution and Third-Party Notices

**Rubix Kubernetes Distribution Version**: `0.1.0`  
**Primary Distribution License**: `ISC`

UNQUALIFIED_FIXTURE_ONLY. This draft inventories declared upstream components and locked workspace dependency metadata. It does not attest the contents of built release binaries or OCI images, provide complete upstream license texts, or certify legal license compliance. Gate C16/C17 remain pending.

## Kubernetes Core Supervised Components

| Component | Version / Ref | License (SPDX) | Upstream Authority | Architectural Role |
|---|---|---|---|---|
| `kube-apiserver` | `v1.35.7` | `Apache-2.0` | [The Kubernetes Authors](https://github.com/kubernetes/kubernetes) | Supervised official API server binary handling authenticated HTTPS/JSON and admission |
| `kube-controller-manager` | `v1.35.7` | `Apache-2.0` | [The Kubernetes Authors](https://github.com/kubernetes/kubernetes) | Supervised official controller manager driving Job, Pod, and core reconcilers |
| `kubelet` | `v1.35.7` | `Apache-2.0` | [The Kubernetes Authors](https://github.com/kubernetes/kubernetes) | Supervised node agent coordinating single-node pod lifecycles and CRI communication |
| `kube-proxy` | `v1.35.7` | `Apache-2.0` | [The Kubernetes Authors](https://github.com/kubernetes/kubernetes) | Supervised service routing agent managing host iptables and nftables egress rules |

## Datastore and Container Runtime

| Component | Version / Ref | License (SPDX) | Upstream Authority | Architectural Role |
|---|---|---|---|---|
| `kine` | `v0.16.3` | `Apache-2.0` | [Rancher Labs, Inc. / The K3s Authors](https://github.com/k3s-io/kine) | Supervised etcd-to-SQLite translation daemon accessed over dedicated loopback mTLS |
| `sqlite (embedded in kine)` | `3.x` | `blessing` | [Public Domain / D. Richard Hipp](https://sqlite.org) | On-disk single-node relational datastore engine powering Kine state persistence |
| `containerd` | `v2.2.5` | `Apache-2.0` | [The containerd Authors](https://github.com/containerd/containerd) | Supervised core container runtime daemon implementing CRI v1 and OCI image services |
| `containerd-shim-runc-v2` | `v2.2.5` | `Apache-2.0` | [The containerd Authors](https://github.com/containerd/containerd) | Containerd runtime shim managing headless container execution and process accounting |
| `crun` | `1.26` | `GPL-2.0-or-later` | [Red Hat, Inc. and contributors](https://github.com/containers/crun) | Lightweight OCI runtime binary executing container processes under containerd supervision |

## OCI Container Images

| Component | Version / Ref | License (SPDX) | Upstream Authority | Architectural Role |
|---|---|---|---|---|
| `CoreDNS` | `1.14.4` | `Apache-2.0` | [The CoreDNS Authors](https://github.com/coredns/coredns) | Cluster DNS resolver pod providing service discovery across cluster namespaces |
| `pause sandbox` | `latest (v3.10 baseline)` | `Apache-2.0` | [The Kubernetes Authors / Portainer.io](https://github.com/kubernetes/kubernetes) | CRI sandbox container holding network namespace and IPC resources for pods |
| `local-path-provisioner` | `v0.0.36` | `Apache-2.0` | [Rancher Labs, Inc.](https://github.com/rancher/local-path-provisioner) | Persistent volume controller provisioning hostPath-backed storage with Retain reclaim policy |
| `local-path helper (busybox)` | `latest (image contents unresolved)` | `GPL-2.0-only` | [Erik Andersen, Rob Landley, Denys Vlasenko, and others](https://busybox.net) | Helper utility pod used by local-path-provisioner for volume initialization |
| `Portainer Edge Agent` | `lts` | `Zlib` | [Portainer.io](https://github.com/portainer/agent) | Optional management agent establishing reverse tunnel connectivity to Portainer Server |
| `D2K` | `1.2.3` | `Apache-2.0` | [Portainer.io](https://github.com/portainer/d2k) | Optional Docker-to-Kubernetes translation gateway exposing Docker Engine API on port 2376 |
| `KubeSolo node image` | `latest` | `Apache-2.0` | [Portainer.io](https://github.com/portainer/kubesolo) | Containerized node distribution image for container execution modes |

## Networking and Snapshotter Utilities

| Component | Version / Ref | License (SPDX) | Upstream Authority | Architectural Role |
|---|---|---|---|---|
| `containernetworking-plugins` | `v1.9.0` | `Apache-2.0` | [The CNI Authors](https://github.com/containernetworking/plugins) | CNI plugins (bridge, host-local, portmap, loopback) managing pod network namespaces |
| `containerd-fuse-overlayfs-grpc` | `v2.1.7` | `Apache-2.0` | [The containerd Authors](https://github.com/containerd/fuse-overlayfs-snapshotter) | Supervised snapshotter plugin providing unprivileged fuse-overlayfs storage roots |

## Rust Workspace Dependencies

Locked workspace Cargo metadata supplies the following declared dependency licenses. This includes development/tooling and potentially inactive dependencies; it is not an inventory of crates linked into a particular release binary.

| Crate | Version | SPDX License | Source |
|---|---|---|---|
| `adler2` | `2.0.1` | `0BSD OR MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `aho-corasick` | `1.1.5` | `Unlicense OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `anyhow` | `1.0.104` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `arraydeque` | `0.5.1` | `MIT/Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `asn1-rs` | `0.7.2` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `asn1-rs-derive` | `0.6.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `asn1-rs-impl` | `0.2.0` | `MIT/Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `async-trait` | `0.1.89` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `atomic-waker` | `1.1.2` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `autocfg` | `1.5.1` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `aws-lc-rs` | `1.18.1` | `ISC AND (Apache-2.0 OR ISC)` | `registry+https://github.com/rust-lang/crates.io-index` |
| `aws-lc-sys` | `0.45.0` | `ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0)` | `registry+https://github.com/rust-lang/crates.io-index` |
| `base64` | `0.22.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `base64` | `0.23.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `base64ct` | `1.8.3` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `bit-vec` | `0.9.1` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `bitflags` | `1.3.2` | `MIT/Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `bitflags` | `2.13.2` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `block-buffer` | `0.12.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `bytes` | `1.12.1` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `cc` | `1.5.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `cfg-if` | `1.0.5` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `cmake` | `0.1.58` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `const-oid` | `0.9.6` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `cpufeatures` | `0.3.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `crc32fast` | `1.5.2` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `crypto-common` | `0.2.2` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `data-encoding` | `2.11.1` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `defmt` | `1.1.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `defmt-macros` | `1.1.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `defmt-parser` | `1.0.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `der` | `0.7.10` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `der-parser` | `10.0.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `deranged` | `0.5.8` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `digest` | `0.11.3` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `displaydoc` | `0.2.7` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `dunce` | `1.0.5` | `CC0-1.0 OR MIT-0 OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `either` | `1.18.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `equivalent` | `1.0.2` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `errno` | `0.3.14` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `fastrand` | `2.5.0` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `filetime` | `0.2.29` | `MIT/Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `find-msvc-tools` | `0.1.14` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `fixedbitset` | `0.5.7` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `flate2` | `1.1.10` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `fnv` | `1.0.7` | `Apache-2.0 / MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `foldhash` | `0.1.5` | `Zlib` | `registry+https://github.com/rust-lang/crates.io-index` |
| `fs_extra` | `1.3.0` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `futures-channel` | `0.3.34` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `futures-core` | `0.3.34` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `futures-sink` | `0.3.34` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `futures-task` | `0.3.34` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `futures-util` | `0.3.34` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `getrandom` | `0.4.3` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `h2` | `0.4.19` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `hashbrown` | `0.15.5` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `heck` | `0.5.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `http` | `1.5.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `http-body` | `1.1.0` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `http-body-util` | `0.1.5` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `httparse` | `1.10.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `httpdate` | `1.0.3` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `hybrid-array` | `0.4.15` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `hyper` | `1.11.1` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `hyper-timeout` | `0.5.2` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `hyper-util` | `0.1.21` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `indexmap` | `2.11.3` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `itertools` | `0.14.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `itoa` | `1.0.18` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `jiff` | `0.2.37` | `Unlicense OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `jiff-core` | `0.1.1` | `Unlicense OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `jiff-static` | `0.2.37` | `Unlicense OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `jobserver` | `0.1.35` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `k8s-openapi` | `0.28.0` | `Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `lazy_static` | `1.5.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `libc` | `0.2.189` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `linux-raw-sys` | `0.12.1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `log` | `0.4.34` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `memchr` | `2.8.3` | `Unlicense OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `minimal-lexical` | `0.2.1` | `MIT/Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `miniz_oxide` | `0.9.1` | `MIT OR Zlib OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `mio` | `1.2.3` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `multimap` | `0.10.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `nom` | `7.1.3` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `num-bigint` | `0.4.8` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `num-conv` | `0.2.2` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `num-integer` | `0.1.47` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `num-traits` | `0.2.19` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `object` | `0.39.1` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `oid-registry` | `0.8.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `once_cell` | `1.21.4` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `pem` | `4.0.0` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `pem-rfc7468` | `0.7.0` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `percent-encoding` | `2.3.2` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `petgraph` | `0.8.3` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `pin-project` | `1.1.13` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `pin-project-internal` | `1.1.13` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `pin-project-lite` | `0.2.17` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `pkcs8` | `0.10.2` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `pkg-config` | `0.3.34` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `portable-atomic` | `1.15.0` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `portable-atomic-util` | `0.2.7` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `powerfmt` | `0.2.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `prettyplease` | `0.2.37` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `proc-macro2` | `1.0.107` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `prost` | `0.14.4` | `Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `prost-build` | `0.14.4` | `Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `prost-derive` | `0.14.4` | `Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `prost-types` | `0.14.4` | `Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `pulldown-cmark` | `0.13.4` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `pulldown-cmark-to-cmark` | `22.0.1` | `Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `quote` | `1.0.47` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `r-efi` | `6.0.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | `registry+https://github.com/rust-lang/crates.io-index` |
| `rcgen` | `0.14.10` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `regex` | `1.13.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `regex-automata` | `0.4.18` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `regex-syntax` | `0.8.11` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `rubix-apiserver` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-assets` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-config` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-containerd` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-containerd-api` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-controller` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-cri` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-datastore` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-dev` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-dns` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-kube` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-kubelet` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-network` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-pki` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-platform` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-portainer` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-proxy` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-storage` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-supervisor` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubix-upstream-codegen` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rubixctl` | `0.1.0` | `unrecorded` | `workspace path crate` |
| `rusticata-macros` | `4.1.0` | `MIT/Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `rustix` | `1.1.5` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `rustls-pki-types` | `1.15.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `saphyr-parser` | `0.0.11` | `MIT OR Apache-2.0` | `local path dependency` |
| `serde` | `1.0.228` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `serde_core` | `1.0.228` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `serde_derive` | `1.0.228` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `serde_json` | `1.0.151` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `serde_spanned` | `1.1.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `sha2` | `0.11.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `shlex` | `2.0.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `signal-hook-registry` | `1.4.8` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `simd-adler32` | `0.3.10` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `slab` | `0.4.12` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `smallvec` | `1.16.2` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `socket2` | `0.6.5` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `spki` | `0.7.3` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `syn` | `2.0.119` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `syn` | `3.0.6` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `sync_wrapper` | `1.0.2` | `Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `synstructure` | `0.13.2` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tar` | `0.4.46` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tempfile` | `3.27.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `thiserror` | `2.0.18` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `thiserror-impl` | `2.0.18` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `time` | `0.3.55` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `time-core` | `0.1.9` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `time-macros` | `0.2.32` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tokio` | `1.53.1` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tokio-macros` | `2.7.1` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tokio-stream` | `0.1.19` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tokio-util` | `0.7.19` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `toml` | `0.9.6` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `toml_datetime` | `0.7.5+spec-1.1.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `toml_parser` | `1.0.5+spec-1.0.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `toml_writer` | `1.1.2+spec-1.1.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tonic` | `0.14.6` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tonic-build` | `0.14.6` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tonic-prost` | `0.14.6` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tonic-prost-build` | `0.14.6` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tower` | `0.5.3` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tower-layer` | `0.3.3` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tower-service` | `0.3.3` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tracing` | `0.1.44` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tracing-attributes` | `0.1.31` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `tracing-core` | `0.1.36` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `try-lock` | `0.2.5` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `typed-path` | `0.12.3` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `typenum` | `1.20.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `unicase` | `2.9.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `unicode-ident` | `1.0.26` | `(MIT OR Apache-2.0) AND Unicode-3.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `untrusted` | `0.7.1` | `ISC` | `registry+https://github.com/rust-lang/crates.io-index` |
| `want` | `0.3.1` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `wasi` | `0.11.1+wasi-snapshot-preview1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `windows-link` | `0.2.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `windows-sys` | `0.61.2` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `winnow` | `0.7.15` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `x509-parser` | `0.18.1` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `yasna` | `0.6.0` | `MIT OR Apache-2.0` | `registry+https://github.com/rust-lang/crates.io-index` |
| `zeroize` | `1.9.0` | `Apache-2.0 OR MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `zip` | `8.6.0` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `zmij` | `1.0.23` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `zstd` | `0.13.3` | `MIT` | `registry+https://github.com/rust-lang/crates.io-index` |
| `zstd-safe` | `7.3.0` | `BSD-3-Clause` | `registry+https://github.com/rust-lang/crates.io-index` |
| `zstd-sys` | `2.1.0+zstd.1.5.7` | `BSD-3-Clause` | `registry+https://github.com/rust-lang/crates.io-index` |

## License Texts and Notices

### Apache-2.0

```
Apache License, Version 2.0
http://www.apache.org/licenses/LICENSE-2.0
```

### GPL-2.0-only

```
GNU General Public License, Version 2.0
https://www.gnu.org/licenses/old-licenses/gpl-2.0.html
```

### GPL-2.0-or-later

```
GNU General Public License, Version 2.0 or later
https://www.gnu.org/licenses/gpl-2.0.html
```

### ISC

```
ISC License

Copyright (c) 2026 Geoff S. and rubix-kube contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

```

### MIT

```
MIT License
https://opensource.org/licenses/MIT
```

### Zlib

```
zlib License
https://opensource.org/licenses/Zlib
```

### blessing

```
SQLite Blessing
May you do good and not evil.
May you find forgiveness for yourself and forgive others.
May you share freely, never taking more than you give.
```

