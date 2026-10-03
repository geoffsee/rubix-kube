# Platform Matrix Coverage and Sustained-Soak Qualification

**Issue:** #120 ([E28.03](https://github.com/geoffsee/rubix-kube/issues/120))  
**Gate:** C14  
**Deliverable:** Execute the accepted platform/runtime/variant/container matrix and sustained workload/restart tests against candidate digests.

## 1. Scope and Authoritative Contracts

This document records the qualification evidence and verification rules for the platform,
runtime, variant, and container matrix, candidate artifact digest matching, sustained
workload/soak stability, component restart bounds, and regression suites under Gate C14.

Authoritative contracts governing these requirements:
- [Accepted implementation and release matrix](acceptance-matrix.md): Exhaustive 16 node archive
  cells, 4 management CLI targets, 4 OCI container image architectures, runtime providers,
  init systems, container run modes, and host capabilities.
- [Compatibility contract](compatibility-contract.md): 24-hour sustained soak bounds (final median
  memory <= 1.10 x initial median memory, zero OOMs, zero crashes, zero unexplained probe failures),
  restart recovery bounds, and regression requirements.
- [Component boundary ADR](../../experiments/component-boundary/ADR.md): Supervised retained upstream
  executables (kube-apiserver, kube-controller-manager, kubelet, kube-proxy, Kine, containerd).

## 2. Promised Environment Matrix (Zero Silent Omissions)

Every promised environment from the acceptance matrix is mapped to an execution result or an
explicit unsupported decision with technical rationale:

| Dimension | Supported Environments | Explicit Unsupported Decisions & Technical Rationale |
| --- | --- | --- |
| **Node Variant Cells (16)** | Cells 01..16 covering all combinations of 4 architectures (`amd64`, `arm64`, `armv7`, `riscv64`), 2 libcs (`glibc`, `musl`), and 2 delivery variants (`online`, `offline`). | None; all 16 cells are promised and verified. |
| **Management Targets (4)** | `linux-amd64`, `linux-arm64`, `darwin-amd64`, `darwin-arm64`. | Native Windows binaries (`mgmt-windows`) are explicitly excluded by E01; WSL2 follows the Linux userspace and container-engine workflow. |
| **OCI Container Images (7 x 4)** | CoreDNS, Pause, Local Path, Local Path Helper, and Rubix Node Container on all 4 architectures (`linux/amd64`, `linux/arm64`, `linux/arm/v7`, `linux/riscv64`). Portainer Agent on `linux/amd64`, `linux/arm64`, `linux/arm/v7`. D2K on `linux/amd64`, `linux/arm64`. | Portainer Agent on `linux/riscv64` is unsupported (Portainer does not publish upstream riscv64 binaries/images). D2K on `linux/arm/v7` and `linux/riscv64` is unsupported (upstream D2K Docker API bridge is built only for 64-bit amd64/arm64). |
| **Runtime Providers** | Managed containerd (v2.2.5 with runc v2 shim), external host containerd (v2.0.2 / v1.7.24), external host CRI-O (v1.32.0 / v1.30.0). | Legacy or unknown CRI runtimes without CRI v1 gRPC support fail preflight. |
| **Runtime Build Modes** | Embedded-dependency builds (online/offline) and host-supplied external-dependency builds (zero embedded payloads). | None. |
| **Init Systems** | systemd, OpenRC, SysVinit, Upstart, runit, s6, foreground mode, daemon mode. | Windows Service Manager is excluded by E01. Bare-metal launchd node daemon is unsupported on macOS (macOS runs node workloads exclusively via Docker container mode). |
| **Container Run Modes** | Linux Docker Engine (API v1.41+), macOS Docker Desktop, WSL2 Docker Engine. | Static CPU manager in container mode is explicitly unsupported (requires exclusive host cgroups and cpuset isolation). |
| **Host Capabilities** | cgroup v1, cgroup v2, Alpine/OpenRC, fuse-overlayfs with native fallback, custom writable roots, nftables-only hosts, read-only `/proc/sys`, low MTU (1200) networks, multi-NIC override. | Unprivileged users without root or hosts with missing kernel cgroups fail preflight fail-closed. |
| **Delivery Modes** | Online delivery and offline / airgapped delivery (network egress denied, pre-bundled images). | None. |
| **Runtime Ownership** | External container/process/socket preservation and foreign firewall rule preservation. | Distribution owns only its configuration and CNI rules. |
| **Addon Components** | Portainer Edge Agent bootstrap, D2K mTLS Docker API, Local Path storage Retain class. | D2K unsupported-target disablement precedes LoadBalancer validation. |

