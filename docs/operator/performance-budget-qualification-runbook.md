# Performance budget qualification runbook (Criterion 7)

This runbook defines the operational protocol for qualifying Roadmap #263 **Criterion 7:
Performance & Memory Budgets** ("Matched live amd64/arm64 budgets, 24-hour settled memory
and shutdown").

Read the [acceptance matrix](../architecture/acceptance-matrix.md),
[compatibility contract](../architecture/compatibility-contract.md),
[budget profiling analysis](../architecture/budget-profiling-analysis.md),
[component boundary ADR](../../experiments/component-boundary/ADR.md), and
[live integration guide](../../tools/integration/README.md) before executing qualification.

---

## 1. Operational scope and integrity policy

Passing unit tests, fixture assertions, or local macOS benchmarks **do not** qualify
production performance budgets. Criterion 7 requires candidate-bound receipts captured on
disposable Linux hosts running candidate release binaries.

### Core integrity rules
1. **Never fabricate or hand-edit receipts**: Receipts must be emitted by live execution of the
   qualification runner on Linux infrastructure. Synthetic or hand-crafted files committed to
   `docs/release/receipts/` invalidate release qualification.
2. **Fail closed on default checkout**: Without authentic live Linux receipts matching candidate
   digests, Criterion 7 evaluates to `Pending`. The release qualification gate fails closed.
3. **No unbacked marketing claims**: Sub-200 MB idle memory or under-60 second startup claims are
   strictly prohibited unless supported by verified, continuous live measurements.
4. **All 8 retained processes must be measured**: Whole-node idle memory accounting must measure
   every retained process in the supervised component boundary. Omitting background daemons or shims
   is prohibited.
5. **Secondary architectures and hardware gaps**: If an architecture (e.g. matched amd64) is unavailable
   during an arm64 qualification run, it constitutes an explicit **hardware gap**. It must be
   documented in the candidate receipt's `skips` ledger with technical justification. Emulation or
   single-architecture runs cannot qualify an unmeasured architecture.

---

## 2. Infrastructure prerequisites and matched host baseline

Performance comparisons require matched hardware configurations between arm64 and amd64 to
eliminate thermal, virtualization, and noisy-neighbor variance.

### Target host specification

| Specification | Primary arm64 target | Primary amd64 target |
| --- | --- | --- |
| **Instance class** | AWS Graviton3 `c7g.2xlarge` (or bare metal) | AWS `c6i.2xlarge` (or bare metal) |
| **Compute** | 8 vCPU | 8 vCPU |
| **Memory** | 16 GiB ECC RAM | 16 GiB ECC RAM |
| **Storage** | Dedicated NVMe SSD (`ext4`) | Dedicated NVMe SSD (`ext4`) |
| **OS / Distribution** | Ubuntu 24.04 LTS (kernel >= 6.6.x) | Ubuntu 24.04 LTS (kernel >= 6.6.x) |
| **cgroups mode** | cgroup v2 (`systemd.unified_cgroup_hierarchy=1`) | cgroup v2 (`systemd.unified_cgroup_hierarchy=1`) |
| **Isolation** | Dedicated disposable runner; no co-located jobs | Dedicated disposable runner; no co-located jobs |

### Host preflight verification

Execute preflight checks on the disposable Linux runner prior to benchmarking:

```bash
# 1. Verify kernel version and architecture
uname -s -r -m

# 2. Verify cgroup v2 hierarchy
mount | grep cgroup2
[ -d /sys/fs/cgroup ] && echo "cgroup v2 present"

# 3. Confirm available memory and storage headroom
free -h
df -h /var/lib /tmp

# 4. Confirm clean host state (no stray containers or runtimes)
pgrep -l "containerd|kubelet|rubix|kine" || echo "Host clean"
```

---

## 3. Retained process boundary and memory measurement

Under the Option B component boundary ([ADR](../../experiments/component-boundary/ADR.md)),
Rubix runs an in-process control plane while supervising external container execution processes.
Whole-node memory accounting requires measuring all **8 canonical process roles**:

| Canonical process role | Binary / Process name | Accounting requirement |
| --- | --- | --- |
| `apiserver` | `kube-apiserver` | In-process or supervised API server PSS/RSS |
| `controller-manager` | `kube-controller-manager` | Supervisor controller loop PSS/RSS |
| `kubelet` | `kubelet` | Node kubelet daemon PSS/RSS |
| `proxy` | `kube-proxy` | Host network routing proxy PSS/RSS |
| `kine` | `kine` / `rubix-datastore` | In-process MVCC/WAL datastore engine PSS/RSS |
| `containerd` | `containerd` | Managed container daemon PSS/RSS |
| `containerd-shim` | `containerd-shim-runc-v2` | Active workload runtime shims PSS/RSS |
| `node-daemon` | `rubix-kube` | Node supervisor and runtime adapter PSS/RSS |

### Linux memory measurement protocol

Memory accounting relies on Linux kernel interfaces:
- **PSS (Proportional Set Size)**: Sampled from `/proc/<pid>/smaps_rollup` (or `/proc/<pid>/smaps`).
  Shared libraries are proportionally divided among sharing processes.
- **RSS (Resident Set Size)**: Sampled from `/proc/<pid>/status` (`VmRSS`).
- **cgroup v2 memory**: Sampled from `/sys/fs/cgroup/rubix.slice/memory.current` and `memory.stat`.

---

## 4. Benchmark execution and contract budgets

The live benchmark executes the protocol defined in `tools/perf` and `rubix-dev`:

### 1. Idle settled footprint (10m settle + 15m sample)
- Allow node to reach steady state for **10 minutes** (600 seconds) after readiness.
- Sample idle PSS every second for **15 minutes** (900 continuous samples).
- Budget: candidate summed PSS <= reference median PSS * 1.10.
- Strict limit: sub-200 MB claims require backing retained process sums.

### 2. Startup latency (20 iterations)
- Measure 20 cold boot-to-API and node-ready cycles.
- Alternating order (`ReferenceFirst` / `CandidateFirst`) across iterations to prevent thermal bias.
- Budget: boot-to-API p95 <= 30.0s; node-ready p95 <= 45.0s.

### 3. Workload pod density (600s hold)
- Deploy density workload up to node memory limit.
- Hold for at least 600 seconds with 100% probe success rate.
- Budget: candidate ready replicas >= reference replicas * 0.90.

### 4. 24-hour sustained soak
- Run steady-state baseline for **24 continuous hours**.
- Sample memory hourly.
- Budget: final memory <= initial memory * 1.05 (growth ratio <= 1.05x).
- Stability gate: exactly 0 OOM kills, 0 process crashes, 0 unexplained failures.

### 5. Clean shutdown and process cleanup
- Trigger graceful node termination with a 30-second budget.
- Measure graceful duration and escalation deadline.
- Budget: graceful shutdown p95 <= 30.0s, max <= 30.0s; escalation p95 <= 10.0s, max <= 10.0s.
- Cleanup gate: exactly 0 surviving owned processes, 0 unrelated host processes killed.

---

## 5. Candidate receipt generation procedure

Once live qualification completes on the disposable Linux runner, generate the candidate-bound
receipt for Criterion 7:

### Step 1: Capture Candidate Identity and Environment

Extract the candidate commit and artifact digests from the built candidate bundle:

```bash
SOURCE_REV=$(git rev-parse HEAD)
KUBE_SHA=$(sha256sum target/release/rubix-kube | awk '{print $1}')
BUNDLE_SHA=$(sha256sum target/release/bundle.manifest | awk '{print $1}')
KERNEL_VER=$(uname -r)
RUNNER_ID="aws-c7g.2xlarge-disposable-$(date +%Y%m%d)"
```

### Step 2: Assemble Receipt Payload

Create the canonical JSON receipt payload containing:
- `schema_version`: 1
- `criterion`: 7
- `description`: "Live performance and memory budget qualification on matched hardware"
- `candidate`: candidate source revision and binary/payload digests matching `cell-inventory.json`.
- `environment`: host (`linux-arm64`), kernel (`6.6.x`), runner identifier.
- `commands`: commands executed with exit code 0, durations, and output digests.
- `assertions`: all 8 canonical process roles and contract gate verifications.
- `skips`: hardware gaps (e.g. `amd64_live_capture`) with mandatory technical reasons.
- `cleanup`: confirmed `status: "complete"`, with empty `remaining_containers` and `remaining_images`.
- `timestamps`: RFC 3339 execution window (`started_at`, `completed_at`).

### Step 3: Compute Canonical Integrity Hash

Sign the payload using SHA-256 over its canonical serialization:

```bash
cargo run --locked -p rubix-dev --bin rubix-release -- sign-receipt \
  --input /tmp/criterion-07-payload.json \
  --output docs/release/receipts/criterion-07-performance-budgets.json
```

---

## 6. Verification and fail-closed validation

After generating the receipt, verify repository qualification integrity:

```bash
# 1. Run receipt schema validation
cargo test --locked -p rubix-dev --test suite receipt_schema_validation

# 2. Run full release qualification audit
cargo run --locked -p rubix-dev --bin rubix-release -- verify-fixtures docs/release

# 3. Verify perf CI regression gates and rebaseline policy
cargo run --locked -p rubix-dev --bin rubix-perf -- gate-ci tools/perf
cargo run --locked -p rubix-dev --bin rubix-perf -- check-rebaseline-policy tools/perf
```

If any check fails, the receipt is unqualified and Criterion 7 remains `Pending`.
Investigate the failure, resolve performance regressions or accounting gaps, and re-run
qualification on clean disposable infrastructure.
