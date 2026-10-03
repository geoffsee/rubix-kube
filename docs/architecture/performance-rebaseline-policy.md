# Performance Gating and Rebaseline Policy

This document defines the authoritative performance gating contracts, sustained memory
growth constraints, and rebaseline policy for the Rubix Kubernetes distribution under
Gate C14 / Work Item E29.03.

## Purpose and Scope

Rubix preserves compatibility with KubeSolo baseline behavior while maintaining a single-node
distribution footprint. Performance gating ensures that Rust architectural replacements
and upstream component integration do not introduce silent regressions in startup latency,
idle memory footprint, pod density, lifecycle responsiveness, or sustained soak stability.

All performance evaluations are governed by fail-closed gating enforced in CI via:

```sh
cargo run --locked -p rubix-dev --bin rubix-perf -- gate-ci tools/perf
```

## Contract Multipliers and Thresholds

Performance gating compares a candidate distribution capture against an authoritative baseline
reference under identical architecture, hardware configuration, and workload conditions.
The 12 committed contract gates and their maximum permissible thresholds are:

| Metric | Target / Gate | Maximum Permissible Multiplier | Directionality |
| --- | --- | --- | --- |
| Startup Latency: Boot to API (`p95`) | `boot_to_api_p95` | reference * 1.10 (+10% max) | Lower is better |
| Startup Latency: Node Ready (`p95`) | `node_ready_p95` | reference * 1.10 (+10% max) | Lower is better |
| Startup Latency: First Pod Scheduled (`p95`) | `first_pod_scheduled_p95` | reference * 1.10 (+10% max) | Lower is better |
| Startup Latency: System Workloads Ready (`p95`) | `system_workloads_ready_p95` | reference * 1.10 (+10% max) | Lower is better |
| Idle Footprint: Total Distribution (`p95`) | `total_idle_memory_p95` | reference * 1.05 (+5% max) | Lower is better |
| Idle Footprint: kube-apiserver (`p95`) | `apiserver_idle_memory_p95` | reference * 1.05 (+5% max) | Lower is better |
| Idle Footprint: Kine (`p95`) | `kine_idle_memory_p95` | reference * 1.05 (+5% max) | Lower is better |
| Idle Footprint: containerd (`p95`) | `containerd_idle_memory_p95` | reference * 1.05 (+5% max) | Lower is better |
| Idle Footprint: kubelet (`p95`) | `kubelet_idle_memory_p95` | reference * 1.05 (+5% max) | Lower is better |
| Pod Density Capacity | `pod_density_capacity` | reference * 0.95 (-5% max) | **Higher is better** |
| Lifecycle: Single Pod Cycle (`p95`) | `single_pod_cycle_p95` | reference * 1.10 (+10% max) | Lower is better |
| Lifecycle: Burst Pod Cycle (`p95`) | `burst_pod_cycle_p95` | reference * 1.15 (+15% max) | Lower is better |

### Pod Density Directionality

Pod density capacity measures maximum schedulable and runnable pods on the single node. Unlike
latency and memory metrics, pod density is **higher-is-better**. A candidate passes only if:

$$\text{candidate\_density} \ge \text{reference\_density} \times 0.95$$

Any candidate with density below 95% of reference is rejected as a capacity regression.

## Sustained Memory Growth and Soak Stability

A distribution node must remain stable under repeated workload deploy, churn, and idle cycles.
Long-running performance validation requires:

1. **Declared Duration**: A 24-hour minimum soak interval composed of repeated workload churn
   and idle phases.
2. **Distribution-Wide Process Accounting**: All 8 canonical retained process roles must be
   individually measured:
   - `kube_apiserver`
   - `kine`
   - `containerd`
   - `containerd_shim`
   - `kubelet`
   - `kube_controller_manager`
   - `kube_proxy`
   - `rubix_engine`
3. **Bounded Memory Ratio**: Final PSS / Initial PSS across the 24-hour cycle must satisfy:
   $$\frac{\text{Final PSS}}{\text{Initial PSS}} \le 1.10$$
   Growth exceeding 10% over 24 hours indicates a leak in runtime shims, Kubernetes components,
   or supervisor daemons and fails gating.
4. **Zero-Defect Reliability**:
   - `oom_kills == 0`
   - `process_crashes == 0`
   - `unexpected_failures == 0`

## Truthfulness and Anti-Misrepresentation Rules

Reports and baselines must honestly reflect measured numbers:

- **No Sub-200MB Unmeasured Footprint Claims**: Claims of sub-200MB distribution footprint are
  categorically rejected if retained supervised processes alone sum to $\ge 200\text{ MB}$.
- **Physical Sanity Bounds**:
  - Sampled PSS and RSS must be strictly positive ($> 0$).
  - Sampled PSS must never exceed RSS ($\text{PSS} \le \text{RSS}$).
  - All latency samples (boot, node ready, scheduling, cycles) must be non-negative.
- **Fail-Closed Fixture Policy**: Synthetic fixtures demonstrate arithmetic correctness but
  fail closed against release qualification until live captures on dedicated Linux hardware
  are authenticated and ingested.

## Rebaseline Policy

Baselines may only be re-established when intentional architecture, upstream version upgrades,
or compiler optimizations change performance characteristics. The following rules govern
any baseline modification:

1. **Paired Architecture Updates**: Reference baselines for `linux/amd64` and `linux/arm64` must
   be captured and updated together. Gaps or one-sided updates are rejected.
2. **Threshold Immutability**: The contract multipliers (1.10, 1.05, 0.95, 1.15) and sustained
   growth bounds ($\le 1.10$) defined in `ContractThresholds` are immutable contracts. They
   cannot be relaxed to accommodate regressed performance.
3. **Full Retained Process Representation**: Baselines may not omit any of the 8 canonical
   retained process roles or substitute placeholder zero values.
4. **Provenance Integrity**: All updated JSON fixture and baseline files must have their SHA-256
   hashes recorded in `provenance.json`. Tampered or desynchronized files cause immediate CI failure.
5. **Secondary Target Honesty**: Secondary architecture limitations (`secondary-targets.json`)
   must be explicitly recorded as gaps; they cannot be used to waive primary tier requirements.

## CI Enforcement

CI enforces this policy on every pull request and push to `main` via:

```sh
cargo run --locked -p rubix-dev --bin rubix-perf -- gate-ci tools/perf
```

The command verifies:
- Provenance digest matching for all performance files.
- Contract threshold compliance and non-relaxation.
- Retained process coverage across primary architectures.
- Arithmetic evaluation of all 12 contract gates for both amd64 and arm64.
- Directional correctness of pod density checks.
- 24-hour sustained memory growth bounds and zero failure counts.
- Secondary target gap records.
