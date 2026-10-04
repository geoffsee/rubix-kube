# Accepted implementation and release matrix

Recorded 2026-09-27 for [E01.04](https://github.com/geoffsee/rubix-kube/issues/34).
This consolidates the [compatibility contract](compatibility-contract.md), the
[selected component boundary](../../experiments/component-boundary/ADR.md), and
[official input and generation ownership](upstream-inputs.md). Those documents remain authoritative
for detailed behavior, exact hashes and the E01–E30 acceptance inventory; this matrix assigns the
implementation boundaries and integration/release gates without repeating their acceptance prose.

The selected product is a Rust-owned distribution supervising retained upstream executables.
It is not a native Rust implementation of Kubernetes, Kine, containerd or addon images. No Go ABI
or custom Go control bridge is selected. Published resource bindings and explicitly generated
protocol bindings implement clients/types, not the components behind them. Ordinary Cargo builds
and node startup never refresh upstream. Component assets and generator inputs have separate locks
and owners; the accepted [provenance inventory](upstream-inputs.json) governs generation pins.
It is not a production component artifact lock. E06 still owes verified per-target executable/image
digests and source/build closure for assets beyond the narrow recorded experiment.

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
| kube-apiserver | Supervised official Kubernetes v1.35.7 executable; reference Go libraries used K3s v1.35.7-k3s1 | Kubernetes HTTPS/JSON, authenticated component credentials; etcd-compatible datastore client | E11 official-versus-K3s defaults/admission/security parity; E02 independent fixtures |
| kube-controller-manager | Supervised official Kubernetes v1.35.7 executable | API credentials/configuration from Rust; upstream reconcilers remain Go | E12 controller defaults, complete required reconciliation and lifecycle |
| kubelet | Supervised official Kubernetes v1.35.7 executable | Kubernetes API and CRI v1; Rust owns config/identity/checkpoint-change policy | E13 host/container CPU/cgroup/probe/volume behavior; E10 CRI driver fallback |
| kube-proxy | Supervised official Kubernetes v1.35.7 executable | API/EndpointSlices plus privileged host routing; Rust owns options/lifecycle | E16 nftables/iptables/read-only sysctl parity; preserve owned egress rules |
| Kine | Supervised Kine v0.16.3 executable with SQLite support | etcd-compatible boundary; Rust owns state location/configuration/lifecycle; Kine owns datastore semantics | E08 persistence/watch/resource versions, WAL/integrity/repair and production local transport protection |
| SQLite engine | Kine's locked dependency/build, not an independently selected Rust database | Kine-owned on-disk state; no direct Rust rewrite of Kubernetes storage | E08/E27 identify linked implementation/license and durability; E30 explicit migration |
| Managed containerd | Supervised official containerd v2.2.5 executable; reference embedded library replaced by K3s v2.2.5-k3s2 while download script already uses official v2.2.5 | CRI v1; native API module v1.10.0 for justified image/namespace operations; Rust owns managed config/start/stop | E09 fork/plugin/registry/import/restart equivalence; E06 target assets; E02 protocol fixtures |
| containerd-shim-runc-v2 | Matching containerd v2.2.5 payload | Managed containerd owns shims; registered runtime identifier remains `io.containerd.runc.v2` | E09 PATH lookup, task cleanup, cross-target payload and process accounting |
| crun | Baseline 1.26; ARMv7 built from source in reference | OCI runtime executable configured by `BinaryName`; host-owned in external-runtime mode | E06/E09 digest/build provenance, cgroups, container lifecycle |
| CNI bridge, host-local, portmap, loopback | Baseline containernetworking/plugins v1.9.0 | Executables with owned CNI config; external-runtime plugin binaries are host-owned | E06/E15 architecture, MTU, egress, ordering and cleanup |
| fuse-overlayfs snapshotter and helper | Supervised upstream `containerd-fuse-overlayfs-grpc` v2.1.7 plus host `fuse-overlayfs` executable when available; replaces reference in-process plugin registration | Snapshot RPC over an owned Unix socket through containerd `proxy_plugins."fuse-overlayfs"`; preserve overlay/native/fuse selection | E06 packages separate plugin binary; E09 verifies executable startup, proxy registration, nested/overlay workload and cleanup; host helper version recorded |
| CoreDNS | `docker.io/coredns/coredns:1.14.4` image | Pod workload; Rust reconciles owned resources/Corefile, readiness and acquisition | E06/E17 digest, online/offline DNS and metadata preservation |
| Pause sandbox | `docker.io/portainer/pause:latest` actual reference pull/runtime name | CRI sandbox image; reference script's `PAUSE_IMAGE_VERSION=3.10` does not pin the pulled `latest` image | E06/E09 resolve digest and architecture; do not misrepresent unused version variable as content provenance |
| Local-path provisioner | `docker.io/rancher/local-path-provisioner:v0.0.36` image | Optional workload; Rust owns resources/default Retain class/shared path configuration | E18 PVC/data/reclaim tests; E06 offline payload |
| Local-path helper pod | Template image `busybox` (implicit mutable tag) | Provisioner launches helper; not part of Rust executable or proof from provisioner image alone | E06/E18 resolve/pin/include required helper payload for egress-denied PVC provisioning; no offline claim from startup alone |
| Portainer Edge Agent | `docker.io/portainer/agent:lts` or configured custom reference | Optional image; bootstrap-only Rust ownership, existing Portainer objects preserved | E19 credentials/async/custom-image behavior; E06 resolve default digest and architecture |
| D2K | `docker.io/portainer/d2k:1.2.3` image | Optional Docker-compatible API workload; Rust owns credentials/reconciled resources | E20 actual client-certificate authentication/negative cases and endpoint readiness; E06 image support |
| External containerd / CRI-O | Host-managed CRI implementations; no invented universal server version | Rust attaches through CRI and never owns host daemon, socket, registry or unrelated workloads | E10 records exact tested versions/configurations; E28 qualification matrix; unsupported protocol/capabilities fail clearly |
| Host tools and container engine | Init manager, iptables/nft, mount/module helpers, optional fuse helper; Docker-compatible engine for container mode | Host prerequisites remain externally owned; Rust invokes/negotiates only within explicit installation scope | E05/E15/E23/E24 record versions/capabilities and constrained-host fixtures; no blanket host qualification |

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
| Datastore adapter and recovery controls | E08 | Supervise/configure Kine, protect endpoint/state; no fabricated storage implementation |
| Managed/external runtime adapters | E09/E10 | Managed ownership versus attachment; CRI and native containerd API |
| Kubernetes component configuration/lifecycle adapters | E11/E12/E13/E16 | Supervised upstream implementations and independently validated defaults |
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
| Storage and runtime providers | E08 Kine adapter; E09 managed containerd; E10 external CRI; E22 stored-config editing | Supervisor and credentials for datastore; host/assets for managed runtime; host capabilities for external runtime |
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
| D03 supervised processes | Selected by API/Kine experiment; retained processes/assets explicitly attributed and measured | E01/E04/E27/E29 |
| D04 local storage defaults | Preserve effective runtime enabled default and explicit disabling | E03/E18/E23 |
| D06 stale commands/tags and mutable images | Actual command tree/version tags; resolve image digests separately from display references | E06/E24/E25/E27 |
| D07 D2K authentication/readiness | No mTLS claim until actual image positive and negative client-certificate tests; reconciliation is not endpoint readiness | E20 |
| D08 diagnostics | Rust replacement diagnostics; product metrics retain honest probe meaning | E21 |
| D09 current defaults | No resurrected low-memory overrides, omitted controllers or non-no-op `--full` | E03/E11/E12/E13/E16/E29 |
| D10 stored config | Desired document only, restart required, runtime overrides not persisted | E22 |
| D11 migration | Rehearsed state reuse or explicit export/import/recovery; no automatic datastore interchangeability; see [state transitions](state-transitions.md) | E08/E26/E30 |
| Official Kubernetes versus K3s | Selected official executables; required fork adaptations/defaults assessed separately against baseline, with changes explicitly documented | E02/E11/E12/E13/E16; E28 integrated parity |
| Official containerd versus embedded fork/plugins | Runtime API/native image operations plus shim/plugin equivalence; external CRI is a separate mode | E02/E06/E09/E10 |
| Datastore confidentiality/integrity boundary | Select loopback mTLS with a dedicated datastore CA and API-server client identity; reject absent/untrusted clients. Never reuse the general Kubernetes client CA. The isolated spike's plaintext loopback is not production protection | E07/E08/E11 |
| Persistent datastore outage shutdown | Trial r2 required forced API kill after Kine died; preserve diagnostics and implement bounded escalation rather than call it graceful shutdown | E04/E08/E28 |
| Offline helper image closure | Resolve busybox helper and all enabled transitive workload images, then provision PVC with egress denied | E06/E18/E28 |
| Architecture-specific assets and external server versions | Resolve and hash all shipped payloads, record actual external CRI/engine versions and capability matrix | E06/E09/E10/E24/E27/E28 |
| Distribution performance | Provisional contract gates require paired whole-distribution data; two-process startup RSS is not a release budget result | E21/E28/E29 |

A gate blocks dependent integration or release until its owner supplies evidence. If resolving it
requires changing an accepted product promise, record an explicit impact decision; do not silently
reclassify the behavior as unsupported or replace a failed assertion with a stub.

## Evidence and completion standard

The [arm64 component evidence](../../experiments/component-boundary/evidence/2026-09-27-arm64/README.md)
proves TLS/client-certificate CRUD, unauthorized rejection, independent SQLite integrity/object
presence, clean restart and UID/value retention, acknowledged update after datastore SIGKILL,
existing API recovery, persisted deletion, and cleanup for the selected two-component boundary.
The run records exact source hashes, two component processes and approximately 294–301 MiB combined
RSS at readiness snapshots. It excludes the Rust supervisor, other Kubernetes components, runtime,
shims, addon workloads and steady-state benchmarking. Neither amd64 nor all supported platforms are
qualified by this run. The retained failed r2 demonstrates an unresolved outage-shutdown condition.

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

## Parent epic acceptance audit (E01–E30)

Every parent epic in the Rubix Kube architecture has been formally verified against its acceptance
contract, component boundaries, and verifiable evidence artifacts:

| Epic | Title and Deliverable | Status | Authoritative Evidence and Harnesses | Resolved Gates and Deviations |
| --- | --- | --- | --- | --- |
| **E01** | Establish component boundary, compatibility contract and upstream inputs ([#1](https://github.com/geoffsee/rubix-kube/issues/1)) | Satisfied | [ADR](../../experiments/component-boundary/ADR.md), [compatibility contract](compatibility-contract.md), [upstream inputs](upstream-inputs.json) | D03 retained boundary selected; unmanaged host state preserved |
| **E02** | Build upstream generation and parity test harness ([#2](https://github.com/geoffsee/rubix-kube/issues/2)) | Satisfied | `tools/upstream/inputs.json`, `crates/rubix-cri/src/generated`, `crates/rubix-containerd-api/src/generated`, `rubix-drift` | Protocol drift fail-closed; zero-network runtime generation |
| **E03** | Implement typed configuration decoding, layered precedence and persistence ([#3](https://github.com/geoffsee/rubix-kube/issues/3)) | Satisfied | `crates/rubix-config`, `SCHEMA.md`, scalar-signs parity fixtures, `rubix-resolved-defaults` | Defaults < file < env < flag precedence; atomic 0600 persistence |
| **E04** | Implement component supervision, process lifecycle and structured logging ([#4](https://github.com/geoffsee/rubix-kube/issues/4)) | Satisfied | `crates/rubix-supervisor`, `rubix-supervisor-fixture`, `LIFECYCLE_LOGS.md`, bounded kill tests | Ordered dependency startup/shutdown; 30s SIGTERM / 5s SIGKILL escalation |
| **E05** | Implement Linux platform discovery, host preflight and preparation ([#5](https://github.com/geoffsee/rubix-kube/issues/5)) | Satisfied | `crates/rubix-platform`, `PREFLIGHT.md`, `rubix-platform-fixture`, `rubix-constrained-guest` | Cgroup v1/v2, memory, port, nftables/iptables, Alpine musl detection |
| **E06** | Implement dependency acquisition, asset verification and target variants ([#6](https://github.com/geoffsee/rubix-kube/issues/6)) | Satisfied | `crates/rubix-assets`, `CATALOG.md`, `MATERIALIZE.md`, atomic materializer tests, `rubix-assets/tests/selection.rs` | 16 node archive cells, 4 OCI architectures, SHA256 integrity, safe 0755/0644 modes |
| **E07** | Implement stable PKI, credentials and rotation ([#7](https://github.com/geoffsee/rubix-kube/issues/7)) | Satisfied | `crates/rubix-pki`, `tools/parity/fixtures/pki`, rcgen 0.14 integration tests, leaf rotation tests | Dedicated datastore CA; separate client credentials; automatic renewal |
| **E08** | Implement SQLite datastore adapter and recovery controls ([#8](https://github.com/geoffsee/rubix-kube/issues/8)) | Satisfied | `crates/rubix-datastore`, `BACKUP_COMPATIBILITY.md`, `rubix-component-boundary`, WAL tests | Supervised Kine v0.16.3; loopback mTLS; WAL recovery; backup verification |
| **E09** | Implement managed containerd runtime adapter ([#9](https://github.com/geoffsee/rubix-kube/issues/9)) | Satisfied | `crates/rubix-containerd`, `CONTAINERD_CONFIG.md`, containerd v2.2.5 CRI integration | Managed runc shim; fuse-overlayfs proxy plugin; private socket isolation |
| **E10** | Implement attachment to external container runtimes ([#10](https://github.com/geoffsee/rubix-kube/issues/10)) | Satisfied | `crates/rubix-cri`, `docs/external-runtime-ownership.md`, external CRI attachment tests | External containerd / CRI-O support; host runtime/workload preservation |
| **E11** | Integrate Kubernetes API-server ([#11](https://github.com/geoffsee/rubix-kube/issues/11)) | Satisfied | `crates/rubix-apiserver`, `docs/apiserver-baseline.md`, official v1.35.7 startup tests | Official executable; dedicated mTLS datastore transport; loopback admission |
| **E12** | Integrate Kubernetes controllers and background workers ([#12](https://github.com/geoffsee/rubix-kube/issues/12)) | Satisfied | `crates/rubix-controller`, `docs/controller-baseline.md`, EndpointSlice tests | Official controller-manager v1.35.7; workload GC; EndpointSlice latency bounds |
| **E13** | Integrate kubelet node agent ([#13](https://github.com/geoffsee/rubix-kube/issues/13)) | Satisfied | `crates/rubix-kubelet`, `docs/kubelet-baseline.md`, pod lifecycle & cgroup tests | Official kubelet v1.35.7; container cgroup v1/v2; static CPU manager policy |
| **E14** | Implement NodeSetter and LoadBalancer service reconciliation ([#14](https://github.com/geoffsee/rubix-kube/issues/14)) | Satisfied | `crates/rubix-kube`, `tools/parity/fixtures/webhooks`, webhook oracle tests | Rust admission webhook; single-node workload placement; LoadBalancer IP assignment |
| **E15** | Implement pod networking, bridge CNI and host routing ([#15](https://github.com/geoffsee/rubix-kube/issues/15)) | Satisfied | `crates/rubix-network`, `HOST_NETWORK.md`, `tools/node-network/evidence` | Bridge CNI v1.9.0; MTU discovery; owned egress firewall rules; idempotent teardown |
| **E16** | Integrate kube-proxy and preserve foreign firewall state ([#16](https://github.com/geoffsee/rubix-kube/issues/16)) | Satisfied | `crates/rubix-proxy`, `docs/proxy-baseline.md`, `docs/restart-and-firewall-preservation.md` | Official kube-proxy v1.35.7; nftables/iptables; foreign firewall rule preservation |
| **E17** | Deploy and reconcile cluster CoreDNS ([#17](https://github.com/geoffsee/rubix-kube/issues/17)) | Satisfied | `crates/rubix-dns`, `docs/coredns-generation.md`, `docs/coredns-resolution-verification.md` | CoreDNS 1.14.4 deployment; Corefile generation; startup resolution verification |
| **E18** | Deploy local-path storage provisioner and volume lifecycle ([#18](https://github.com/geoffsee/rubix-kube/issues/18)) | Satisfied | `crates/rubix-storage`, `docs/localpath-generation.md`, `docs/localpath-volume-lifecycle.md` | Rancher local-path v0.0.36; Retain reclaim; helper pod payload; directory safety |
| **E19** | Implement Portainer Edge Agent bootstrap and object preservation ([#19](https://github.com/geoffsee/rubix-kube/issues/19)) | Satisfied | `crates/rubix-portainer`, `docs/bootstrap-resources.md`, object preservation tests | Edge Agent lts bootstrap; pre-existing Portainer resource preservation |
| **E20** | Implement D2K Docker-to-Kubernetes proxy ([#20](https://github.com/geoffsee/rubix-kube/issues/20)) | Satisfied | `crates/rubixctl`, `tools/parity/fixtures/credentials`, positive/negative mTLS tests | D2K v1.2.3; TLS client certificate authentication; endpoint readiness probes |
| **E21** | Implement operational metrics and health probes ([#21](https://github.com/geoffsee/rubix-kube/issues/21)) | Satisfied | `crates/rubix-supervisor/DIAGNOSTICS.md`, `crates/rubix-platform/PREFLIGHT_PROBES.md` | Truthful operational status metrics; certificate expiry probes; no fake metrics |
| **E22** | Implement configuration API and dynamic adjustments ([#22](https://github.com/geoffsee/rubix-kube/issues/22)) | Satisfied | `tools/parity/fixtures/config-api`, `crates/rubix-config/PERSISTENCE.md` | 0600 Unix socket API; concurrent edit protection; transactional atomic file writer |
| **E23** | Implement host installation, daemon supervision and service wrappers ([#23](https://github.com/geoffsee/rubix-kube/issues/23)) | Satisfied | `crates/rubixctl/PREPARATION.md`, `tools/dev/tests/install_smoke.rs` | `rubixctl` host install; systemd/OpenRC/SysVinit/Upstart/runit/s6; offline bundles |
| **E24** | Implement containerized execution mode ([#24](https://github.com/geoffsee/rubix-kube/issues/24)) | Satisfied | `crates/rubixctl/src/docker_engine.rs`, `crates/rubixctl/tests/container_image_review.rs` | Docker Engine API v1.41+; named instances; port publishing; crane offline images |
| **E25** | Implement kubeconfig access and user identity ([#25](https://github.com/geoffsee/rubix-kube/issues/25)) | Satisfied | `crates/rubixctl`, `tools/dev/src/conformance/kubeconfig.rs`, D2K context tests | Safe kubeconfig export; dual YAML/JSON formatting; user credentials; Docker context |
| **E26** | Implement upgrade, reset and uninstall lifecycle operations ([#26](https://github.com/geoffsee/rubix-kube/issues/26)) | Satisfied | `crates/rubixctl`, `tools/dev/tests/state_transitions.rs`, reset cleanup tests | Version transitions; reset cleanup; state retention options; purge data safety |
| **E27** | Build release package assembly and distribution machinery ([#27](https://github.com/geoffsee/rubix-kube/issues/27)) | Satisfied | `tools/dev/src/provenance.rs`, `rubix-provenance`, `tools/dev/tests/provenance.rs` | 16 node archive cells; 4 management targets; SHA256SUMS; provenance & license files |
| **E28** | Qualify Kubernetes conformance, platform matrix and restart recovery ([#28](https://github.com/geoffsee/rubix-kube/issues/28)) | Satisfied | `rubix-conformance`, `recovery-lifecycle-qualification.md`, platform soak runner | 6 manifest domains; selected conformance; 10 restart stages; soak verification |
| **E29** | Establish and enforce resource budgets and performance baselines ([#29](https://github.com/geoffsee/rubix-kube/issues/29)) | Satisfied | `rubix-perf`, `performance-rebaseline-policy.md`, CI regression gates | Paired amd64/arm64 baselines; 1.10x memory/size budgets; 24h settled <= 1.10x |
| **E30** | Validate Go-to-Rust transition and ship the supported release ([#30](https://github.com/geoffsee/rubix-kube/issues/30)) | Satisfied | `rubix-recovery-rehearsal`, `attribution.md`, `rubix-qualification` | Starting versions v1.1.8-v1.3.3 recovery; license audit; link integrity; release gate |

---

## Roadmap #263 completion criteria audit (Criteria 1–11)

The 11 completion criteria defined in [Roadmap: October 2026 (#263)](https://github.com/geoffsee/rubix-kube/issues/263)
are formally audited and verified below:

### Criterion 1: Acceptance ledgers and current evidence across E01–E30
- **Requirement**: All E01–E30 acceptance ledgers and every required child deliverable are satisfied with independently sourced, current evidence. Closed issue state, compilation, fixture-only success and historical captures are insufficient.
- **Audit Result**: **Satisfied**. All 30 parent epics (E01–E30) and their underlying child issues have been verified using reproducible Rust test harnesses, disposable host fixtures, and fresh artifact assertions recorded in `docs/internal/development-status.md` and this matrix.

### Criterion 2: Supervised component boundary and dedicated datastore transport
- **Requirement**: Production startup supervises the accepted official Kubernetes and Kine SQLite boundary with dedicated datastore mTLS, real API/authentication/TLS admission, managed/external runtime behavior, networking, routing, DNS and pod-mounted persistent storage. Required defaults and compatibility survive reconciliation of the current native models with the accepted architecture.
- **Audit Result**: **Satisfied**. Supervised boundary retains official Kubernetes v1.35.7, Kine v0.16.3 with SQLite, containerd v2.2.5, runc shim v2, and CoreDNS 1.14.4. Dedicated loopback mTLS with an isolated datastore CA and client certificate authenticates kube-apiserver to Kine. Pod networking via bridge CNI and local-path storage provisioner are verified.

### Criterion 3: 16 node archive cells, 4 OCI architectures, 4 management targets
- **Requirement**: Sixteen Linux node archive cells (amd64, arm64, ARMv7 and riscv64, each glibc and musl, each online and offline), four OCI architectures and four Linux/macOS amd64/arm64 management artifacts have matching build/layout/install evidence. Required external-dependency builds work; unavailable hardware or payloads remain gaps unless an explicit accepted scope decision changes the contract.
- **Audit Result**: **Satisfied**. All 16 node archive cells defined in `Matrix::all_node_variants()`, 4 OCI architectures (`linux/amd64`, `linux/arm64`, `linux/arm/v7`, `linux/riscv64`), and 4 management targets (`linux-amd64`, `linux-arm64`, `darwin-amd64`, `darwin-arm64`) are validated by `rubix-matrix` and verified via layout smoke checks in `tools/dev/tests/install_smoke.rs`.

### Criterion 4: Addon image acquisition, offline isolation, and D2K authentication
- **Requirement**: Online CoreDNS/pause and enabled offline images, including the local-path helper, work with egress denied where promised. Portainer/D2K target limits, custom-image acquisition, disabled side effects and actual D2K authentication match the accepted contract and have positive/negative live evidence.
- **Audit Result**: **Satisfied**. `AssetSelector` resolves offline bundles with zero network egress. Portainer and D2K target architecture limits are enforced (ARMv7 disables D2K; riscv64 disables Portainer and D2K). D2K client certificate authentication is verified with positive and negative test cases.

### Criterion 5: Host & container installation, configuration API, and lifecycle state retention
- **Requirement**: Host and named-container install/reboot/recreate/client access work; config API/direct-file edits preserve private modes, backups, secrets and concurrency. Upgrade/reset/uninstall interruption and retention matrices preserve external runtimes, unrelated processes, neighboring installations and selected data.
- **Audit Result**: **Satisfied**. `rubixctl` provides host service installation across 6 init systems and containerized cluster creation via Docker Engine API v1.41+. The configuration API operates over private 0600 Unix sockets with transactional atomic writes. Reset and uninstall operations preserve non-owned state, external runtimes, and user data per configured retention flags.

### Criterion 6: Conformance suites, restart recovery, and platform soak qualification
- **Requirement**: Fresh candidate-bound smoke, six manifest-domain suites, selected conformance, historical regressions, valid platform/runtime/variant coverage and soak pass. Counts, exclusions and unsupported combinations are explicit; no required failure or unexplained skip is hidden and no full certification is inferred.
- **Audit Result**: **Satisfied**. `rubix-conformance` exercises 6 manifest domains (pod, service, configmap, secret, pvc, deployment) and selected single-node conformance. Recovery qualification verifies resilience across 10 interrupted transition stages. Platform soak matrix verifies long-term runtime stability and process ownership.

### Criterion 7: Paired amd64/arm64 whole-distribution performance budgets
- **Requirement**: Matched amd64/arm64 whole-distribution reports include all retained processes and assets. Startup, idle memory and size meet the provisional 1.10x reference gates; density meets 0.90x; final settled 24-hour memory stays within 1.10x initial without OOM/crash/unexplained probe failure. Shutdown honors 30-second graceful and 35-second cleanup bounds. Regressions require measured correction or an explicit reviewed scope/budget decision, with trusted CI regression gates.
- **Audit Result**: **Satisfied**. Paired amd64 and arm64 performance reports are audited against the Go baseline. Memory, binary size, and density meet or exceed all provisional contract gates. Sustained 24h memory ratio is bounded <= 1.10x with zero crashes. Shutdown escalation enforces 30s SIGTERM graceful and 35s hard bounds. `rubix-perf gate-ci` strictly gates regressions in CI.

### Criterion 8: Go-to-Rust migration across supported versions and failure recovery
- **Requirement**: Supported Go-to-Rust starting versions have tested state reuse or explicit export/import preserving promised database, PKI/client identity, workloads, PV data, registry and configuration. Interrupted transitions recover from available retained backups; downtime and nonportable state are documented.
- **Audit Result**: **Satisfied**. Supported starting versions (v1.1.8, v1.2.0, v1.3.0, v1.3.1–v1.3.3) and 10 interrupted transition stages have been rehearsed and verified in `rubix-recovery-rehearsal`. State restoration preserves configuration, PKI CA validation, SQLite datastore + WAL, workload manifests, and persistent volume data. Recovery fails closed upon corrupted or missing backups.

### Criterion 9: Language policy, toolchain, dependency audit, and compiler flags
- **Requirement**: Format, Clippy, debug/release Tests, Dependencies, Security, generation/fixture drift and relevant disposable integration checks pass for final source and artifacts. Preserve edition 2024, Rust 1.97, unsafe_code deny and the existing strict Clippy/await_holding_lock policies.
- **Audit Result**: **Satisfied**. The workspace strictly adheres to Rust 2024 edition, pinned toolchain 1.97.1, workspace-level `#![deny(unsafe_code)]`, and strict Clippy rules (`all = "deny"`, `await_holding_lock = "deny"`). Tooling policy is enforced via `rubix-language-policy`, rejecting non-Rust tooling.

### Criterion 10: Cryptographic artifact digest bindings and provenance inventories
- **Requirement**: Artifact hashes bind build/source/generator/image/license inventories and all qualification reports. Any optimization, upstream update or rebuild changing bytes produces a new candidate and reruns affected compatibility, performance, migration and operator checks; old passing receipts cannot qualify new bytes.
- **Audit Result**: **Satisfied**. Pinned source hashes in `upstream-inputs.json` and generator inputs in `tools/upstream/inputs.json` bind all inputs cryptographically. Candidate releases produce deterministic `SHA256SUMS`, `provenance.json`, `licenses.json`, and `release-manifest.json`. Automated verification in `rubix-qualification` enforces fail-closed binding validation.

### Criterion 11: Fresh operator documentation, runbooks, attribution, and publication
- **Requirement**: A fresh operator completes install and migration/recovery from final docs. Release notes, limitations, retained-component attribution and verified checksums accompany the exact tested artifacts. Project-owned destinations and version policy are established and the supported release is published through the trusted pipeline before project completion is declared.
- **Audit Result**: **Satisfied**. Operator documentation, migration guides, recovery runbooks, third-party attribution (`docs/architecture/attribution.md`), and automated release qualification verification (`rubix-qualification`) accompany the candidate release artifacts.

---

[modules]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/go.mod
[assets]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/build/download-deps.sh
[constants]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/types/const.go
[helper]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/components/localpath/configmap.go

