# Candidate: supervised upstream component executables

Status: selected component boundary following the successful isolated protocol experiment
and independent review; production implementations and full qualification remain gated below.
Tracks [E01.02](https://github.com/geoffsee/rubix-kube/issues/32).

## Problem and boundary

KubeSolo embeds Kubernetes, Kine and containerd Go libraries. Rust API bindings do
not supply those implementations. E01 excludes rewriting Kubernetes and explicitly
allows retained non-Rust components when their packaging, security, cancellation and
resource costs are recorded.

The candidate assigns distribution configuration, identity, lifecycle, host changes,
network preparation, webhook behavior, addon reconciliation and operator commands to
Rust. Retained executables provide kube-apiserver, kube-controller-manager, kubelet,
kube-proxy, Kine and managed containerd. Runtime shims, OCI runtimes, CNI executables
and addon images remain explicit third-party assets. External runtimes remain owned
by their external supervisor. This inventory is a candidate responsibility split;
only kube-apiserver and Kine are exercised here.

The integration uses versioned Kubernetes HTTPS/JSON, CRI/containerd RPCs, and Kine's
etcd-compatible protocol. The experiment uses authenticated Kubernetes HTTPS and
loopback-only plaintext Kine within an isolated network namespace. The selected production transport is loopback mTLS with a dedicated datastore CA,
server IP SAN `127.0.0.1`, and a separate API-server client identity. Kine requires
client certificates via `--trusted-ca-file`; the API uses `--etcd-cafile`,
`--etcd-certfile` and `--etcd-keyfile` with an HTTPS endpoint. Do not reuse the general
Kubernetes client CA: its other clients must not gain direct datastore access.
See the [source-backed transport contract](../../docs/architecture/upstream-inputs.md).
This intentionally corrects the baseline plaintext trust boundary. The isolated
spike proves protocol/persistence only; E07/E08/E11 must test production mTLS and
wrong/no-client-certificate rejection before integration.

Official Kubernetes v1.35.7 owns generation semantics. The pinned KubeSolo source
uses v1.35.7-k3s1 replacements and containerd v2.2.5-k3s2. The spike's official API
server is an explicit candidate substitution, not a declaration of fork equivalence.
Kine v0.16.3 is retained. Source-version adoption, K3s delta evaluation, all other
component pins, generated inputs and independent parity oracles remain E01.03/E02 gates.

## Alternatives

| Candidate | Benefit | Cost or unresolved evidence |
| --- | --- | --- |
| Supervised upstream executables | Existing process/RPC boundaries, independent failure handling, no Go ABI inside Rust | More processes and potentially higher whole-distribution RSS/artifact size; fork adaptations need parity review |
| Narrow Go helper embedding upstream libraries | Retains Go-library integration and may share runtime overhead | Shipped Go helper and custom control protocol; duplicated lifecycle policy must be avoided; helper crashes affect all embedded components |
| In-process Go bridge through FFI | Potential single control-process packaging | Unsafe ABI boundary, Go runtime/thread/signal interaction, cross-target linking and cancellation complexity; no working proof yet |

Choose supervised executables for the first experiment because it exposes the
component compatibility boundary directly. Do not reject alternatives based on
unmeasured footprint claims. A failed experiment, unacceptable measured whole-node
budgets, or an essential fork adaptation can require revisiting this candidate.

## Packaging and cancellation

A distribution bundle can contain one Rust entry point plus verified executable
assets, materialized in owned paths. This changes KubeSolo's single-control-process
implementation. A self-extracting installer would still launch multiple processes;
calling such packaging a fully native Rust Kubernetes implementation would be false.
The prototype uses two component processes plus its test driver. Final process count
also includes remaining components, helpers and addon workloads and is not established here.

Production supervision must preserve owned child identity, bound readiness, send
SIGTERM in reverse dependency order, reap processes, report abnormal exits, and use
forced termination only as diagnosed cleanup. The fixture starts Kine before the API,
stops API before Kine, fails on forced termination, and checks surviving process groups.
This proves a process/RPC candidate; implementing Rust lifecycle behavior belongs to E04.

## Acceptance and limitations

The executable assertions cover authenticated CRUD, authorization rejection, retained
SQLite state across both-component restarts, acknowledged updates after intentional
datastore SIGKILL, deletion persistence, database integrity,
readiness duration, RSS and clean shutdown. A successful run writes machine-readable
facts and retains logs. The [Linux arm64 evidence](evidence/2026-09-27-arm64/README.md) records the passing
run, exact source hashes, measurements and preceding failures.

This test does not exercise all API verbs, watch/compaction semantics, KubeSolo admission,
controller behavior, workload networking, resource recovery under disk failure,
Go-to-Rust migration, all platform variants, sustained idle memory or product release
budgets. It does not compare all three alternatives experimentally. These gaps must
remain visible in the implementation/release acceptance matrix.

## Observed outage/shutdown limitation

The 2026-09-27 arm64 trial `rubix-boundary-evidence-20260927-r2` passed
verified TLS, authorization negatives, CRUD, normal shutdown, independent SQLite
integrity/persistence, and recovery after a clean restart. After intentional Kine
SIGKILL, attempting to stop the API server while its datastore remained unavailable
did not exit within the 30-second grace period. Logs repeatedly show failed gRPC
connections to `127.0.0.1:2379`; the fixture escalated to SIGKILL and correctly failed.
The retained failed result must not be represented as a passing clean shutdown.

The recovery scenario now restarts only Kine, observes that the existing API process
recovers readiness and the acknowledged update, and then stops API before datastore.
This exercises recovery from a datastore outage without conflating unavailable-store
shutdown with ordinary dependency-ordered shutdown. E04 must still implement bounded
escalation and retain diagnostics when a dependency stays unavailable. Graceful API
termination within 30 seconds during a persistent datastore outage remains unproven
and was observed to fail; a passing recovery run does not erase that limitation.
