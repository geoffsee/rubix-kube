# Rubix v0.1.0 Qualified Release Evidence and Checksum Manifests

**Gate**: Gate C16/C17 Production Readiness (Epic E30 / Issue #126)  
**Status**: QUALIFIED RELEASE EVIDENCE ASSEMBLED  
**Distribution**: Single-Node Kubernetes Distribution Supervising Retained Upstream Executables

This directory consolidates the authoritative cryptographic checksum manifests (`SHA256SUMS`), release package metadata (`release-manifest.json`), source provenance (`provenance.json`), upstream license attribution (`attribution.md`, `licenses.json`), production release notes (`release-notes.md`), and domain qualification reports binding all 16 Linux node variant cells and 4 management binaries to their verified evidence.

## 1. Supported Node Variant Matrix (16 Archive Cells)

| Cell | Architecture | Libc | Variant | Archive Filename | SHA-256 Digest | Bound Qualification Report |
|---|---|---|---|---|---|---|
| 01 | `amd64` | `glibc` | `online` | `rubix-kube-0.1.0-linux-amd64.tar.gz` | `df6bf585f9d2fb74a025a6d2d0b90d279066b0970c6920225dd916cd185a781f` | conformance-qualification-report.json, performance-qualification-report.json (amd64) |
| 02 | `amd64` | `glibc` | `offline` | `rubix-kube-0.1.0-linux-amd64-offline.tar.gz` | `a53aee3ca690fbf07c9d94fcb1590ca559e5e99bbf38bd7690c669150b3a9c59` | conformance-qualification-report.json, performance-qualification-report.json (amd64) |
| 03 | `amd64` | `musl` | `online` | `rubix-kube-0.1.0-linux-amd64-musl.tar.gz` | `5162a40c0dc9088b42d3730502ba60e0ae2cc282f4a6ed8ab3f85f20ffd79dc7` | conformance-qualification-report.json, performance-qualification-report.json (amd64) |
| 04 | `amd64` | `musl` | `offline` | `rubix-kube-0.1.0-linux-amd64-musl-offline.tar.gz` | `59f9d25852954d1455b3c78d90f8a5e29a72723b07d0b85a4830e5baa61a0e2e` | conformance-qualification-report.json, performance-qualification-report.json (amd64) |
| 05 | `arm64` | `glibc` | `online` | `rubix-kube-0.1.0-linux-arm64.tar.gz` | `b8a3f11c1edf96461807465df723f0c2e6b91683f785b319dda5d658e5535bdf` | conformance-qualification-report.json, performance-qualification-report.json (arm64) |
| 06 | `arm64` | `glibc` | `offline` | `rubix-kube-0.1.0-linux-arm64-offline.tar.gz` | `5f3a035168a368e68ae6fff027628bd3d234b4e32daeb08a0da786adc1bde996` | conformance-qualification-report.json, performance-qualification-report.json (arm64) |
| 07 | `arm64` | `musl` | `online` | `rubix-kube-0.1.0-linux-arm64-musl.tar.gz` | `4c40663f219535cfc1f07bbdfc7ac789affaff76e9f8be1cf040ca99a7405d41` | conformance-qualification-report.json, performance-qualification-report.json (arm64) |
| 08 | `arm64` | `musl` | `offline` | `rubix-kube-0.1.0-linux-arm64-musl-offline.tar.gz` | `5a5768897e2e6824b5198e91ee0737bd0ae0fccf86c947a43e55de78136c4c54` | conformance-qualification-report.json, performance-qualification-report.json (arm64) |
| 09 | `arm` | `glibc` | `online` | `rubix-kube-0.1.0-linux-arm.tar.gz` | `8758bb47318e030bc0192464c9d1f5328d10288b17303d4b5da42260088786ca` | conformance-qualification-report.json, state-transition-qualification-report.json |
| 10 | `arm` | `glibc` | `offline` | `rubix-kube-0.1.0-linux-arm-offline.tar.gz` | `9533df94b4c6189351577bbe536d686d015e09f0b6f2a364a4c225dbd7e791c3` | conformance-qualification-report.json, state-transition-qualification-report.json |
| 11 | `arm` | `musl` | `online` | `rubix-kube-0.1.0-linux-arm-musl.tar.gz` | `f18520b95661cd3957ddb1ff81f0f7b202863064c78cb7063ee15aaaf1ed061c` | conformance-qualification-report.json, state-transition-qualification-report.json |
| 12 | `arm` | `musl` | `offline` | `rubix-kube-0.1.0-linux-arm-musl-offline.tar.gz` | `13cc6dfe886d9e3b02d184df555934e8d11b99f0eecd0d4e373d366b97577ab2` | conformance-qualification-report.json, state-transition-qualification-report.json |
| 13 | `riscv64` | `glibc` | `online` | `rubix-kube-0.1.0-linux-riscv64.tar.gz` | `359356e6dd9d8c7b01ea6638b5145d3735393004df1751a93d8fae34f51b5ade` | conformance-qualification-report.json, state-transition-qualification-report.json |
| 14 | `riscv64` | `glibc` | `offline` | `rubix-kube-0.1.0-linux-riscv64-offline.tar.gz` | `975c7bd62b6e6368540643cd6da348a82558e7f333a4c682887190baab141dc2` | conformance-qualification-report.json, state-transition-qualification-report.json |
| 15 | `riscv64` | `musl` | `online` | `rubix-kube-0.1.0-linux-riscv64-musl.tar.gz` | `a56dfd703a5e8c9fea7082d759884ea7fc1970ee291f855ef7491445e67edf96` | conformance-qualification-report.json, state-transition-qualification-report.json |
| 16 | `riscv64` | `musl` | `offline` | `rubix-kube-0.1.0-linux-riscv64-musl-offline.tar.gz` | `84375e705197cfd0b071cf8ab625a9115b044999c1593d7485f7be36388b3ba6` | conformance-qualification-report.json, state-transition-qualification-report.json |

## 2. Management Targets (4 Artifacts)

| Operating System | Architecture | Binary Filename | SHA-256 Digest |
|---|---|---|---|
| `linux` | `amd64` | `rubixctl-linux-amd64` | `c372b6c9fd1a616bc328400f202d8319b21bae735cfc27c82576dc3eb7c6440e` |
| `linux` | `arm64` | `rubixctl-linux-arm64` | `049272ff5f35c67166353422fc544efd2b612a57bc3eb4874b80c9ed6f39961a` |
| `darwin` | `amd64` | `rubixctl-darwin-amd64` | `14ef3685535401b1a839a8014824995fe58c109910c14fb4ea7aea95ad59b8a1` |
| `darwin` | `arm64` | `rubixctl-darwin-arm64` | `079393e68940a05d61b8ed7739b8b286ca83b33ddb769419a3417128d7aeb8ef` |

## 3. Qualification Reports and Evidence Inventory

| Document | Kind | Description | Status |
|---|---|---|---|
| [`SHA256SUMS`](SHA256SUMS) | Cryptographic Manifest | Cryptographic checksum manifest binding all 16 node variants, 4 management binaries, and reports | AUTHORITATIVE |
| [`release-manifest.json`](release-manifest.json) | Package Descriptor | Full distribution manifest specifying 16 node cells, 4 management targets, and 7 OCI images | VERIFIED |
| [`provenance.json`](provenance.json) | Source Provenance | Upstream source commits, generator inputs, and reproducibility declarations | VERIFIED |
| [`attribution.md`](attribution.md) | License Attribution | Comprehensive license attribution for Kubernetes, Kine, containerd, and OCI images | COMPLETE |
| [`licenses.json`](licenses.json) | License Inventory | Structured machine-readable license inventory from Cargo metadata and catalog | COMPLETE |
| [`release-notes.md`](release-notes.md) | Release Notes | Production release notes: version transitions, breaking changes, budgets, disclaimer | PRODUCTION |
| [`conformance-qualification-report.json`](conformance-qualification-report.json) | Conformance Report | Qualification report across 6 manifest domains, 3 smoke checks, 64 conformance tests | VERIFIED (synthetic fixture) |
| [`performance-qualification-report.json`](performance-qualification-report.json) | Performance Report | 12 contract thresholds evaluation, 24h soak growth ratio, 8 retained processes | EVALUATED (paired baselines) |
| [`state-transition-qualification-report.json`](state-transition-qualification-report.json) | Migration Report | Go-to-Rust state transition qualification across v1.1.8-v1.3.3 and 5 state domains | VERIFIED (fixture model) |

## 4. Mandatory Multi-Node Non-Certification Disclaimer

> [!IMPORTANT]
> Synthetic in-process fixtures only. C13/E28 and the selected upstream conformance suite remain unqualified. DO NOT claim official CNCF Certified Kubernetes qualification or multi-node certification.

## 5. Verification Command

To verify all release evidence, checksum manifests, license attributions, and release notes:

```sh
cargo run --locked -p rubix-dev --bin rubix-release -- verify docs/release
```
