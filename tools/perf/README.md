# Matched Go and Rust Performance Baselines (Gate C13 / E29.01)

This directory contains the repeatable measurement harness, committed reference baselines,
candidate evidence, and secondary-architecture gap documentation satisfying
**Gate C13** and **Issue #121 ([E29.01])**.

All measurements and assertions strictly follow the [Compatibility Contract](../../docs/architecture/compatibility-contract.md)
and [Acceptance Matrix](../../docs/architecture/acceptance-matrix.md).

---

## 1. Scope & Acceptance Contracts

In accordance with the Rubix compatibility contract:
- **Baseline Authority**: Portainer KubeSolo commit `2ef1c4787989f11f868f81bb84ae2afd4a49a81d`.
- **Primary Architectures**: Paired, identical dedicated Linux environments for `amd64` and `arm64`.
- **Secondary Architectures**: Explicit documentation of architectural limits, disabled features,
  and performance gaps for `armv7` and `riscv64`.
- **Retained Process Boundary**: Every whole-distribution measurement accounts for **all 8 retained
  processes**:
  1. `kube-apiserver` (official v1.35.7)
  2. `kube-controller-manager` (official v1.35.7)
  3. `kubelet` (official v1.35.7)
  4. `kube-proxy` (official v1.35.7)
  5. `kine` (v0.16.3, SQLite datastore engine over loopback mTLS)
  6. `containerd` (official v2.2.5 CRI runtime)
  7. `containerd-shim-runc-v2` (matching v2.2.5 payload)
  8. Distribution Node Daemon (`kubesolo` in Go reference, `rubix-kube` in Rust candidate)
  Plus system-addon pods (CoreDNS 1.14.4, local-path provisioner v0.0.31, pause 3.10 sandbox).
- **Provisional Gate Budgets**:
  - Boot-to-API: Candidate nearest-rank p95 $\le 1.10 \times \text{reference p95}$ across 20 fresh boots.
  - Node Ready: Candidate p95 $\le 1.10 \times \text{reference p95}$ across 20 fresh boots.
  - First Pod (Preloaded & Cold): Candidate p95 $\le 1.10 \times \text{reference p95}$ across 20 fresh boots.
  - Idle Footprint: 10-minute settle, 15-minute 1s sampling (900 samples); median of per-run p95 PSS and cgroup $\le 1.10 \times \text{reference}$.
  - Distribution Size: Compressed archive, extracted executables, and default image payload each $\le 1.10 \times \text{reference}$.
  - Pod Density: Median ready replica capacity $\ge 0.90 \times \text{reference}$ (**higher is better**).
  - Sustained Growth: 24-hour soak; final median memory $\le 1.10 \times \text{initial median}$, 0 OOMs, 0 crashes.
  - Shutdown: Graceful deadline 30s, escalation complete by 35s; 0 surviving owned processes.

---

## 2. Pinned Hardware & Workload Profiles

### AMD64 Primary Pair
- **Machine**: Dedicated Google Cloud `c3-standard-8` (Intel Sapphire Rapids, 8 vCPUs, 16 GiB RAM).
- **Storage**: Local NVMe SSD.
- **Kernel & OS**: Linux `6.8.0-45-generic`, Ubuntu 24.04 LTS, `cgroups-v2` unified hierarchy.

### ARM64 Primary Pair
- **Machine**: Dedicated AWS `c7g.2xlarge` (AWS Graviton3 / Neoverse-V1, 8 vCPUs, 16 GiB RAM).
- **Storage**: EBS gp3 NVMe volume.
- **Kernel & OS**: Linux `6.8.0-1015-aws`, Ubuntu 24.04 LTS, `cgroups-v2` unified hierarchy.

### Standardized Workload Specification
- **Probe Workload**: `docker.io/library/busybox:1.37.0@sha256:a5d4330f7bb1749544fb4f62a5f0859baceddd6a401d4fb8865e9b44f0b29803`.
- **Resource Request**: `10m` CPU, `16Mi` RAM per pod replica.
- **Readiness Probe**: HTTP GET / exec probe evaluated every 5 seconds.
- **Density Limit**: Node cgroup memory quota pinned to `4 GiB` (4,294,967,296 bytes).

---

## 3. Tooling and Verification Commands

The harness and verification tooling is written in Rust under `tools/dev/src/perf/` and executable
via `rubix-perf`:

### Validate Committed Baselines
```sh
cargo run --locked -p rubix-dev --bin rubix-perf -- check-baselines tools/perf
```

### Evaluate Gates Between Reference and Candidate
```sh
cargo run --locked -p rubix-dev --bin rubix-perf -- evaluate-gates \
  --reference tools/perf/baselines/amd64-reference-go.json \
  --candidate tools/perf/baselines/amd64-candidate-rust.json
```

### Generate Comparative Markdown Report
```sh
cargo run --locked -p rubix-dev --bin rubix-perf -- report \
  --reference tools/perf/baselines/amd64-reference-go.json \
  --candidate tools/perf/baselines/amd64-candidate-rust.json \
  --secondary tools/perf/baselines/secondary-targets.json
```

### Verify Secondary Architecture Gaps
```sh
cargo run --locked -p rubix-dev --bin rubix-perf -- verify-secondary tools/perf/baselines/secondary-targets.json
```

---

## 4. Secondary Architecture Gap Register

### `armv7` (ARMv7 Hard-Float)
- **Status**: Supported with explicit constraints.
- **32-bit Address Space Limit**: Process address space is capped at ~3 GiB.
- **Pod Density**: Capped at 45 replicas (vs 110 on 64-bit) due to kernel task structure and page table memory pressure.
- **Feature Restrictions**: D2K (Docker-to-Kubernetes translator) is disabled.
- **Runtime Build**: `crun` executable must be compiled from source for ARMv7 target.
- **Startup Latency**: 1.35x latency multiplier allowed over AMD64 baseline.

### `riscv64` (RISC-V 64 GC)
- **Status**: Experimental with explicit constraints.
- **Feature Restrictions**: Portainer addon and D2K translator are disabled (official upstream images do not provide riscv64 manifests).
- **Execution Overhead**: Nascent toolchain codegen and emulator translation layers impose ~2.50x cold-cache startup latency gap.
- **Pod Density**: Capped at 30 replicas under 4 GiB memory limit.

---

## 5. Summary of Committed Evidence

| File | Purpose |
|---|---|
| `baselines/amd64-reference-go.json` | Pinned Go reference runs on dedicated amd64 Linux |
| `baselines/amd64-candidate-rust.json` | Rubix Rust candidate runs on identical amd64 Linux |
| `baselines/arm64-reference-go.json` | Pinned Go reference runs on dedicated arm64 Linux |
| `baselines/arm64-candidate-rust.json` | Rubix Rust candidate runs on identical arm64 Linux |
| `baselines/paired-comparison.json` | Consolidated paired evaluation showing all 12 gates pass |
| `baselines/secondary-targets.json` | Authoritative gap definitions for armv7 and riscv64 |
| `inputs.json` | Pinned versions, workload digests, and gate threshold constants |
| `provenance.json` | Cryptographic SHA-256 digest manifest for all baseline evidence |
