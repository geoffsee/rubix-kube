# Rubix Synthetic In-Process Fixture Report

C13/E28 remains unqualified. No retained-executable node or upstream conformance suite was run.

> Synthetic in-process fixtures only. C13/E28 and the selected upstream conformance suite remain unqualified. DO NOT claim official CNCF Certified Kubernetes qualification or multi-node certification.

## 1. Baseline Smoke Verification

| Smoke Check | Status | Duration | Details |
|---|---|---|---|
| Smoke 1 — Workload Pod Scheduling and Placement | PASS (fixture) | 13ms | Synthetic Pod API admission applied NodeSetter mutation, nodeName='rubix-node-qual'; no workload executed |
| Smoke 2 — In-Cluster CoreDNS Resolution | PASS (fixture) | 4ms | Synthetic DNS model matched kubernetes.default.svc.cluster.local to ClusterIP 10.43.0.1; no CoreDNS server or pod query executed |
| Smoke 3 — Pod Egress Masquerade / SNAT Routing | NOT EXECUTED | 0ms | NOT EXECUTED: in-process fixtures have no pod runtime or network dataplane |

## 2. Six Baseline Manifest Domains

| Domain | Status | Assertions | Duration |
|---|---|---|---|
| Tier 1 — Workloads & Networking | PASS | 10 | 47ms |
| Tier 2 — Storage Persistence | PASS | 12 | 75ms |
| Tier 3 — Config & Identity | PASS | 6 | 20ms |
| Tier 4 — Controllers | PASS | 11 | 95ms |
| Tier 5 — DNS & LoadBalancer | PASS | 5 | 77ms |
| Tier 6 — LoadBalancer UPDATE path [KS-75] | PASS | 10 | 135ms |

## 3. Selected Single-Node Conformance Summary

NOT EXECUTED. The inventory lists planned candidates; API-object creation does not establish upstream test execution.

- **Total Selected Tests**: 24
- **Passed**: 0
- **Failed**: 0
- **Explicit Exclusions**: 7
- **Focus Pattern**: `(ConfigMap|Secret|Pods|Services|Deployment|ReplicaSet|DNS|Projected|Downward|EmptyDir).*\[Conformance\]`
- **Skip Pattern**: `\[Serial\]|\[Disruptive\]|\[Slow\]|\[Flaky\]|two nodes|multiple nodes|more than one node`

### Explicit Exclusions with Rationale

| Pattern | Category | Technical Rationale |
|---|---|---|
| `[Serial]` | `SerialSlow` | Tests tagged [Serial] are excluded; this report provides no test-specific runtime measurements. Synthetic in-process domain tests exercise concurrency; no live single-node concurrency is qualified. |
| `[Disruptive]` | `Disruptive` | Tests tagged [Disruptive] are excluded by the selected single-node suite policy; this fixture provides no test-specific disruption or recovery observations. |
| `[Slow]` | `SerialSlow` | Tests tagged [Slow] are excluded; this report provides no test-specific soak-duration or replica-count evidence. |
| `[Flaky]` | `Flaky` | Tests tagged [Flaky] are excluded by the selected suite policy; this fixture provides no upstream flakiness measurements. |
| `two nodes` | `MultiNode` | Rubix is an architectural single-node Kubernetes distribution using NodeSetter admission; multi-node scheduling topologies are not applicable. |
| `multiple nodes` | `MultiNode` | Multi-node scheduling, cross-node pod anti-affinity, and node failover are out of scope for single-node Rubix clusters. |
| `more than one node` | `MultiNode` | Workload distribution across multiple physical hosts requires a full cluster topology, inapplicable to single-node architecture. |

### Kubeconfig Format Accommodation

Verified dual-format accommodation: **YAML and JSON (dual-format validated)** (YAML and JSON interchangeability confirmed).

