# Upstream License Attribution and Third-Party Notices

**Rubix Kubernetes Distribution Version**: `0.1.0`  
**Primary Distribution License**: `Apache-2.0`

This document compiles formal third-party license notices, copyright declarations, and architectural responsibilities for all retained upstream executables, OCI container images, protocol modules, and compiled Rust dependencies included in or supervised by the Rubix Kubernetes Distribution, in satisfaction of Gate C16/C17 (Epic E30 / Issue #126).

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
| `CoreDNS` | `1.14.4` | `Apache-2.0` | [The CoreDNS Authors](docker.io/coredns/coredns:1.14.4) | Cluster DNS resolver pod providing service discovery across cluster namespaces |
| `pause sandbox` | `latest (v3.10 baseline)` | `Apache-2.0` | [The Kubernetes Authors / Portainer.io](docker.io/portainer/pause:latest) | CRI sandbox container holding network namespace and IPC resources for pods |
| `local-path-provisioner` | `v0.0.36` | `Apache-2.0` | [Rancher Labs, Inc.](docker.io/rancher/local-path-provisioner:v0.0.36) | Persistent volume controller provisioning hostPath-backed storage with Retain reclaim policy |
| `local-path helper (busybox)` | `latest (1.37.0 baseline)` | `GPL-2.0-only` | [Erik Andersen, Rob Landley, Denys Vlasenko, and others](docker.io/library/busybox:latest) | Helper utility pod used by local-path-provisioner for volume initialization |
| `Portainer Edge Agent` | `lts` | `Zlib` | [Portainer.io](docker.io/portainer/agent:lts) | Optional management agent establishing reverse tunnel connectivity to Portainer Server |
| `D2K` | `1.2.3` | `Apache-2.0` | [Portainer.io](docker.io/portainer/d2k:1.2.3) | Optional Docker-to-Kubernetes translation gateway exposing Docker Engine API on port 2376 |
| `KubeSolo node image` | `latest` | `Apache-2.0` | [Portainer.io](ghcr.io/portainer/kubesolo:latest) | Containerized node distribution image for container execution modes |

## Networking and Snapshotter Utilities

| Component | Version / Ref | License (SPDX) | Upstream Authority | Architectural Role |
|---|---|---|---|---|
| `containernetworking-plugins` | `v1.9.0` | `Apache-2.0` | [The CNI Authors](https://github.com/containernetworking/plugins) | CNI plugins (bridge, host-local, portmap, loopback) managing pod network namespaces |
| `containerd-fuse-overlayfs-grpc` | `v2.1.7` | `Apache-2.0` | [The containerd Authors](https://github.com/containerd/fuse-overlayfs-snapshotter) | Supervised snapshotter plugin providing unprivileged fuse-overlayfs storage roots |

## Rust Workspace Dependencies

The Rubix distribution binaries (`rubix-kube`, `rubixctl`) and developer verification tools are built from the pinned Rust toolchain (1.97.1, edition 2024). Below is the complete inventory of compiled Rust crates with their declared SPDX licenses:

| Crate | Version | SPDX License | Source |
|---|---|---|---|
| `adler2` | `2.0.1` | `0BSD OR MIT OR Apache-2.0` | `crates.io` |
| `aho-corasick` | `1.1.5` | `Unlicense OR MIT` | `crates.io` |
| `anyhow` | `1.0.104` | `MIT OR Apache-2.0` | `crates.io` |
| `arraydeque` | `0.5.1` | `MIT/Apache-2.0` | `crates.io` |
| `asn1-rs` | `0.7.2` | `MIT OR Apache-2.0` | `crates.io` |
| `asn1-rs-derive` | `0.6.0` | `MIT OR Apache-2.0` | `crates.io` |
| `asn1-rs-impl` | `0.2.0` | `MIT/Apache-2.0` | `crates.io` |
| `async-trait` | `0.1.89` | `MIT OR Apache-2.0` | `crates.io` |
| `atomic-waker` | `1.1.2` | `Apache-2.0 OR MIT` | `crates.io` |
| `autocfg` | `1.5.1` | `Apache-2.0 OR MIT` | `crates.io` |
| `aws-lc-rs` | `1.18.1` | `ISC AND (Apache-2.0 OR ISC)` | `crates.io` |
| `aws-lc-sys` | `0.45.0` | `ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0)` | `crates.io` |
| `base64` | `0.22.1` | `MIT OR Apache-2.0` | `crates.io` |
| `base64` | `0.23.1` | `MIT OR Apache-2.0` | `crates.io` |
| `base64ct` | `1.8.3` | `Apache-2.0 OR MIT` | `crates.io` |
| `bit-vec` | `0.9.1` | `Apache-2.0 OR MIT` | `crates.io` |
| `bitflags` | `1.3.2` | `MIT/Apache-2.0` | `crates.io` |
| `bitflags` | `2.13.2` | `MIT OR Apache-2.0` | `crates.io` |
| `block-buffer` | `0.12.1` | `MIT OR Apache-2.0` | `crates.io` |
| `bytes` | `1.12.1` | `MIT` | `crates.io` |
| `cc` | `1.5.1` | `MIT OR Apache-2.0` | `crates.io` |
| `cfg-if` | `1.0.5` | `MIT OR Apache-2.0` | `crates.io` |
| `cmake` | `0.1.58` | `MIT OR Apache-2.0` | `crates.io` |
| `const-oid` | `0.9.6` | `Apache-2.0 OR MIT` | `crates.io` |
| `cpufeatures` | `0.3.1` | `MIT OR Apache-2.0` | `crates.io` |
| `crc32fast` | `1.5.2` | `MIT OR Apache-2.0` | `crates.io` |
| `crypto-common` | `0.2.2` | `MIT OR Apache-2.0` | `crates.io` |
| `data-encoding` | `2.11.1` | `MIT` | `crates.io` |
| `defmt` | `1.1.1` | `MIT OR Apache-2.0` | `crates.io` |
| `defmt-macros` | `1.1.1` | `MIT OR Apache-2.0` | `crates.io` |
| `defmt-parser` | `1.0.0` | `MIT OR Apache-2.0` | `crates.io` |
| `der` | `0.7.10` | `Apache-2.0 OR MIT` | `crates.io` |
| `der-parser` | `10.0.0` | `MIT OR Apache-2.0` | `crates.io` |
| `deranged` | `0.5.8` | `MIT OR Apache-2.0` | `crates.io` |
| `digest` | `0.11.3` | `MIT OR Apache-2.0` | `crates.io` |
| `displaydoc` | `0.2.7` | `MIT OR Apache-2.0` | `crates.io` |
| `dunce` | `1.0.5` | `CC0-1.0 OR MIT-0 OR Apache-2.0` | `crates.io` |
| `either` | `1.18.0` | `MIT OR Apache-2.0` | `crates.io` |
| `equivalent` | `1.0.2` | `Apache-2.0 OR MIT` | `crates.io` |
| `errno` | `0.3.14` | `MIT OR Apache-2.0` | `crates.io` |
| `fastrand` | `2.5.0` | `Apache-2.0 OR MIT` | `crates.io` |
| `filetime` | `0.2.29` | `MIT/Apache-2.0` | `crates.io` |
| `find-msvc-tools` | `0.1.14` | `MIT OR Apache-2.0` | `crates.io` |
| `fixedbitset` | `0.5.7` | `MIT OR Apache-2.0` | `crates.io` |
| `flate2` | `1.1.10` | `MIT OR Apache-2.0` | `crates.io` |
| `fnv` | `1.0.7` | `Apache-2.0 / MIT` | `crates.io` |
| `foldhash` | `0.1.5` | `Zlib` | `crates.io` |
| `fs_extra` | `1.3.0` | `MIT` | `crates.io` |
| `futures-channel` | `0.3.34` | `MIT OR Apache-2.0` | `crates.io` |
| `futures-core` | `0.3.34` | `MIT OR Apache-2.0` | `crates.io` |
| `futures-sink` | `0.3.34` | `MIT OR Apache-2.0` | `crates.io` |
| `futures-task` | `0.3.34` | `MIT OR Apache-2.0` | `crates.io` |
| `futures-util` | `0.3.34` | `MIT OR Apache-2.0` | `crates.io` |
| `getrandom` | `0.4.3` | `MIT OR Apache-2.0` | `crates.io` |
| `h2` | `0.4.19` | `MIT` | `crates.io` |
| `hashbrown` | `0.15.5` | `MIT OR Apache-2.0` | `crates.io` |
| `heck` | `0.5.0` | `MIT OR Apache-2.0` | `crates.io` |
| `http` | `1.5.0` | `MIT OR Apache-2.0` | `crates.io` |
| `http-body` | `1.1.0` | `MIT` | `crates.io` |
| `http-body-util` | `0.1.5` | `MIT` | `crates.io` |
| `httparse` | `1.10.1` | `MIT OR Apache-2.0` | `crates.io` |
| `httpdate` | `1.0.3` | `MIT OR Apache-2.0` | `crates.io` |
| `hybrid-array` | `0.4.15` | `MIT OR Apache-2.0` | `crates.io` |
| `hyper` | `1.11.1` | `MIT` | `crates.io` |
| `hyper-timeout` | `0.5.2` | `MIT OR Apache-2.0` | `crates.io` |
| `hyper-util` | `0.1.21` | `MIT` | `crates.io` |
| `indexmap` | `2.11.3` | `Apache-2.0 OR MIT` | `crates.io` |
| `itertools` | `0.14.0` | `MIT OR Apache-2.0` | `crates.io` |
| `itoa` | `1.0.18` | `MIT OR Apache-2.0` | `crates.io` |
| `jiff` | `0.2.37` | `Unlicense OR MIT` | `crates.io` |
| `jiff-core` | `0.1.1` | `Unlicense OR MIT` | `crates.io` |
| `jiff-static` | `0.2.37` | `Unlicense OR MIT` | `crates.io` |
| `jobserver` | `0.1.35` | `MIT OR Apache-2.0` | `crates.io` |
| `k8s-openapi` | `0.28.0` | `Apache-2.0` | `crates.io` |
| `lazy_static` | `1.5.0` | `MIT OR Apache-2.0` | `crates.io` |
| `libc` | `0.2.189` | `MIT OR Apache-2.0` | `crates.io` |
| `linux-raw-sys` | `0.12.1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `crates.io` |
| `log` | `0.4.34` | `MIT OR Apache-2.0` | `crates.io` |
| `memchr` | `2.8.3` | `Unlicense OR MIT` | `crates.io` |
| `minimal-lexical` | `0.2.1` | `MIT/Apache-2.0` | `crates.io` |
| `miniz_oxide` | `0.9.1` | `MIT OR Zlib OR Apache-2.0` | `crates.io` |
| `mio` | `1.2.3` | `MIT` | `crates.io` |
| `multimap` | `0.10.1` | `MIT OR Apache-2.0` | `crates.io` |
| `nom` | `7.1.3` | `MIT` | `crates.io` |
| `num-bigint` | `0.4.8` | `MIT OR Apache-2.0` | `crates.io` |
| `num-conv` | `0.2.2` | `MIT OR Apache-2.0` | `crates.io` |
| `num-integer` | `0.1.47` | `MIT OR Apache-2.0` | `crates.io` |
| `num-traits` | `0.2.19` | `MIT OR Apache-2.0` | `crates.io` |
| `object` | `0.39.1` | `Apache-2.0 OR MIT` | `crates.io` |
| `oid-registry` | `0.8.1` | `MIT OR Apache-2.0` | `crates.io` |
| `once_cell` | `1.21.4` | `MIT OR Apache-2.0` | `crates.io` |
| `pem` | `4.0.0` | `MIT` | `crates.io` |
| `pem-rfc7468` | `0.7.0` | `Apache-2.0 OR MIT` | `crates.io` |
| `percent-encoding` | `2.3.2` | `MIT OR Apache-2.0` | `crates.io` |
| `petgraph` | `0.8.3` | `MIT OR Apache-2.0` | `crates.io` |
| `pin-project` | `1.1.13` | `Apache-2.0 OR MIT` | `crates.io` |
| `pin-project-internal` | `1.1.13` | `Apache-2.0 OR MIT` | `crates.io` |
| `pin-project-lite` | `0.2.17` | `Apache-2.0 OR MIT` | `crates.io` |
| `pkcs8` | `0.10.2` | `Apache-2.0 OR MIT` | `crates.io` |
| `pkg-config` | `0.3.34` | `MIT OR Apache-2.0` | `crates.io` |
| `portable-atomic` | `1.15.0` | `Apache-2.0 OR MIT` | `crates.io` |
| `portable-atomic-util` | `0.2.7` | `Apache-2.0 OR MIT` | `crates.io` |
| `powerfmt` | `0.2.0` | `MIT OR Apache-2.0` | `crates.io` |
| `prettyplease` | `0.2.37` | `MIT OR Apache-2.0` | `crates.io` |
| `proc-macro2` | `1.0.107` | `MIT OR Apache-2.0` | `crates.io` |
| `prost` | `0.14.4` | `Apache-2.0` | `crates.io` |
| `prost-build` | `0.14.4` | `Apache-2.0` | `crates.io` |
| `prost-derive` | `0.14.4` | `Apache-2.0` | `crates.io` |
| `prost-types` | `0.14.4` | `Apache-2.0` | `crates.io` |
| `pulldown-cmark` | `0.13.4` | `MIT` | `crates.io` |
| `pulldown-cmark-to-cmark` | `22.0.1` | `Apache-2.0` | `crates.io` |
| `quote` | `1.0.47` | `MIT OR Apache-2.0` | `crates.io` |
| `r-efi` | `6.0.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | `crates.io` |
| `rcgen` | `0.14.10` | `MIT OR Apache-2.0` | `crates.io` |
| `regex` | `1.13.1` | `MIT OR Apache-2.0` | `crates.io` |
| `regex-automata` | `0.4.18` | `MIT OR Apache-2.0` | `crates.io` |
| `regex-syntax` | `0.8.11` | `MIT OR Apache-2.0` | `crates.io` |
| `rubix-apiserver` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-assets` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-config` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-containerd` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-containerd-api` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-controller` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-cri` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-datastore` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-dev` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-dns` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-kube` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-kubelet` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-network` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-pki` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-platform` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-portainer` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-proxy` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-storage` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-supervisor` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubix-upstream-codegen` | `0.1.0` | `unrecorded` | `crates.io` |
| `rubixctl` | `0.1.0` | `unrecorded` | `crates.io` |
| `rusticata-macros` | `4.1.0` | `MIT/Apache-2.0` | `crates.io` |
| `rustix` | `1.1.5` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `crates.io` |
| `rustls-pki-types` | `1.15.1` | `MIT OR Apache-2.0` | `crates.io` |
| `saphyr-parser` | `0.0.11` | `MIT OR Apache-2.0` | `crates.io` |
| `serde` | `1.0.228` | `MIT OR Apache-2.0` | `crates.io` |
| `serde_core` | `1.0.228` | `MIT OR Apache-2.0` | `crates.io` |
| `serde_derive` | `1.0.228` | `MIT OR Apache-2.0` | `crates.io` |
| `serde_json` | `1.0.151` | `MIT OR Apache-2.0` | `crates.io` |
| `serde_spanned` | `1.1.1` | `MIT OR Apache-2.0` | `crates.io` |
| `sha2` | `0.11.0` | `MIT OR Apache-2.0` | `crates.io` |
| `shlex` | `2.0.1` | `MIT OR Apache-2.0` | `crates.io` |
| `signal-hook-registry` | `1.4.8` | `MIT OR Apache-2.0` | `crates.io` |
| `simd-adler32` | `0.3.10` | `MIT` | `crates.io` |
| `slab` | `0.4.12` | `MIT` | `crates.io` |
| `smallvec` | `1.16.2` | `MIT OR Apache-2.0` | `crates.io` |
| `socket2` | `0.6.5` | `MIT OR Apache-2.0` | `crates.io` |
| `spki` | `0.7.3` | `Apache-2.0 OR MIT` | `crates.io` |
| `syn` | `2.0.119` | `MIT OR Apache-2.0` | `crates.io` |
| `syn` | `3.0.6` | `MIT OR Apache-2.0` | `crates.io` |
| `sync_wrapper` | `1.0.2` | `Apache-2.0` | `crates.io` |
| `synstructure` | `0.13.2` | `MIT` | `crates.io` |
| `tar` | `0.4.46` | `MIT OR Apache-2.0` | `crates.io` |
| `tempfile` | `3.27.0` | `MIT OR Apache-2.0` | `crates.io` |
| `thiserror` | `2.0.18` | `MIT OR Apache-2.0` | `crates.io` |
| `thiserror-impl` | `2.0.18` | `MIT OR Apache-2.0` | `crates.io` |
| `time` | `0.3.55` | `MIT OR Apache-2.0` | `crates.io` |
| `time-core` | `0.1.9` | `MIT OR Apache-2.0` | `crates.io` |
| `time-macros` | `0.2.32` | `MIT OR Apache-2.0` | `crates.io` |
| `tokio` | `1.53.1` | `MIT` | `crates.io` |
| `tokio-macros` | `2.7.1` | `MIT` | `crates.io` |
| `tokio-stream` | `0.1.19` | `MIT` | `crates.io` |
| `tokio-util` | `0.7.19` | `MIT` | `crates.io` |
| `toml` | `0.9.6` | `MIT OR Apache-2.0` | `crates.io` |
| `toml_datetime` | `0.7.5+spec-1.1.0` | `MIT OR Apache-2.0` | `crates.io` |
| `toml_parser` | `1.0.5+spec-1.0.0` | `MIT OR Apache-2.0` | `crates.io` |
| `toml_writer` | `1.1.2+spec-1.1.0` | `MIT OR Apache-2.0` | `crates.io` |
| `tonic` | `0.14.6` | `MIT` | `crates.io` |
| `tonic-build` | `0.14.6` | `MIT` | `crates.io` |
| `tonic-prost` | `0.14.6` | `MIT` | `crates.io` |
| `tonic-prost-build` | `0.14.6` | `MIT` | `crates.io` |
| `tower` | `0.5.3` | `MIT` | `crates.io` |
| `tower-layer` | `0.3.3` | `MIT` | `crates.io` |
| `tower-service` | `0.3.3` | `MIT` | `crates.io` |
| `tracing` | `0.1.44` | `MIT` | `crates.io` |
| `tracing-attributes` | `0.1.31` | `MIT` | `crates.io` |
| `tracing-core` | `0.1.36` | `MIT` | `crates.io` |
| `try-lock` | `0.2.5` | `MIT` | `crates.io` |
| `typed-path` | `0.12.3` | `MIT OR Apache-2.0` | `crates.io` |
| `typenum` | `1.20.1` | `MIT OR Apache-2.0` | `crates.io` |
| `unicase` | `2.9.0` | `MIT OR Apache-2.0` | `crates.io` |
| `unicode-ident` | `1.0.26` | `(MIT OR Apache-2.0) AND Unicode-3.0` | `crates.io` |
| `untrusted` | `0.7.1` | `ISC` | `crates.io` |
| `want` | `0.3.1` | `MIT` | `crates.io` |
| `wasi` | `0.11.1+wasi-snapshot-preview1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `crates.io` |
| `windows-link` | `0.2.1` | `MIT OR Apache-2.0` | `crates.io` |
| `windows-sys` | `0.61.2` | `MIT OR Apache-2.0` | `crates.io` |
| `winnow` | `0.7.15` | `MIT` | `crates.io` |
| `x509-parser` | `0.18.1` | `MIT OR Apache-2.0` | `crates.io` |
| `yasna` | `0.6.0` | `MIT OR Apache-2.0` | `crates.io` |
| `zeroize` | `1.9.0` | `Apache-2.0 OR MIT` | `crates.io` |
| `zip` | `8.6.0` | `MIT` | `crates.io` |
| `zmij` | `1.0.23` | `MIT` | `crates.io` |
| `zstd` | `0.13.3` | `MIT` | `crates.io` |
| `zstd-safe` | `7.3.0` | `BSD-3-Clause` | `crates.io` |
| `zstd-sys` | `2.1.0+zstd.1.5.7` | `BSD-3-Clause` | `crates.io` |

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

