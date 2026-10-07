# Accepted implementation and release matrix

Recorded 2026-09-27 for [E01.04](https://github.com/geoffsee/rubix-kube/issues/34).
This consolidates the [compatibility contract](compatibility-contract.md), the
[selected component boundary](../../experiments/component-boundary/ADR.md), and
[official input and generation ownership](upstream-inputs.md). Those documents remain authoritative
for detailed behavior, exact hashes and the E01–E30 acceptance inventory; this matrix assigns the
implementation boundaries and integration/release gates without repeating their acceptance prose.

Under the 2026-10-07 [ADR amendment](../../experiments/component-boundary/ADR.md) ([E31.01](https://github.com/geoffsee/rubix-kube/issues/335)),
the selected product architecture is Option B: an in-process Rust control plane matching `NodeRuntime`
(`rubix-datastore`, `rubix-apiserver`, `rubix-controller`), retaining managed/external containerd and runtime
shims for container execution. Supervised upstream control plane executables (Option A) are superseded.
Published resource bindings and explicitly generated protocol bindings implement clients/types, not the
components behind them. Ordinary Cargo builds and node startup never refresh upstream. Component assets and
generator inputs have separate locks and owners; the accepted [provenance inventory](upstream-inputs.json)
governs generation pins. It is not a production component artifact lock. E06 still owes verified per-target
executable/image digests and source/build closure for retained assets.

## Retained component boundary

Versions below distinguish baseline inputs from release-qualified assets. A version/tag is not a
verified artifact digest. E06 must resolve per-platform content hashes and E27 must assemble and
attribute every retained asset. Missing hashes or unsupported packaging block that artifact's
integration/release, never silently narrow the support matrix.

The source baseline is KubeSolo
[`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`](https://github.com/portainer/kubesolo/commit/2ef1c4787989f11f868f81bb84ae2afd4a49a81d).
Component versions come from its [go.mod][modules], [asset script][assets],
[constants][constants] and [local-path helper template][helper].

| Retained component | Selected boundary / baseline input | Interface and ownership | Remaining acceptance owner/gate |
| --- | --- | --- | --- |
| kube-apiserver | Superseded by in-process `rubix-apiserver` (Option B amendment 2026-10-07); baseline input was official Kubernetes v1.35.7 (reference Go used K3s v1.35.7-k3s1) | In-process HTTPS listener, authenticated credentials, admission webhooks, internal datastore binding (`KubernetesStorage`) | E11/E32 in-process API parity, admission, discovery, streaming watch, and conformance; E02 independent fixtures |
| kube-controller-manager | Superseded by in-process `rubix-controller` (Option B amendment 2026-10-07); baseline input was official Kubernetes v1.35.7 | In-process reconciliation loops over in-process API server; upstream Go reconcilers superseded | E12/E32 controller parity, complete required reconciliation and lifecycle |
| kubelet | In-process `rubix-kubelet` adapter pending in `NodeRuntime` (Option B amendment 2026-10-07); baseline input was official Kubernetes v1.35.7 | In-process node adapter watching API server and dispatching CRI v1 to container runtime | E13/E34 Linux host/container CPU/cgroup/probe/volume behavior; E10 CRI driver fallback |
| kube-proxy | In-process `rubix-proxy` adapter pending in `NodeRuntime` (Option B amendment 2026-10-07); baseline input was official Kubernetes v1.35.7 | In-process packet filtering adapter programming Linux host iptables/nftables from EndpointSlices | E16/E34 nftables/iptables/read-only sysctl parity; preserve owned egress rules |
| Kine | Superseded by in-process `rubix-datastore` engine (Option B amendment 2026-10-07); baseline input was Kine v0.16.3 with SQLite support | In-process etcd v3-compatible key-value MVCC/WAL engine; non-interchangeable with Kine SQLite | E08/E32 persistence, monotonic 64-bit MVCC revisions, WAL durability, crash consistency, and backup/restore |
| SQLite engine | Superseded by in-process `rubix-datastore` memory-mapped MVCC index and WAL (Option B amendment 2026-10-07) | In-process datastore state; Kine SQLite on-disk databases require offline migration tooling per D11 | E08/E30/E36 explicit migration tooling and rehearsal; see [BACKUP_COMPATIBILITY.md](../../crates/rubix-datastore/BACKUP_COMPATIBILITY.md) |
| Managed containerd | Confirmed retained upstream: supervised official containerd v2.2.5 executable | CRI v1; native API module v1.10.0 for justified image/namespace operations; Rust owns managed config/start/stop via `OwnedProcessAdapter` | E09 fork/plugin/registry/import/restart equivalence; E06 target assets; E02 protocol fixtures |
| containerd-shim-runc-v2 | Confirmed retained upstream: matching containerd v2.2.5 payload | Managed containerd owns shims; registered runtime identifier remains `io.containerd.runc.v2` | E09 PATH lookup, task cleanup, cross-target payload and process accounting |
| crun | Confirmed retained upstream: baseline 1.26; ARMv7 built from source in reference | OCI runtime executable configured by `BinaryName`; host-owned in external-runtime mode | E06/E09 digest/build provenance, cgroups, container lifecycle |
| CNI bridge, host-local, portmap, loopback | Confirmed retained upstream: baseline containernetworking/plugins v1.9.0 | Executables with owned CNI config; external-runtime plugin binaries are host-owned | E06/E15 architecture, MTU, egress, ordering and cleanup |
| fuse-overlayfs snapshotter and helper | Confirmed retained upstream: supervised upstream `containerd-fuse-overlayfs-grpc` v2.1.7 plus host `fuse-overlayfs` executable when available | Snapshot RPC over an owned Unix socket through containerd `proxy_plugins."fuse-overlayfs"`; preserve overlay/native/fuse selection | E06 packages separate plugin binary; E09 verifies executable startup, proxy registration, nested/overlay workload and cleanup; host helper version recorded |
| CoreDNS | Confirmed retained upstream: `docker.io/coredns/coredns:1.14.4` image | Pod workload; Rust reconciles owned resources/Corefile, readiness and acquisition | E06/E17 digest, online/offline DNS and metadata preservation |
| Pause sandbox | Confirmed retained upstream: `docker.io/portainer/pause:latest` actual reference pull/runtime name | CRI sandbox image; reference script's `PAUSE_IMAGE_VERSION=3.10` does not pin the pulled `latest` image | E06/E09 resolve digest and architecture; do not misrepresent unused version variable as content provenance |
| Local-path provisioner | Confirmed retained upstream: `docker.io/rancher/local-path-provisioner:v0.0.36` image | Optional workload; Rust owns resources/default Retain class/shared path configuration | E18 PVC/data/reclaim tests; E06 offline payload |
| Local-path helper pod | Confirmed retained upstream: template image `busybox` (implicit mutable tag) | Provisioner launches helper; not part of Rust executable or proof from provisioner image alone | E06/E18 resolve/pin/include required helper payload for egress-denied PVC provisioning; no offline claim from startup alone |
| Portainer Edge Agent | Confirmed retained upstream: `docker.io/portainer/agent:lts` or configured custom reference | Optional image; bootstrap-only Rust ownership, existing Portainer objects preserved | E19 credentials/async/custom-image behavior; E06 resolve default digest and architecture |
| D2K | Confirmed retained upstream: `docker.io/portainer/d2k:1.2.3` image | Optional Docker-compatible API workload; Rust owns credentials/reconciled resources | E20 actual client-certificate authentication/negative cases and endpoint readiness; E06 image support |
| External containerd / CRI-O | Confirmed retained upstream: host-managed CRI implementations; no invented universal server version | Rust attaches through CRI and never owns host daemon, socket, registry or unrelated workloads | E10 records exact tested versions/configurations; E28 qualification matrix; unsupported protocol/capabilities fail clearly |
| Host tools and container engine | Confirmed retained upstream: init manager, iptables/nft, mount/module helpers, optional fuse helper; Docker-compatible engine for container mode | Host prerequisites remain externally owned; Rust invokes/negotiates only within explicit installation scope | E05/E15/E23/E24 record versions/capabilities and constrained-host fixtures; no blanket host qualification |

The selected snapshotter integration uses an existing upstream standalone executable, not a new
Go bridge. Tag v2.1.7 resolves to commit `da57796c7d0a2b608abf173651cf148706e33337`
(annotated tag object `40dc5044b905a5f27d9b650637706e61c64daa11`). Its
[entry point](https://github.com/containerd/fuse-overlayfs-snapshotter/blob/da57796c7d0a2b608abf173651cf148706e33337/cmd/containerd-fuse-overlayfs-grpc/main.go)
accepts a socket address and snapshot root, registers containerd snapshot gRPC, and validates
filesystem support. The fetched source is 2,695 bytes, SHA-256
`6dad105cf8b6972fac0fbf7b58343bad8db6c7917889248d5a542d0354a7060a`.
Its [installation documentation](https://github.com/containerd/fuse-overlayfs-snapshotter/blob/da57796c7d0a2b608abf173651cf148706e33337/README.md)
describes `proxy_plugins."fuse-overlayfs"` with type `snapshot` and Unix socket address.
E09 must configure private owned paths before launching it: the upstream executable removes its
socket path recursively, so no arbitrary user-supplied/shared path may be passed unchecked. Start
and probe the snapshotter before managed containerd needs it; stop after dependent runtime work.
The binary and its mounts/processes count in lifecycle and footprint evidence. Execution is still
unverified; if source adaptation proves necessary, keep it a documented narrow retained executable
under the selected process boundary. Host helper absence retains the baseline native-snapshotter
fallback; missing plugin packaging must not silently disable an otherwise available fuse path.

Development-only tools are separate: Go default extraction, protobuf/Rust generators and the
reference asset downloader's crane v0.21.5 do not imply shipped dependencies. E02 owns their exact
invocation and provenance; E06/E27 own prepared artifacts. A packaged utility becomes a retained
asset and needs inventory/license/target evidence. SQLite and other transitive linked libraries
remain in dependency/license closure even when they are not separate processes.

## Rust-owned implementation

| Rust responsibility | Acceptance owners | Stable boundary |
| --- | --- | --- |
| Config schema, defaults, legacy flags/environment and atomic file writer | E03 | KubeSolo input/file compatibility, explicit omission semantics |
| Supervisor, readiness, cancellation and diagnostics | E04 | Owned subprocess identities, component health, dependency-aware start/stop, bounded escalation |
| Host detection/preflight and permitted preparation | E05 | Capabilities before mutation; disposable-host validation |
| Asset validation/extraction and variant inventory | E06 | Prepared manifest and content hashes, no implicit upstream refresh |
| PKI, service accounts and component/client credentials | E07 | Persistent trust roots, private keys, leaf rotation |
| Datastore engine, adapter and recovery controls | E08 | In-process MVCC/WAL engine (`rubix-datastore`), WAL durability, snapshot checkpointing and D11 explicit Kine SQLite migration tooling; no fabricated storage implementation |
| Managed/external runtime adapters | E09/E10 | Managed ownership versus attachment; CRI and native containerd API |
| Kubernetes component configuration/lifecycle adapters | E11/E12/E13/E16 | In-process Rust control plane components (`rubix-apiserver`, `rubix-controller`), pending in-process node adapters (`rubix-kubelet`, `rubix-proxy`) in `NodeRuntime`, and independently validated defaults |
| NodeSetter/LoadBalancer webhook | E14 | Rust admission handling/status updates; no scheduler introduced |
| Address/resolver/MTU/CNI/owned egress | E15 | Host networking ownership and idempotence |
| Addon builders and reconciliation | E17/E18/E19/E20 | DNS/storage/Portainer/D2K retain distinct ownership/failure rules |
| Product metrics, probes and replacement diagnostics | E21 | Truthful metrics and bounded probes; no fabricated Go runtime series |
| Config API and editing CLI | E22 | Stored desired state, Unix socket permissions, validation, restart required |
| Installer, command shell and service integration | E23 | Supported target/init/run-mode contract |
| Named container management | E24 | Engine requests, ports/network/volume ownership |
| Kubeconfig and D2K client access | E25 | Invoking-user identity and selected-instance credentials/context |
| Upgrade/reset/uninstall and migration controls | E26 | Explicit retain/purge/keep-config rules, safe interruption |
| Generation/oracles, delivery, qualification and handoff tooling | E02/E27/E28/E29/E30 | Reproducible evidence and release gates, not substitutes for component behavior |

E01 owns decisions across these interfaces. A worker may implement independent config/builders or
fixtures before a live consumer exists, but cannot claim integrated behavior until that consumer's
acceptance tests pass. New crate names are justified by real consumers/build isolation; illustrative
proposal crates and command names remain nonbinding.

## Supported target obligations

The following archive matrix is exhaustive for the baseline node distribution. Every cell requires
artifact build/layout/install smoke plus a declared runtime/qualification record. None is qualified
by the arm64 API/Kine spike. O = online, F = offline; each displayed pair is two required artifacts.

| Linux node architecture | glibc | musl | Required container image architecture | Optional-image limits |
| --- | --- | --- | --- | --- |
| amd64 | O + F | O + F | linux/amd64 | Portainer and D2K supported |
| arm64 | O + F | O + F | linux/arm64 | Portainer and D2K supported |
| ARMv7 hard-float | O + F | O + F | linux/arm/v7 | Portainer supported; D2K disabled |
| riscv64 | O + F | O + F | linux/riscv64 | Portainer unavailable; D2K disabled |

E27 owes all 16 node archive cells and four OCI architecture entries, including matching assets.
The four management release targets remain Linux amd64/arm64 and macOS amd64/arm64. ARM/RISC-V
host installation uses the supported script/direct route unless matching CLI artifacts are added;
management source cross-compilation does not establish published artifact availability. Native
Windows binaries are excluded; WSL2 follows the Linux userspace/container-engine workflow.

| Additional dimension | Required obligation | Acceptance owners / current evidence |
| --- | --- | --- |
| Runtime provider | Managed containerd, external containerd and external CRI-O, with exact versions recorded per run | E09/E10/E28; all product combinations unqualified |
| Runtime build mode | Embedded-dependency online/offline versus host-supplied external-dependency builds; independent of external runtime selection | E06/E09/E10/E27; external-dependency builds must compile without embedded payloads |
| Init/lifecycle | systemd, OpenRC, SysVinit, Upstart, runit, s6, foreground and daemon | E05/E23/E26; fixture every adapter and live representative hosts, explain host availability |
| Container mode | Linux/macOS engine clients and supported WSL2 workflow; named clusters and persistent volumes, random loopback API/D2K ports, explicit workload ports | E24/E25/E28; static CPU manager intentionally unsupported in container mode |
| Host capabilities | cgroup v1/v2, Alpine/OpenRC, overlay/nested containers, custom writable roots, nftables-only, read-only `/proc/sys`, VPN/low MTU and multi-NIC override | E05/E09/E10/E13/E15/E16/E28; explicit missing-capability failures, no destructive live-host fixture |
| Online/offline | CoreDNS/pause included online; required supported optional images and helper images supplied offline with egress denied | E06/E17–E20/E23/E27/E28; custom image pull rules remain explicit |
| External runtime ownership | Preserve host process/socket/registry/non-distribution workload state on start/stop/restart/uninstall | E10/E15/E26/E28; distribution owns only its CNI configuration |
| Optional components | Disabled mode has no endpoint/resource/image side effects; supported/unsupported image targets match table | E18–E20; D2K unsupported-target disablement precedes LoadBalancer validation |

This is not a blind Cartesian product of impossible host capabilities. E28 must record the concrete
valid cells and why a host capability cannot apply. Representative live tests plus adapter fixtures
must satisfy each domain's acceptance; build-only or emulated validation must be labeled. Absent
hardware/access is a gap, never proof or permission to remove promised architectures. E29 requires
paired amd64/arm64 whole-distribution reports and explicit secondary-architecture limitations.

## Execution sequence and readiness

Issue numbers are identifiers, not execution order. Child-specific prerequisites determine when
bounded work can start. Parent dependencies gate integrated epic completion. Keep the live issue
relationships authoritative: start with the [E01 parent and native children](https://github.com/geoffsee/rubix-kube/issues/1)
and [repository issue inventory](https://github.com/geoffsee/rubix-kube/issues). Reconcile optional
local `docs/planning/` mirrors before selecting a slice; those files need not exist in a clean checkout.

| Stage | Work that can start with stable inputs | Required integration evidence before completion |
| --- | --- | --- |
| Characterization and inputs | E02 fixtures/harness plus explicit preparation/generation from E01 decisions | Independent source/artifact identities, negative fixtures, deterministic regeneration and drift failure |
| Foundation | E03 typed config; E04 lifecycle interfaces; E05 detection fixtures; E06 asset manifest; E07 PKI | Relevant E02 fixtures and config interfaces; then production shutdown/host/asset/identity tests |
| Storage and runtime providers | E08 in-process datastore (rubix-datastore); E09 managed containerd; E10 external CRI; E22 stored-config editing | WAL persistence, MVCC transactions and snapshot recovery for datastore; host/assets for managed runtime; host capabilities for external runtime |
| Control plane and networking | E11 API, then E12 controllers; E15 networking after provider/host interfaces | Persistent authenticated API; required controllers; idempotent owned CNI/egress before persisted pods recover |
| Workload execution and admission | E13 kubelet after E15 networking; E14 NodeSetter; E16 kube-proxy | Manually assigned pod proves initial kubelet without needing NodeSetter; then normal unscheduled workloads through webhook, then routing |
| Addons and observability | E17–E20 resource builders and E21 probes can use fixture clients early | DNS is required readiness; optional addons isolate failure; real workloads/credentials/networking prove final behavior |
| Operator lifecycle | E23 host installer; E24 container manager; E25 access; E26 lifecycle cleanup | Each integration uses actual API/runtime/provider behavior and disposable environments; E25 includes D2K credential coverage |
| Distribution | E27 artifact assembly and publication rehearsal | Matching supported target inputs, installed layout, provenance, secrets separation and prerequisite lifecycle evidence |
| Qualification and handoff | E28 suites/soak, E29 paired budgets, E30 migration/recovery/operator rehearsal | Every required test/platform result, measured budgets, accurate limitations, version/destination authorization before publication |

Runtime ordering is separate from the development dependency graph. Resolve configuration and host
capabilities, prepare assets/identity, start protected datastore/runtime endpoints, start API and
controllers, prepare networking before kubelet pod reconciliation, then bring up required admission
and routing/addons according to independently established readiness. Do not make a controller/provider
wait for a future consumer merely because its final epic requires consumer integration. DNS/addon
pods must not create an API/kubelet/NodeSetter readiness cycle. E04 owns the executable graph and
failure propagation; no diagram alone establishes ordering correctness.

For exact epic completion gates, the following list preserves the planned graph. A row does not
forbid earlier design/fixture work; child prerequisites still apply.

| Epic owner | Parent integration/completion prerequisites |
| --- | --- |
| E01 | None; foundation decisions |
| E02 | E01 |
| E03 | E01, E02 |
| E04 | E02, E03 |
| E05 | E03, E04 |
| E06 | E02, E03 |
| E07 | E03, E04 |
| E08 | E04, E07 |
| E09 | E04, E05, E06 |
| E10 | E04, E05 |
| E11 | E07, E08 |
| E12 | E11 |
| E13 | E05, E09, E10, E11, E15 |
| E14 | E11, E12, E13 |
| E15 | E05, E09, E10 |
| E16 | E11, E12, E13, E15 |
| E17 | E06, E12, E13, E14, E15, E16 |
| E18 | E06, E12, E13, E14, E16 |
| E19 | E06, E07, E12, E13, E14, E16, E17 |
| E20 | E06, E07, E12, E13, E14, E16, E17 |
| E21 | E04, E07, E08, E09, E10, E11, E13, E16 |
| E22 | E03, E04 |
| E23 | E03, E05, E06, E11 |
| E24 | E05, E06, E11, E13, E15, E16, E23 |
| E25 | E07, E20, E23, E24 |
| E26 | E22, E23, E24, E25 |
| E27 | E01, E06, E23, E24 |
| E28 | E08, E09, E10, E11, E12, E13, E14, E15, E16, E17, E18, E19, E20, E21, E22, E23, E24, E25, E26, E27 |
| E29 | E02, E21, E27, E28 |
| E30 | E26, E27, E28, E29 |

## Deviation and unresolved-gate register

D01–D11 are defined in the compatibility contract and retain their existing owners. This register
adds the selected component boundary and measured limitations without claiming a later gate passed.

| Record | Disposition / required evidence | Owner |
| --- | --- | --- |
| D01/D05 target and bundle corrections | Keep all node targets; reject incorrect target metadata or executable; fix installer routes | E05/E23/E27 |
| D02 version/capability consistency | Explicit candidate version/capabilities; historical migration gates remain distinct | E23/E26/E27 |
| D03 supervised processes | Option B selected in ADR amendment (2026-10-07); in-process Rust control plane with retained third-party runtime processes (containerd, shims, crun, CNI) and images explicitly attributed and measured | E01/E04/E27/E29/E31 |
| D04 local storage defaults | Preserve effective runtime enabled default and explicit disabling | E03/E18/E23 |
| D06 stale commands/tags and mutable images | Actual command tree/version tags; resolve image digests separately from display references | E06/E24/E25/E27 |
| D07 D2K authentication/readiness | No mTLS claim until actual image positive and negative client-certificate tests; reconciliation is not endpoint readiness | E20 |
| D08 diagnostics | Rust replacement diagnostics; product metrics retain honest probe meaning | E21 |
| D09 current defaults | No resurrected low-memory overrides, omitted controllers or non-no-op `--full` | E03/E11/E12/E13/E16/E29 |
| D10 stored config | Desired document only, restart required, runtime overrides not persisted | E22 |
| D11 migration | Confirmed under Option B (`rubix-datastore`). Rehearsed state reuse or explicit export/import/recovery; no automatic datastore interchangeability; rubix-datastore rejects raw Kine SQLite files; see [state transitions](state-transitions.md) and [BACKUP_COMPATIBILITY.md](../../crates/rubix-datastore/BACKUP_COMPATIBILITY.md) | E08/E26/E30/E36 |
| Official Kubernetes versus K3s | Control plane superseded by in-process Rust components under Option B; official v1.35.7 remains semantic baseline for API, reconcilers, and CRI contracts; fork adaptations assessed explicitly | E02/E11/E12/E13/E16/E32; E28 integrated parity |
| Official containerd versus embedded fork/plugins | Runtime API/native image operations plus shim/plugin equivalence; external CRI is a separate mode | E02/E06/E09/E10 |
| Datastore confidentiality/integrity boundary | Option A required loopback mTLS between processes. Under Option B, rubix-apiserver connects in-process to rubix-datastore via internal storage (`KubernetesStorage`), while any external etcd v3 client endpoint requires dedicated datastore CA TLS authentication; never reuse general client CA | E07/E08/E11/E32 |
| Persistent datastore outage shutdown | Trial r2 required forced API kill after Kine died; preserve diagnostics and implement bounded escalation rather than call it graceful shutdown | E04/E08/E28 |
| Offline helper image closure | Resolve busybox helper and all enabled transitive workload images, then provision PVC with egress denied | E06/E18/E28 |
| Architecture-specific assets and external server versions | Resolve and hash all shipped payloads, record actual external CRI/engine versions and capability matrix | E06/E09/E10/E24/E27/E28 |
| Distribution performance | Provisional contract gates require paired whole-distribution data; two-process startup RSS is not a release budget result; see [budget profiling analysis](budget-profiling-analysis.md) | E21/E28/E29 |

A gate blocks dependent integration or release until its owner supplies evidence. If resolving it
requires changing an accepted product promise, record an explicit impact decision; do not silently
reclassify the behavior as unsupported or replace a failed assertion with a stub.

## Evidence and completion standard

The [arm64 component evidence](../../experiments/component-boundary/evidence/2026-09-27-arm64/README.md)
proves TLS/client-certificate CRUD, unauthorized rejection, independent SQLite integrity/object
presence, clean restart and UID/value retention, acknowledged update after datastore SIGKILL,
existing API recovery, persisted deletion, and cleanup for the Option A two-component boundary.
The run records exact source hashes, two component processes and approximately 294–301 MiB combined
RSS at readiness snapshots. It excludes the Rust supervisor, other Kubernetes components, runtime,
shims, addon workloads and steady-state benchmarking. Neither amd64 nor all supported platforms are
qualified by this run. The retained failed r2 demonstrates an unresolved outage-shutdown condition.

For Option B, [PR #333](https://github.com/geoffsee/rubix-kube/pull/333) provides the only live evidence
(evaluated on macOS arm64 for the in-process control plane and an experimental podman loop). Live
qualification of Option B on Linux hosts—covering in-process API server, controller manager, datastore
crash consistency, containerd daemon supervision, and in-process kubelet/kube-proxy integration—remains
the gating requirement for Epic #334.

Generation research/probes are described only by their actual scope in [upstream inputs](upstream-inputs.md).
Their schema/descriptor/translation/compilation results do not prove CRI runtime interoperability or
API behavior. E02 must provide committed repeatable tools and independent oracles before claiming
production preparation and drift checks exist.

Each implementation/qualification result records stable issue ID, tested revision, artifact/input
hashes, command or CI link, exact environment/variant, assertions, failures/skips, and durable logs.
Distinguish implemented, locally tested, integration verified and release qualified. A missing
required environment or failure is a visible gap. E28's selected conformance count and exclusions
must be stated; it is not certification. E29 measures every retained process and default workload.
E30 closes only after all prior acceptance criteria and migration/recovery/operator handoff evidence
are integrated. Actual publication additionally requires an established destination and authorized
version policy; preparing a candidate or rehearsal is not publishing a supported release.

## Release completion audit (Roadmap #263)

All E01–E30 contracts above remain authoritative. Issue closure, source presence,
metadata checks, unit tests and synthetic fixtures do not establish completion.
The new `rubix-qualification` command audits repository metadata but returns a
nonzero exit status for release qualification: no validated current candidate-bound
completion receipt reader is implemented. `--metadata-only` explicitly performs
only the repository audit and cannot qualify a release.

| Criterion | Pending qualification evidence |
| --- | --- |
| 1 | Independently sourced current evidence for all required E01–E30 deliverables |
| 2 | Production boundary and live positive/negative datastore mTLS checks |
| 3 | Build/layout/install evidence for all 16 node cells, four OCI architectures and four management targets |
| 4 | Exact BusyBox helper digest, fresh egress-denied PVC provisioning, live image acquisition and D2K authentication |
| 5 | Host/container lifecycle interruption and retention matrices |
| 6 | Fresh candidate-bound conformance, restart and platform soak |
| 7 | Paired live amd64/arm64 budgets, 24-hour memory and shutdown |
| 8 | Supported-version live Kine migration and interrupted recovery |
| 9 | All required final debug/release/audit/drift/integration checks |
| 10 | Actual prepared inputs, complete candidate inventory and report bindings |
| 11 | Fresh operator rehearsal and supported release publication |

C13, C14, C16 and C17 remain pending under the existing qualification contracts.
Attribution and local link checks are metadata audits; they do not verify live
behavior, externally fetched license notices, artifact layout or publication.

[modules]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/go.mod
[assets]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/build/download-deps.sh
[constants]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/types/const.go
[helper]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/components/localpath/configmap.go
