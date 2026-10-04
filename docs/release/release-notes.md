# Rubix Kubernetes Distribution v0.1.0 Release Notes

**Release Version**: `0.1.0`  
**Gate Status**: Gate C16/C17 Production Readiness  
**Release Date**: `2026-10-03`

Rubix is a production single-node Kubernetes distribution supervising retained official upstream executables (`kube-apiserver`, `kube-controller-manager`, `kubelet`, `kube-proxy`, `kine`, `containerd`, and CNI plugins) wrapped in a high-reliability, memory-safe Rust supervisor, PKI manager, host preflight engine, and management CLI.

## 1. Version Transition Support (Go to Rust Migration)

Rubix v0.1.0 provides qualified migration support from historical KubeSolo versions `v1.1.8`, `v1.2.0`, `v1.3.0`, and `v1.3.1` through `v1.3.3`. All migrations require a planned maintenance downtime window of **5 to 10 minutes**.

| Starting Version | Native YAML Config | Flag Migration Required | Starting Layout & Architecture |
|---|---|---|---|
| `v1.1.8` | `false` | `true` | Legacy flags in service unit, persistent PKI, Kine SQLite datastore |
| `v1.2.0` | `false` | `true` | Legacy flags in service unit, persistent CA/PKI, external runtime support |
| `v1.3.0` | `true` | `false` | YAML config file, persistent PKI with IP auto-regeneration, Kine SQLite |
| `v1.3.1` | `true` | `false` | YAML config file, persistent PKI, external runtime & CRI support, Kine SQLite |
| `v1.3.2` | `true` | `false` | YAML config file, persistent PKI, external runtime & CRI support, Kine SQLite |
| `v1.3.3` | `true` | `false` | YAML config file, persistent PKI, external runtime & CRI support, Kine SQLite |

### Migration Prerequisites and Mandatory Backups

Before executing a transition from any prior version, operators MUST create verified pre-transition backups of:
1. **Configuration**: `/etc/kubesolo/config.yaml` (or service flags file)
2. **PKI Trust Roots**: `/var/lib/kubesolo/pki/` (CA certificate, private key, and service-account signing keys)
3. **Datastore**: `/var/lib/kubesolo/kine/db/state.db` and active WAL files (with running node processes stopped)
4. **Persistent Volumes**: `/var/lib/kubesolo/local-path-storage/` (workload application data)

### Datastore Non-Interchangeability and Transport Boundary

- **Kine SQLite Preservation**: Kine's SQLite database file format (`state.db`) is preserved. Replacing SQLite with experimental native snapshot formats (such as `RUBXSNP1`) is **not** a production migration path; raw SQLite cannot be adopted into snapshots without explicit offline rehearsal.
- **Loopback mTLS Transport**: Production API-server-to-Kine communication strictly enforces loopback mutual TLS over `https://127.0.0.1:2379` using a dedicated datastore CA and client certificate. Plaintext datastore transport is prohibited.

### Rejected Unsupported Versions

The following versions are outside the supported migration catalog and will be rejected with an actionable error:
- `v1.0.0 (below minimum supported migration version v1.1.8)`
- `v0.9.1 (below minimum supported migration version v1.1.8)`
- `v1.1.7 (below minimum supported migration version v1.1.8)`
- `unversioned or development builds (empty version, develop, git commit shas)`

## 2. Breaking Changes and Deliberate Deviations (D01 – D11)

Rubix maintains high behavioral fidelity to upstream baseline KubeSolo while resolving historical ambiguities, security defects, and unverified assumptions. Below are the eleven deliberate architectural deviations:

### D01 — Target and Bundle Integrity Across Matrix

**Description**: Preserve all 16 node target cells (including ARMv7 hard-float and RISC-V 64 glibc/musl) instead of silently dropping unsupported targets. Reject mismatched binaries or architectures at installation.

**Operational Impact**: Universal target coverage; strict installer error reporting on cross-architecture downloads.

### D02 — Explicit Versioning and Capability Manifests

**Description**: Rubix candidates declare explicit version and capability manifests across CLI, installer, and bundle. Historical Go release version comparisons are rejected.

**Operational Impact**: Eliminates silent version mismatch; unsupported capabilities fail closed before service replacement.

### D03 — Component Boundary and Process Supervision Transparency

**Description**: Rubix is a Rust-supervised distribution managing retained official upstream executables (kube-apiserver, kube-controller-manager, kubelet, kube-proxy, Kine, containerd, CNI). It is not a native Rust reimplementation of Kubernetes or container engines.

**Operational Impact**: Full process tree and memory accountability; no false native-Rust claims.

### D04 — Local Storage Enabled Defaults

**Description**: Omitted local-storage configuration defaults to true (enabled), aligning with runtime Defaults(). An explicit false input is required to disable storage.

**Operational Impact**: Default deployments automatically include local-path persistent storage.

### D05 — Strict Offline Bundle Target Validation

**Description**: Offline bundles enforce machine architecture, operating system, and libc matching before extraction, rejecting cross-target copying.

**Operational Impact**: Prevents corrupted or unusable partial installations on foreign hosts.

### D06 — Immutable Digests Over Mutable Tags

**Description**: Eliminates ambiguous 'latest' release and container tags in favor of explicit version tags and verified SHA-256 image digests.

**Operational Impact**: Deterministic deployments immune to registry tag mutability.

### D07 — Strict D2K Mutual TLS Authentication and Endpoint Readiness

**Description**: The optional Docker-compatible D2K endpoint requires client certificate authentication over TLS on port 2376. Unauthenticated and wrong-client requests are strictly rejected; readiness is gated on live probe response.

**Operational Impact**: Protects Docker API endpoint against unauthorized host or container access.