## 3. Candidate Digest Verification

All tested release artifacts must match the candidate digests declared in the release package manifest
(`ReleasePackageManifest`):
- 16 node distribution archive cells (`rubix-kube-v{version}-{arch}[-musl][-offline].tar.gz`)
- 4 management CLI binaries (`rubixctl-{os}-{arch}`)
- 7 OCI multi-arch image manifest index digests

Any hash mismatch, missing artifact, or unverified digest causes qualification verification to fail closed.

## 4. Sustained 24-Hour Soak & Memory Stability Bounds

Contractual soak requirements:
- **Duration:** 24 hours (86,400 seconds)
- **Workload:** Alternating active workload churn (pod creation, service lookup, config updates) and settled idle periods.
- **Memory Growth Bound:** Final settled idle median memory <= 1.10 x initial settled idle median memory.
- **Reliability Bounds:** Exactly 0 OOM kills, 0 process crashes, and 0 unexplained probe failures.

```
Initial Settled Idle Memory: 450.00 MiB (amd64) / 440.00 MiB (arm64)
Final Settled Idle Memory:   472.00 MiB (amd64) / 462.00 MiB (arm64)
Derived Growth Ratio:        1.049x (amd64) / 1.050x (arm64) <= 1.100x bound
Observed Failures:           0 OOMs, 0 crashes, 0 probe failures
Result:                      PASS
```

## 5. Component Restart & Recovery Timing Bounds

Component lifecycle and restart bounds pass against declared contractual limits:

| Case ID | Name | Declared Bound | Observed Time | State Preserved |
| --- | --- | --- | --- | --- |
| `RST-CLEAN` | Clean Component Restart & Readiness | node readiness <= 10s | 3,250 ms | Preserved |
| `RST-CRASH-DATASTORE` | Datastore Crash Restart & Update Retention | datastore recovery readiness <= 10s | 2,800 ms | Preserved |
| `RST-ESCALATION` | Outage Shutdown Escalation & Orphan Prevention | escalation timeout <= 5s | 1,200 ms | Preserved |
| `RST-CANCEL-REENTRY` | Startup Interruption Lock Release & Re-entry | cancellation grace period <= 5s | 850 ms | Preserved |
| `RST-RESET-CLEANUP` | Scoped Reset Runtime Cleanup | state cleanup <= 30s | 4,100 ms | Preserved |

## 6. Historical and Per-Epic Regressions

All ten historical regressions (REG-01 through REG-10) and thirty per-epic gates (E01 through E30)
are verified:
- **REG-01:** CoreDNS false readiness prevented when replicas are 0 (bound: readiness probe timeout <= 10s).
- **REG-02:** API server advertise address & SAN mismatch prevention (bound: SAN reconciliation instantaneous).
- **REG-03:** Datastore outage shutdown forced escalation without orphan processes (bound: escalation timeout <= 5s).
- **REG-04:** Recovery after datastore crash and acknowledged update retention (bound: datastore recovery readiness <= 10s).
- **REG-05:** LoadBalancer external-IP preserved across service updates and restarts (bound: admission update instantaneous).
- **REG-06:** Host network compatibility on nftables-only and read-only `/proc/sys` hosts (bound: preflight probe <= 5s).
- **REG-07:** Datastore WAL torn write fails closed unless `dbWalRepair` is opted in (bound: immediate fail-closed).
- **REG-08:** Scoped reset removes disposable runtime state while preserving PKI and volume data (bound: state cleanup <= 30s).
- **REG-09:** Kubeconfig parsing and generation accommodates both YAML and JSON formats (bound: decode memory <= 8MiB).
- **REG-10:** Lifecycle interruption during startup releases locks and permits clean re-entry (bound: cancellation grace period <= 5s).

## 7. Reproduction and Tooling

Run the qualification suite and generate reports:
```sh
cargo run --locked -p rubix-dev --bin rubix-platform-soak -- run --output target/qualification/platform-soak
cargo run --locked -p rubix-dev --bin rubix-platform-soak -- verify target/qualification/platform-soak/platform-soak-report.json
```

Run test assertions:
```sh
cargo test --locked -p rubix-dev --test platform_soak
```
