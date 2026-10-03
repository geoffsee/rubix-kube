# Performance Gating and Rebaseline Policy

This policy implements the provisional engineering gates in the
[compatibility contract](compatibility-contract.md#measurable-engineering-gates)
for C13/E29. The compatibility contract owns performance thresholds and sampling.

## Scope and evidence boundary

CI checks committed **synthetic fixture integrity and arithmetic**, not live
node performance qualification:

```sh
cargo run --locked -p rubix-dev --bin rubix-perf -- gate-ci tools/perf
```

Passing this command demonstrates that the fixture data is consistent with the
committed thresholds. It does not establish actual Linux measurements, bounded
live memory growth, release qualification, or a performance improvement. All
qualification paths still fail closed for synthetic or unverified reports.
A verified capture/provenance importer and real matched Linux captures remain
required before C13/E29 can be qualified.

## Implemented gates and default thresholds

The twelve arithmetic gates compare matched reference/candidate reports. The
sampling contract requires twenty fresh boot/shutdown runs, five idle boots
with ten-minute settling and 900 one-second samples per boot, and five density
runs with ten minutes of successful probes. Idle inputs are the per-run p95
values; the gate compares their median.

| Gate | Default pass condition |
| --- | --- |
| Boot-to-API latency, p95 | Candidate ≤ reference × 1.10 |
| Node Ready latency, p95 | Candidate ≤ reference × 1.10 |
| First Pod latency, preloaded image, p95 | Candidate ≤ reference × 1.10 |
| First Pod latency, cold image, p95 | Candidate ≤ reference × 1.10 |
| Idle summed PSS, median of run p95 values | Candidate ≤ reference × 1.10 |
| Idle cgroup memory, median of run p95 values | Candidate ≤ reference × 1.10 |
| Compressed distribution archive bytes | Candidate ≤ reference × 1.10 |
| Extracted executable/helper bytes | Candidate ≤ reference × 1.10 |
| Default image payload bytes | Candidate ≤ reference × 1.10 |
| Pod density, median Ready replicas | Candidate ≥ reference × 0.90 |
| Sustained memory growth | Final / initial settled idle median ≤ 1.10; duration ≥ 24 hours; zero OOM kills, crashes and unexplained failures |
| Shutdown and process cleanup | Graceful p95 and maximum ≤ 30 seconds; escalation p95 and maximum ≤ 35 seconds; zero surviving owned processes and unrelated processes killed |

System-workload readiness, per-component memory and single/burst pod-cycle
latencies are not additional implemented gates. Distribution accounting must
still include retained executables, shims and default addon workloads; those
costs cannot be excluded to make a candidate appear smaller.

### Pod density directionality

Pod density is higher-is-better. Candidate median capacity below 90% of the
matched reference fails. Lower-is-better memory, latency and size gates permit
at most a 10% increase under default thresholds.

## Sustained growth and process accounting

The soak contract alternates fixed workload and idle phases for at least 24
hours and compares the first and final settled idle-hour medians. CI arithmetic
uses the byte-derived final/initial ratio. A stored ratio must agree within its
existing three-decimal rounding precision; it cannot choose the gate outcome.
Zero initial memory and observed OOMs, crashes or unexplained failures fail.

Canonical process roles are `apiserver`, `controller-manager`, `kubelet`,
`proxy`, `kine`, `containerd`, `containerd-shim` and `node-daemon`. The daemon and
shim must be accounted for independently. Retained process PSS/RSS must be
positive, PSS must not exceed RSS, and a claimed sub-200-MB idle footprint
cannot contradict the retained-process accounting. These structural checks
validate fixture consistency; they do not attest actual measurements.

## Rebaseline policy

A hardware, workload, enabled component, variant or other environment change
requires new matched Go and Rust measurements and a reviewed baseline update.
Do not silently change the selected component boundary or omit owned/runtime
costs. Preserve raw samples, failures, commands, hashes, timestamps and logs;
timeouts are failures rather than discarded samples. Alternate Go/Rust run order.

1. Update matched amd64 and arm64 reference/candidate pairs together.
2. `ContractThresholds` rejects relaxation: lower-is-better multipliers are at
   most 1.10; the density multiplier is at least 0.90; growth is at most 1.10;
   shutdown deadlines are at most 30 and 35 seconds. Stricter thresholds are
   allowed, and all values must be finite and positive.
3. Retain every required process role and the whole enabled distribution payload.
4. `provenance.json.files` must contain **exactly** the following seven paths:
   `inputs.json`, `fixtures/amd64-reference-go.json`,
   `fixtures/amd64-candidate-rust.json`, `fixtures/arm64-reference-go.json`,
   `fixtures/arm64-candidate-rust.json`, `fixtures/paired-comparison.json` and
   `fixtures/secondary-targets.json`. Every SHA-256 digest must match. Missing,
   duplicate, unexpected or noncanonical paths fail before evaluation. The
   evaluator parses the same bounded bytes whose digests it checked. These
   hashes establish fixture integrity, not independently trusted live provenance.
5. Secondary targets remain explicit unqualified gaps; they cannot waive
   primary-tier requirements or invent measured capacity/latency exceptions.