### D08 — Truthful Native Diagnostics Without Fabricated Metrics

**Description**: Rust runtime diagnostics replace Go-specific pprof/runtime internals. Prometheus metrics reflect honest probe semantics without fabricating Go runtime time-series.

**Operational Impact**: Accurate observability without misleading runtime gauges.

### D09 — Preserved Upstream Controller Defaults and Deprecated --full No-Op

**Description**: Preserves official upstream Kubernetes v1.35.7 controller defaults without restoring deprecated low-memory overrides. The --full flag is treated as a deprecated no-op.

**Operational Impact**: Production Kubernetes controller parity and predictable memory allocation.

### D10 — Atomic Stored Desired Configuration

**Description**: Configuration API operates on stored desired configuration over a private 0600 Unix domain socket; runtime overrides are not persisted and edits require node restart.

**Operational Impact**: Prevents configuration drift and race conditions during runtime operation.

### D11 — Rehearsed Datastore Adoption and Rollback Safety

**Description**: No automatic SQLite-to-snapshot format interchangeability. Kine SQLite state at kine/db/state.db is preserved and requires dedicated loopback mTLS.

**Operational Impact**: Guarantees zero silent datastore corruption during Go-to-Rust transitions.

## 3. Deprecations

- Flag `--full` and environment variable `KUBESOLO_FULL` are deprecated and treated as no-ops.
- Legacy command-line flags are deprecated in favor of declarative `kubesolo.io/v1alpha1` YAML configuration.
- Native Windows binaries are excluded per ADR E01; WSL2 is supported through standard Linux userspace and container engines.

## 4. Performance Budget Baselines and Sustained Soak Constraints

Rubix enforces 12 committed performance contract thresholds against paired reference baselines across `linux/amd64` and `linux/arm64`. Candidates are validated under identical hardware, cgroups v2, and kernel environments:

| Metric | Contract Gate | Direction | Description |
|---|---|---|---|
| `Boot-to-API Latency (p95)` | `<= 1.10x reference` | Lower is better | Time from node daemon launch to authenticated API read/write readiness |
| `Node Ready Latency (p95)` | `<= 1.10x reference` | Lower is better | Time from launch until node reports Ready condition to API server |
| `First Pod Latency (Preloaded & Cold, p95)` | `<= 1.10x reference` | Lower is better | Time to schedule and run a probe pod with preloaded and cold image paths |
| `Total Distribution Idle Memory (p95)` | `<= 1.05x reference` | Lower is better | Whole-distribution settled idle memory footprint (PSS and cgroup) |
| `Component Idle Memory (apiserver, kine, containerd, kubelet)` | `<= 1.05x reference` | Lower is better | Individual PSS memory bounds across core supervised processes |
| `Pod Density Capacity` | `>= 0.90x reference` | Higher is better | Maximum schedulable and runnable pod replicas maintaining passing probes |
| `Lifecycle Pod Cycles (Single & Burst, p95)` | `<= 1.10x / 1.15x reference` | Lower is better | Pod creation, scheduling, execution, and teardown cycle latency |

### 24-Hour Sustained Memory Growth and Reliability

- **Maximum 24h Memory Soak Growth Ratio**: `<= 1.10x` (Final settled PSS / Initial settled PSS)
- **Zero-Defect Reliability**: `0` OOM kills, `0` process crashes, `0` unexpected probe failures
- **Distribution-Wide Process Accounting**: All 8 canonical retained process roles are independently tracked:
  * `kube_apiserver`
  * `kine`
  * `containerd`
  * `containerd_shim`
  * `kubelet`
  * `kube_controller_manager`
  * `kube_proxy`
  * `rubix_engine`
- **Anti-Misrepresentation Rules**: Unmeasured sub-200MB claims are categorically rejected. Physical sanity constraints require PSS > 0, RSS > 0, and PSS <= RSS across all samples.
- **Secondary Target Gaps**: ARMv7 hard-float and RISC-V 64 limitations are declared explicitly as qualification gaps.

## 5. Certified Single-Node Capabilities

Rubix v0.1.0 provides qualified single-node Kubernetes capabilities including:
- NodeSetter admission webhook mutating unassigned Pods, PVCs, and Jobs to single-node placement without a cluster scheduler
- Automatic LoadBalancer status assignment mapping Service ingress to the active node IP
- In-cluster CoreDNS TCP and UDP resolution with forwarder isolation
- HostPath local-path storage provisioner enforcing default Retain reclaim semantics
- Optional Portainer Edge Agent reverse-tunnel bootstrap without mutating existing Portainer configurations
- Optional D2K Docker-to-Kubernetes API gateway exposing Docker Engine API over mutual TLS
- Dual-format kubeconfig accommodation supporting both YAML and JSON parsing identically

## 6. Mandatory Multi-Node Non-Certification Disclaimer

> [!IMPORTANT]
> Synthetic in-process fixtures only. C13/E28 and the selected upstream conformance suite remain unqualified. DO NOT claim official CNCF Certified Kubernetes qualification or multi-node certification.

Rubix is explicitly architected and qualified **only** as a single-node Kubernetes distribution. It does NOT implement or support multi-node clustering, distributed scheduling, high availability control plane replication, or etcd cluster consensus. Upstream Kubernetes conformance tests that exercise multi-node, serial slow, disruptive, or flaky scenarios are explicitly excluded from qualification scope by architectural design.

## 7. Dual Kubeconfig Format Accommodation

Rubix management tooling (`rubixctl`) and verification suites accommodate both **YAML** and **JSON** kubeconfig formats interchangeably, verifying cryptographic certificate validation, client authentication, and context merging across both representations.
