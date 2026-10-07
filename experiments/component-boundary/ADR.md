# Component boundary ADR: supervised upstream executables and in-process Rust control plane

Status: Amended 2026-10-07. Option B (in-process Rust control plane) selected as the
production node architecture, superseding Option A (supervised upstream executables).
Tracks [E01.02](https://github.com/geoffsee/rubix-kube/issues/32) and
[E31.01](https://github.com/geoffsee/rubix-kube/issues/335) (Gate G01 of Epic #334).

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

## Amendment (2026-10-07): Production node boundary selection (Option B)

### Background and decision

The initial 2026-09-27 decision selected supervised upstream executables (Option A) based on an isolated two-component spike (official kube-apiserver + Kine v0.16.3 on Linux arm64). In October 2026, the node implementation converged on an in-process Rust control plane (Option B), codified in `crates/rubix-kube/src/runtime.rs` (`NodeRuntime`), `rubix-datastore`, `rubix-apiserver`, `rubix-controller`, `rubix-dns`, `rubix-storage`, and `rubix-portainer`.

Following the evaluation in Roadmap Epic #334 and Issue #335, **Option B (in-process Rust control plane)** is formally selected as the production architecture for Rubix Kubernetes control plane components, while retaining managed/external containerd and runtime shims for container execution.

### Component-by-component comparison

| Component | What `NodeRuntime` registers today | What Option B requires | Evidence gap between them |
| --- | --- | --- | --- |
| **Datastore** | In-process `rubix-datastore` engine (`COMPONENT_DATASTORE`) via `DatastoreAdapter::registration_for_engine`. Backed by an in-memory MVCC index, WAL, and snapshot checkpoints. | Embedded etcd v3-compatible key-value storage engine in-process implementing 64-bit monotonic revision MVCC, transactional operations, watch notifications, snapshot checkpointing (`snapshot.db`), and write-ahead logging (`member/wal/`). Must provide durable crash consistency, multi-watcher concurrency, and consistent online/offline backup procedures (`DatastoreClient::create_backup` / `restore_backup`). | `rubix-datastore` passes unit tests and ran in the macOS arm64 spike (PR #333). However, `rubix-datastore` is non-interchangeable with Kine SQLite (`../../crates/rubix-datastore/BACKUP_COMPATIBILITY.md`), rejecting SQLite files by design. Multi-watcher concurrency under sustained load, heavy transactional contention, and crash-recovery under disk failures on production Linux workloads remain unverified. |
| **API Server** | In-process `rubix-apiserver` service (`COMPONENT_APISERVER`) via `ApiserverAdapter::registration`. Backed by `KubernetesStorage` over `rubix-datastore`, exposing internal admin client and HTTP/TLS listener. | Native in-process Kubernetes API server providing standard API discovery, authenticated CRUD/list/watch, RBAC security, projected service accounts, validating/mutating webhook integration (NodeSetter on port 10443), and admission control matching official Kubernetes v1.35.7 semantics over HTTPS (port 6443). | In-process API server passes `https_gateway` tests and basic CRUD/pod status in PR #333 on macOS. However, OpenAPI v2/v3 schemas, Table responses for `kubectl`, HTTP chunked watch streaming at scale, CustomResourceDefinitions (CRDs), dynamic admission webhooks, aggregated APIs, and comprehensive upstream conformance suites on Linux remain unimplemented or unverified. |
| **Controller Manager** | In-process `rubix-controller` service (`COMPONENT_CONTROLLER_MANAGER`) via `ControllerManagerAdapter::registration`. Reconciles core resources via `apiserver_service`. | In-process Kubernetes control loops reconciling core workloads: owner-reference garbage collection, Namespace lifecycle, ServiceAccount tokens, Node leases/status, and Job/CronJob controllers without an external scheduler. | `rubix-controller` implements initial reconcilers and runs in `NodeRuntime`. However, upstream controller manager parity (including complete Garbage Collector, Job/CronJob controller edge cases, EndpointSlice batching defaults, and live workload reconciliation under Linux) has only unit/mock test coverage and is unproven on live Linux nodes. |
| **Kubelet** | Unregistered in `NodeRuntime::from_config_with_context` today. (`COMPONENT_KUBELET` constant is defined, but no adapter or service is registered in default assembly). | Single-node workload execution adapter (`rubix-kubelet`) running in-process that watches Pods assigned to the node, reconciles container lifecycles via CRI v1 (or container runtime adapter), manages cgroups, node status reporting, volume mounting (`local-path`), secret/configmap projection, CPU manager static/none policies, container probes (liveness/readiness/startup), and log collection. | Unregistered in `NodeRuntime::from_config_with_context`. PR #333 demonstrated an experimental podman-backed kubelet loop on macOS arm64 (reconciling single pods, reading logs, deleting containers). However, production kubelet integration with CRI v1, container restart policies (`Always`/`OnFailure`), probes, volume mounts, secret/token projections, static CPU reservations, and pod-to-pod networking on Linux hosts remains completely unimplemented in `NodeRuntime` and unqualified. |
| **Kube-proxy** | Unregistered in `NodeRuntime::from_config_with_context` today. (`COMPONENT_PROXY` constant is defined, but no adapter is registered in default assembly). | Service routing management (`rubix-proxy`), programming Linux host networking (iptables / nftables) for ClusterIP and NodePort Services, updating endpoints based on EndpointSlice changes, and preserving custom host firewall rules and pod egress without blanket NAT table flushes. | Unregistered in `NodeRuntime`. No live Linux iptables or nftables rules are programmed or tested from `NodeRuntime`. Parity on nftables-only and read-only `/proc/sys` hosts remains completely unverified. |
| **Container Runtime** | Unregistered in `NodeRuntime::from_config_with_context` today. `rubix-containerd` provides `ContainerdService` and `into_process_adapter()` to supervise containerd as an `OwnedProcessAdapter`, but it is not registered in default node assembly. | Supervised managed containerd executable (`containerd`, `containerd-shim-runc-v2`, `crun`, CNI plugins) or attachment to an external CRI daemon (`containerd` or `CRI-O`). For managed mode, supervisor must manage containerd lifecycle, clean disposable state, write `config.toml`, probe CRI readiness, ensure `k8s.io` namespace, and import baseline images. | `rubix-containerd` has `into_process_adapter()` and unit tests, but is not registered in `NodeRuntime`. Live supervision of the containerd daemon, runtime shims, OCI runtimes, CNI plugin execution, and sandbox container creation on Linux has not been run or qualified as part of `NodeRuntime`. |

### Explicit evidence limits

1. **Option A limit**: The [2026-09-27 Linux arm64 spike](evidence/2026-09-27-arm64/README.md) is the **only live evidence** for Option A (supervised official kube-apiserver + Kine v0.16.3 running in an isolated Linux arm64 container). It proved authenticated CRUD and datastore restart recovery, but revealed an unresolved outage-shutdown limitation (trial r2 failed graceful shutdown within 30 seconds when Kine was killed).
2. **Option B limit**: [PR #333](https://github.com/geoffsee/rubix-kube/pull/333) (experimental podman-kubelet) is the **only live evidence** for Option B, and it ran exclusively on **macOS arm64**. It proved that an in-process control plane (`rubix-datastore`, `rubix-apiserver`, `rubix-controller`) could bind a pod, launch a podman container, report status, read logs, and delete containers on macOS. It did not test Linux hosts, CRI v1, containerd supervision, volume mounts, probes, restart policies, or pod-to-pod networking.

### Impact on architecture and contracts

- **Compatibility Contract**:
  - **Deviation D03**: Updated to reflect that delivery simplicity is preserved via a single in-process Rust control plane binary (`rubix-kube`), while container execution retains managed or external runtime processes (containerd, shims, crun, CNI) and workload images.
  - **Deviation D11**: Confirmed and reinforced. `rubix-datastore` is non-interchangeable with Kine SQLite databases; state transition from Go/Kine requires explicit export/import tooling rather than direct database adoption.
  - All `kubesolo.io/v1alpha1`, `KUBESOLO_*`, `/etc/kubesolo`, and `/var/lib/kubesolo` paths, configuration precedence, and resource ownership contracts remain strictly intact.
- **Acceptance Matrix**:
  - The "Retained component boundary" table is updated: kube-apiserver, kube-controller-manager, Kine, and SQLite engine are transitioned from retained upstream executables to in-process Rust implementations (`rubix-apiserver`, `rubix-controller`, `rubix-datastore`). Kubelet and kube-proxy remain pending in-process adapter integration. Managed containerd, shims, OCI runtimes, CNI plugins, and addon images remain retained third-party assets.
- **Downstream Roadmap Gates**:
  - Releases E32.01 (Kubernetes rows), E33.01/E33.02 (Linux node bring-up scope), E34.02 (Kubelet gap list), and E36.04 (migration rehearsal shape).

