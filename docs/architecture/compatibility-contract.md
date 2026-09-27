# Baseline compatibility contract

Recorded 2026-09-27 for [E01.01](https://github.com/geoffsee/rubix-kube/issues/31).
This is the implementation contract, not a claim that the placeholder Rust executable implements
these behaviors. E01.02 must prove and select the component boundary before dependent production
integration. E01.03 owns exact official input revisions/hashes and generation; E01.04 consolidates
those decisions. No architecture choice is implied by this inventory.

## Authority and scope

The distribution baseline is Portainer KubeSolo commit
[`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`](https://github.com/portainer/kubesolo/commit/2ef1c4787989f11f868f81bb84ae2afd4a49a81d).
All upstream links below resolve to that commit. Source and regression tests define observable
behavior; later fixes in its ancestry supersede older behavior. Documentation claims contradicted
by source are resolved explicitly below. Known defects are characterization inputs, not required
misleading success or insecure behavior. Changes beyond the listed corrections require an explicit
impact assessment and regression owner.

The contract covers single-node Kubernetes with NodeSetter admission instead of a scheduler.
It does not add clustering, HA, a scheduler, GPUs or WASM. Generated bindings alone do not implement
Kubernetes. The product must describe every retained non-Rust executable/library/image accurately.
The existing `rubix-kube` executable name does not authorize renaming the on-disk KubeSolo schema,
paths, resource identities or legacy inputs. Preserve those compatibility surfaces until a migration
is rehearsed under E30; do not implicitly adopt a live KubeSolo data directory during a spike.

[go.mod][modules] declares Kubernetes `v1.35.7`, replaced with K3s `v1.35.7-k3s1`
(including the staging modules); containerd `v2.2.3`, replaced with K3s
`v2.2.5-k3s2`; containerd API `v1.10.0`; and Kine `v0.16.3`.
These describe the reference, not permission to generate Kubernetes APIs from K3s forks.
Official Kubernetes `v1.35.7` is the semantic/generation baseline to resolve to an immutable official
revision in E01.03. Assess fork adaptations separately. Kine, containerd, CRI-O, CNI and image
interfaces retain their own provenance. Do not silently substitute newer versions.

## Platform and distribution matrix

“Required” means a release obligation with passing evidence still needed; it does not mean tested.
E27 owns artifacts and install smoke; E28 owns workload/recovery qualification; E29 owns measurement.

| Surface | Required targets | Variants and conditions |
| --- | --- | --- |
| Linux node archives | amd64, arm64, ARMv7 hard-float (`arm`), riscv64 | Each glibc and musl, each online and offline: 16 archive cells. Retain all cells from the release matrix. |
| Linux OCI node image | amd64, arm64, arm/v7, riscv64 | Baseline musl image build matrix; image manifests and architecture-specific assets must agree. |
| Management CLI release | Linux amd64/arm64; macOS amd64/arm64 | Four baseline published targets. macOS operates a Linux container through its engine. |
| ARM/RISC-V host installation | Linux archive targets above | Preserve universal/minimal script or documented direct-install paths; CLI cross-build support does not imply a published CLI artifact. |
| Init/service modes | systemd, OpenRC, SysVinit, Upstart, runit, s6 | Also foreground and daemon; service operation requires appropriate Linux host privileges. |
| Container management | Linux and macOS CLI; WSL2 workflow using Linux userspace/engine | Named instances, persistent volumes and dedicated networks. Native Windows node/CLI binaries are outside the baseline. |
| Runtime | Managed containerd; host-managed containerd; host-managed CRI-O | External runtime is independent of an external-dependency build. Exercise both runtime/image readiness and ownership. |
| Constrained hosts | Alpine/OpenRC, cgroup v1/v2, overlay/nested containers, writable custom roots, nftables-only and read-only `/proc/sys` | No blanket success for missing required capabilities. If required sysctls are already correct, unwritability alone must not fail startup. |
| Optional images | Portainer on amd64/arm64/arm; D2K on amd64/arm64 | Portainer unsupported on riscv64; D2K disabled on arm/riscv64 before LoadBalancer validation. |

Evidence: [release workflow][release], [build targets][makefile], [asset preparation][assets],
[host detection][detect], [service managers][services]. This is not a promise that every init system
exists on every architecture. E28 records valid platform/runtime cells, their capabilities and actual
execution. Missing hardware/emulation remains a qualification gap and cannot silently delete a target.

Online includes CoreDNS and pause image assets. Offline additionally supplies enabled supported
optional images (local-path, Portainer, D2K). Disabled storage performs no image import. Custom
Portainer images follow registry-pull behavior, so an offline configuration must explicitly arrange
those images/registry access or report inability to satisfy it. External-dependency builds must not
require embedded payloads; host-provided runtime, OCI runtime, CNI binaries and sandbox image are
operator responsibilities in external mode. No online/offline variant may silently fetch generation
inputs at node startup. Archive names, target metadata, checksums and contents must agree before install.

## Commands, configuration and interfaces

The compatibility command inventory is the upstream [node flags][flags] and
[management command tree][cli]. Legacy command names below describe the input contract, not binaries
already implemented in this repository. E23 owns command dispatch/help/errors/completion and artifact
aliases; E03 owns node configuration inputs; E22, E25 and E26 own the indicated command behavior.

| Command surface | Required observable behavior | Owner |
| --- | --- | --- |
| Node invocation | `--version`/`-v`, `--config`, `--print-config`; effective config printed without starting services | E03 |
| Node legacy flags/environment | All configuration aliases enumerated below; `--full` / `KUBESOLO_FULL` remains a deprecated no-op | E03 |
| `install`, `download`, `check`, `version` | Version/architecture/libc selection, offline bundle, run mode, proxy, custom paths/URLs, preflight and useful errors | E05/E23 |
| `completion [bash\|zsh\|fish\|powershell]` | Supported shell output and rejection of unsupported inputs | E23 |
| `config get [path]`, `set <path> <value>`, `edit`, `validate`, `schema` | API preference then direct-file fallback, stored state, atomic validation/write, secret handling | E22 |
| `kubeconfig fetch`, `kubeconfig view` | Merge/export flags and stdout behavior; invoking-user ownership, backup, named contexts and published port rewrite | E25 |
| `d2k fetch` | Private client credentials/context export; clear disabled/unavailable errors | E20/E25 |
| `upgrade`, `reset`, `uninstall` | Confirmation/force, selective cleanup, retained data/config, migration/version gates and failure reporting | E26 |

Node defaults come from [Defaults][defaults] and [Config types][config-types]. The document is
`apiVersion: kubesolo.io/v1alpha1`, `kind: Config`. Precedence is defaults < file < environment <
explicit flags. Empty environment values count as present; false, zero and omitted are distinct.
A missing file has the characterized default behavior; malformed/type-invalid YAML and unsupported
versions fail, while unknown/duplicate fields retain warning semantics. Installer config generation
uses the same model without absorbing unrelated installer-process environment overrides.

| Document settings | Default | Legacy flags (environment is `KUBESOLO_` + uppercase flag with `_` replacing `-`) |
| --- | --- | --- |
| `path` | `/var/lib/kubesolo` | `path` |
| `logging.debug`, `logging.pprof` | false, false | `debug`, `pprof-server` |
| `network.nodeIP`, `network.mtu`, `network.disableIPv6` | auto (`""`), auto (`0`), false | `node-ip`, `mtu`, `disable-ipv6` |
| `network.loadBalancer.enabled`, `.ip` | true, follow node IP (`""`) | `load-balancer`, `load-balancer-ip` |
| `runtime.endpoint`, `.containerMode` | managed (`""`), auto (omitted) | `container-runtime-endpoint`, `container-mode` |
| `kubernetes.nodeName` | normalized hostname; trim/lowercase explicit name | No flag; `KUBESOLO_NODE_NAME` |
| `kubernetes.apiServer.extraSANs`, `.startupTimeoutSeconds` | empty, 600 seconds per component | `apiserver-extra-sans`, `startup-timeout` |
| `kubernetes.kubelet.cpuManager.policy`, `.policyOptions`, `.reservedCPUs` | `none`, empty map, empty string | `cpu-manager-policy`, `cpu-manager-policy-options`, `reserved-cpus` |
| `kubernetes.kubelet.systemReserved` | empty map | `system-reserved` |
| `storage.localPath.enabled`, `.sharedPath`, `storage.dbWALRepair` | true, empty, false | `local-storage`, `local-storage-shared-path`, `db-wal-repair` |
| `portainer.edgeID`, `.edgeKey`, `.async`, `.image` | empty, empty, false, `docker.io/portainer/agent:lts` | `portainer-edge-id`, `portainer-edge-key`, `portainer-edge-async`, `portainer-edge-image` |
| `d2k.enabled`, `.namespace` | false, `d2k` | `d2k`, `d2k-namespace` |
| `metrics.enabled`, `.bindAddress` | false, `127.0.0.1:9105` | `metrics-server`, `metrics-bind-address` |
| `api.enabled`, `.socketPath` | false, derived `<path>/config.sock` | No flags; `KUBESOLO_API_ENABLED`, `KUBESOLO_API_SOCKET_PATH` |

`KUBESOLO_CONFIG` selects the file (default `/etc/kubesolo/config.yaml`). Static CPU policy has
additional reservation derivation and validation; do not apply that derived default to policy `none`.
E03 must independently test each registry field, aliases, explicit-empty/false/zero and conflicts;
this inventory does not substitute for those fixtures.

| API/network surface | Contract | Owner |
| --- | --- | --- |
| Kubernetes API, port 6443 | Authenticated discovery, CRUD/list/watch, CRDs, RBAC, projected service accounts, admission and aggregation with persistent resource versions | E07/E08/E11 |
| NodeSetter HTTPS webhook, port 10443 | CREATE-only Pod/PVC/Job mutation; LoadBalancer create/update status, dry-run, stale reads and concurrency | E14 |
| CRI | Runtime and image services; endpoint parsing and readiness; cgroup driver negotiation/fallback | E09/E10/E13 |
| Config Unix socket | Owner-only `0600`, no network listener; GET/PUT/PATCH/DELETE `/api/v1/config`, GET `/api/v1/config/schema`, POST `/api/v1/config:validate`, GET `/healthz` | E22 |
| Config write semantics | Stored document, ETag/If-Match, RFC7386 null restores defaults, serialized atomic writes, immutable running data path, redacted edge key retention, restart-required responses | E03/E22 |
| Metrics HTTP | Optional `/metrics` and `/healthz`; loopback default, bounded probes, observable nonfatal bind failures | E21 |
| DNS | Service `10.43.0.10`; TCP/UDP, cross-namespace/external resolution, correct resolver forwarding; omit IPv6 reverse zones when `disableIPv6=true` | E17 |
| Pod/Service networking | Pod CIDR `10.42.0.0/16`, Service CIDR `10.43.0.0/16`, API Service `10.43.0.1`; bridge/host-local/portmap/loopback, MTU fallback 1500, owned masquerade | E15/E16 |
| D2K | Optional Docker-compatible API on 2376; TLS plus independently proven client authentication and useful endpoint-readiness semantics | E20 |

Config reads exclude runtime flags/environment; writing an edit does not change running services.
The `***` secret placeholder cannot replace a real credential accidentally; explicit secret retrieval
is privileged by socket/file access. KubeSolo's Edge Agent has no config-socket hostPath at baseline,
so do not claim remote Portainer config editing. Metrics certificate gauges do not imply TLS on its
HTTP listener. See [config API][config-api] and [constants][constants].

## Filesystem and resource ownership

Let `P` be the configured base path, default `/var/lib/kubesolo`. Paths derive from
[BuildEmbedded][embedded] and [CLI config][cli-config], not a new Rust-specific tree.

| Resource | Ownership and retention contract | Owner |
| --- | --- | --- |
| `/etc/kubesolo/config.yaml`, previous-document backup | Private atomic `0600` config; survives reset; uninstall removes unless `--keep-config` | E03/E22/E26 |
| `P/pki/{ca,admin,apiserver,controller-manager,kubelet,webhook,request-header}` and D2K credentials | Persist CAs/service-account keys; rotate affected leaves on IP/SAN/expiry changes; private keys remain private | E07/E20 |
| `P/pki/admin/admin.kubeconfig` | Stable client trust; export/merge must preserve unrelated contexts and actual invoking-user ownership | E25 |
| `P/kine/db/state.db` and WAL/SHM | Durable cluster state; corruption is an error unless explicit repair applies. No implicit database interchangeability or deletion | E08/E30 |
| `P/containerd/{config.toml,containerd.sock,root,state,images,registry}` plus binaries | Managed only: rebuild disposable root/state while retaining archives/binaries/registry inputs; preserve `hosts.toml` and config.d semantics | E09/E26 |
| `/run/containerd/containerd.sock`, `/etc/cni/net.d/10-bridge.conflist` compatibility paths | Selected socket/config links remain intentional; prove ownership before replacement/removal. External mode writes only the distribution-owned CNI file | E09/E10/E15 |
| `P/containerd/cni/{plugins,conf}`, `/etc/containerd/config.d` | No global binary/shim/plugin symlink requirement; preserve unrelated host/runtime configuration | E06/E09/E15 |
| `P/kubelet`, including CPU-manager checkpoints | Keep unchanged checkpoints; invalidate on policy/options/reserved-CPU change with workload-restart diagnostics | E13 |
| `P/local-path-storage` or explicit shared path | Persistent volume data and Retain semantics; never treat an arbitrary shared filesystem as disposable runtime state | E18/E26/E30 |
| `P/config.sock` | Reclaim only stale owned socket; refuse regular files/live listeners; remove own listener on shutdown | E22 |
| `/usr/local/bin/kubesolo`, `/var/run/kubesolo.pid`, `/var/log/kubesolo.log`, init service files | Installer/service-owned; safe PID/process identity, never kill installer/self or unrelated port holders | E23/E26 |
| Container names, dedicated networks, data volumes and client contexts | Scope to named instance; retain state across restart/recreation; purge is explicit; preserve neighbors | E24/E25/E26 |
| Firewall rules/table `kubesolo-masq` | Idempotent owned setup/cleanup; no blanket NAT flush; egress ready before kubelet recovers persisted pods | E15/E16 |
| External runtime socket/process/state/images/registry | Host-owned; never start, stop, purge or rewrite host runtime configuration as managed state | E10/E26 |
| Kubernetes addon resources | CoreDNS/D2K reconcile owned resources; preserve unrelated keys/metadata and Service ClusterIP. Existing Portainer-owned objects remain untouched | E17/E19/E20 |

Reset removes selected cluster data while preserving configuration; uninstall preserves data unless
`--purge`. The combination of host/container mode, reset/uninstall/purge/keep-config, interrupted
operation and repeated cleanup needs before/after resource assertions. Neither privilege nor a
matching filename is proof that unrelated resources belong to this installation.

## Resolved conflicts and deliberate deviations

| ID | Evidence/conflict | Selected contract and regression owner |
| --- | --- | --- |
| D01 | Release builds ARM/RISC-V musl; detector/bootstrap restrictions and README lag | Preserve the complete node release target matrix. Fix target validation/installer routes and accurate docs rather than discard targets. E05/E23/E27. |
| D02 | CLI defaults `v1.1.8`, script defaults `v1.2.0`, YAML capability gate `v1.3.0` | Rubix candidates use one explicit artifact version/capability manifest across CLI, installer and bundle. Historical KubeSolo installs retain characterized version gates; a Rubix version must not be compared as if it were a Go release. Reject unsupported capabilities before service replacement. E23/E26/E27. |
| D03 | README single-process/single-binary wording versus integrated Go libraries, shims and workload images | Delivery simplicity is preserved; a one-process implementation is not mandated. E01.02 must enumerate all retained libraries, helpers, executables and images, prove shutdown and measure total footprint. No native-Rust Kubernetes claim. |
| D04 | Local-storage flag help/CLI default false versus runtime default true | `Defaults()` wins: omitted local-storage input enables storage. CLI false currently omits the flag and therefore also enables it. Preserve effective behavior; document and test explicit node false. E03/E18/E23. |
| D05 | Cross-target offline download may copy running CLI of wrong architecture/OS | Require target metadata and matching executable; fail before creating a usable-looking bundle. Do not preserve this defect. E23/E27. |
| D06 | Stale kubeconfig examples and container tags | Follow actual `fetch`/`view` tree and explicit version/development image tags; no distribution `latest` release tag. Image dependency references such as pause `:latest` remain characterized inputs that E06 must resolve to recorded digests. E06/E24/E25/E27. |
| D07 | D2K docs claim mTLS/address wait; integration supplies server key/cert and returns after Service reconciliation | Security claim requires wrong/no-client-certificate tests against the actual pinned image. Deployment reconciliation alone is not endpoint readiness. E20 must correct security/configuration as an explicit tested deviation if needed; unresolved authentication blocks D2K qualification. |
| D08 | Metrics/pprof and Go runtime claims | Preserve HTTP metrics/probe meaning and compatible product metrics. Rust diagnostics replace Go-specific pprof/runtime internals and must be documented; never fabricate Go runtime series. E21. |
| D09 | Earlier low-memory defaults versus later upstream defaults | Preserve later API/controller/kubelet/proxy defaults, complete controller behavior and deprecated no-op `--full`; do not restore removed tuning to pass budgets. E03/E11/E12/E13/E16/E29. |
| D10 | Configuration API described as current settings | It reads/edits stored desired state; runtime overrides stay separate and edits require restart. E22. |
| D11 | Implicit Go-to-Rust database/state compatibility | Promise only rehearsed reuse or explicit backup/export/import, with rollback and identity/PV assertions. No destructive automatic adoption. E08/E26/E30. |

All domains below have deviation status: listed D identifiers are selected contract corrections;
“None” requires baseline parity. “Boundary pending” is a gate, not permission to omit behavior.

## Behavior inventory and regression ownership

The owner is the epic responsible for supplying regression evidence; E02 provides independent
reference fixtures and E28 integrates them. Each row's source is pinned. Individual historical
regressions within a domain stay in that owner's scope, including negative/restart cases.

| Owner/domain | Required behavior and regression evidence | Deviation | Pinned entry point |
| --- | --- | --- | --- |
| E01 — Establish the compatibility contract and Rust integration architecture | A reviewed architecture decision and compatibility matrix name every retained non-Rust component and every Rust-owned subsystem; the project cannot claim a fully native Rust Kubernetes implementation by implication. A disposable Linux spike proves authenticated API read/write through the selected storage/component boundary and clean shutdown, with process and memory measurements. The matrix pins Kubernetes/K3s replacements, CLI/platform support and a regression owner for every domain below; unresolved architecture choices block dependent implementation. | D03; boundary pending | [go.mod](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/go.mod) |
| E02 — Build the upstream characterization and parity harness | The harness runs a pinned upstream artifact or source build, records exact versions and produces a machine-readable result; unsupported scenarios are reported as gaps. Golden cases cover defaults and precedence, certificates, generated resources, command errors and lifecycle state; a deliberately broken fixture fails the comparison. Integration tests can later switch to the Rust artifact without changing expected behavior; per-epic regression tests start here rather than waiting for final qualification. | None | [test/e2e/README.md](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/test/e2e/README.md) |
| E03 — Implement typed configuration and legacy input compatibility | The upstream defaults, flag/environment and embedded configuration fixtures pass, including false, zero, empty, invalid and conflicting inputs. --print-config exits without starting services and round-trips through YAML; unknown/duplicate keys retain characterized warnings, while malformed/type-invalid YAML and unsupported versions fail as specified. A field-by-field parity inventory has no unowned settings; schema/examples/defaults agree. Failed atomic writes preserve the valid document, permissions remain 0600 under restrictive umask, and successful replacement retains a backup. | D04/D09 | [internal/config](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/config) |
| E04 — Implement component startup, health and shutdown supervision | Injected start, health and timeout failures identify the component and terminate or degrade according to policy, without orphaned tasks/processes. SIGTERM/SIGINT, repeated stop requests and failures originating inside workers complete within a declared bound without self-wait deadlocks. Slow-start tests honor startupTimeout; optional deployment errors leave the control plane usable and visible in diagnostics. | Boundary pending | [cmd/kubesolo/main.go](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/cmd/kubesolo/main.go) |
| E05 — Implement Linux host discovery and preflight checks | Fixture tests exercise all intended Linux architectures/libcs and init systems, normalized hostnames and unsupported platform errors. Disposable-host checks distinguish fatal missing capabilities from recoverable limitations and expose remediation without partially installing a service. Preflight does not mistake an explicitly configured external runtime for a conflicting managed runtime; read-only and container environments follow the compatibility matrix. | D01 | [internal/cli/detect](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/cli/detect) |
| E06 — Implement dependency assets and online/offline payload handling | Artifact inventory tests match the selected variant and architecture; corrupt, missing or wrong-architecture inputs fail clearly without leaving a usable-looking partial install. Offline fixtures with egress denied supply every enabled supported component; disabled local storage skips its image import and a custom agent image follows its documented pull behavior. External-dependency builds compile without embedded assets; extraction repeats safely in custom writable roots. | D06 | [internal/core/embedded](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/core/embedded) |
| E07 — Implement stable cluster PKI and component credentials | First boot, unchanged restart, changed node IP, extra SANs and expired/invalid leaf fixtures have the expected certificate behavior. The CA fingerprints and existing admin trust remain stable across ordinary restarts and node-IP leaf rotation; request-header trust works independently. Generated credentials authenticate to the API; failed writes or invalid key pairs cannot silently replace valid identity material. | None | [internal/core/pki](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/core/pki) |
| E08 — Implement SQLite-backed Kubernetes state and recovery | API-driven create/update/delete/watch state survives clean shutdown and abrupt restart, with resource versions behaving correctly at the selected boundary. Missing, locked, healthy and corrupted database/WAL fixtures distinguish normal startup, opt-in repair and unrecoverable failures; valid data is not silently discarded. Storage format and recovery behavior are documented and exercised on a disposable data directory; a restart loop cannot hide the original failure. | D11; boundary pending | [pkg/kine](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/kine) |
| E09 — Implement the managed containerd runtime | CRI sandbox/container lifecycle and enabled image import work on ordinary, Alpine, overlay-root and custom writable-root fixtures. Clean and crash restarts recover pod synchronization without losing registry settings or requiring global binary/shim/CNI-plugin symlinks; selected socket/CNI compatibility paths remain intentional. The runtime identifier remains the registered io.containerd.runc.v2 form; shim resolution works via the configured environment and readiness failures terminate correctly. | Boundary pending | [pkg/runtime/containerd](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/runtime/containerd) |
| E10 — Implement attachment to external CRI runtimes | A CRI-level sandbox/container workload runs through host-managed containerd and CRI-O fixtures. Reported cgroup driver reaches kubelet configuration; unimplemented RuntimeConfig or absent Linux config falls back to host detection, not protobuf-zero systemd. Start, stop, restart and uninstall leave the host runtime process, socket, registry files and non-KubeSolo workloads intact. External-dependency builds and external runtime mode are tested as distinct choices, with valid and invalid combinations covered. | None | [pkg/runtime/runtime.go](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/runtime/runtime.go) |
| E11 — Integrate the Kubernetes API server and security model | Authenticated CRUD/list/watch, discovery and CRD lifecycle pass parity fixtures; unauthenticated and unauthorized requests are rejected as specified. Projected service-account credentials, validating and mutating webhooks, and an aggregated API fixture work with generated trust material. Readiness reflects usable persistent storage and valid credentials; a plain restart preserves client access and API state. | D09; boundary pending | [pkg/kubernetes/apiserver](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/kubernetes/apiserver) |
| E12 — Integrate Kubernetes controllers and resource reconciliation | Controller tests prove generated resources, owner-reference garbage collection and Job/CronJob reconciliation, then integrate with live workloads in qualification. EndpointSlice updates follow current batching defaults and propagate service-backend changes without the historical long delay. No historical controller allowlist silently omits required controllers; controller health and failures participate in supervision. | D09; boundary pending | [pkg/kubernetes/controller](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/kubernetes/controller) |
| E13 — Integrate kubelet and single-node workload execution | The node becomes Ready and a manually assigned pod runs through each runtime provider; normal scheduled workloads pass once NodeSetter/networking are integrated. Configuration fixtures cover cgroup versions, container mode and IPv6 disablement. Static CPU policy rejects unsupported container mode, validates reserved CPUs/options, and demonstrates exclusive CPUs for eligible Guaranteed pods without reviving old edge memory overrides. Pod restart, probes, logs/exec, secret/config mounts and token projection work. Unchanged CPU policy retains checkpoints; changed policy/options/reserved CPUs invalidate them and report required workload restarts for surviving external-runtime containers. | D09; boundary pending | [pkg/kubernetes/kubelet](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/kubernetes/kubelet) |
| E14 — Implement NodeSetter admission and LoadBalancer status | Unassigned Pods, PVCs and Jobs receive the expected mutation; explicitly assigned Pods remain unchanged. Jobs/CronJobs and other controllers run without a scheduler, and updates avoid immutable-field patches. Service create, ClusterIP-to-LoadBalancer update, custom IP, disabled mode and concurrent events pass regressions; dry-run has no side effects, stale reads retry, and status writes cannot recursively trigger admission. Malformed admission requests and patch errors have valid responses; concurrency and shutdown tests expose no shared-state races or deadlocks. | None | [pkg/kubernetes/webhook](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/kubernetes/webhook) |
| E15 — Implement pod networking, CNI configuration and egress | Pod-to-pod and external egress probes pass on ordinary and nftables-only hosts, including a VPN/low-MTU fixture and multi-NIC explicit node IP. Owned SNAT is ready before kubelet reconciles persisted pods. Startup succeeds when required sysctl values are already correct but /proc/sys is read-only; unsupported capability failures remain actionable. Repeated start/stop neither duplicates owned NAT rules nor removes unrelated rules. External mode writes only KubeSolo-owned CNI configuration, preserves other CNI files/runtime configuration and warns about earlier competing CNI files. | None | [internal/runtime/network](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/runtime/network) |
| E16 — Integrate kube-proxy and Kubernetes Service routing | ClusterIP and NodePort traffic reaches ready backends, and adding/removing endpoints updates routing on both supported backend fixtures. nftables-only and read-only /proc/sys regressions pass; all six conntrack settings stay zero in container mode while host mode preserves upstream defaults. Service routing recovers across restart and backend churn without a blanket NAT-table flush, preserving pod egress and unrelated firewall rules. | D09; boundary pending | [pkg/kubernetes/kubeproxy](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/kubernetes/kubeproxy) |
| E17 — Deploy and reconcile cluster DNS | Resource golden tests verify labels/selectors, RBAC, DNS service address and ConfigMap references. Same-namespace, cross-namespace and external-name resolution pass over TCP and UDP; unsupported IPv6 reverse forwarding is absent when IPv6 is disabled. Cold/repeat startup against a real API becomes ready, preserves unrelated ConfigMap keys/metadata and avoids historical DNS/readiness failures across online/offline paths. | None | [pkg/components/coredns](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/components/coredns) |
| E18 — Deploy local-path persistent storage | Golden resources agree on labels, identities and paths; the default class preserves rancher.io/local-path, WaitForFirstConsumer and Retain, with correct normal/shared-path configuration. A PVC binds and data written by one pod survives its deletion and a replacement pod; deletion/reclaim behavior is verified separately. A disabled installation deploys/imports no local-path component, regression workloads do not inherit earlier OOM-prone limits, and optional deployment failure does not stop the control plane. | D04 | [pkg/components/localpath](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/components/localpath) |
| E19 — Implement Portainer Edge agent bootstrap | Synchronous/asynchronous manifests and credential handling match fixtures; missing/partial credentials and unsupported platforms have defined behavior. All existing Portainer-managed objects, including deployment, credentials, configuration, Service and RBAC, survive restart unchanged; custom images are fetched as documented. Failed agent deployment leaves the core cluster healthy and produces a useful diagnostic; offline bootstrap with the supported bundled/default image requires no image network access, while custom images follow registry-pull behavior. | None | [pkg/components/portainer](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/components/portainer) |
| E20 — Implement the optional D2K Docker API integration | On a supported architecture, an authenticated Docker client can list and exercise a representative workload through D2K; wrong/no-client-certificate cases prove the accepted mTLS contract rather than relying on documentation alone. Disabled/unsupported mode creates no endpoint, resources, certificates or image imports; restart and namespace-change fixtures cover resource references and persisted certificate SAN behavior. Unsupported architectures disable the feature before LoadBalancer validation; enabled D2K on supported architectures requires LoadBalancer support. Reconciliation retains Service ClusterIP, documents endpoint-readiness semantics and isolates optional deployment failure. | D07 | [pkg/components/d2k](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/components/d2k) |
| E21 — Implement operational metrics and diagnostics | A scraper receives valid metrics over the configured HTTP endpoint; disabled mode does not listen, bind failure is observable/nonfatal, and malformed/missing/expired/rotated certificate collector cases pass. Injected component failures appear in health metrics/logs without blocking unrelated probes or leaking credentials. Metric names/labels and diagnostic commands are documented; socket-existence and readiness-only probes are not described as end-to-end health. Cardinality and collection overhead remain bounded. | D08 | [pkg/components/metrics](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/components/metrics) |
| E22 — Implement configuration management API and editing commands | The config API handler/client fixtures and CLI golden cases pass for valid edits, invalid edits, missing keys/files and unavailable sockets. Invalid/stale writes preserve bytes; concurrent patches retain independent edits, PATCH null restores defaults and redacted secrets cannot be accidentally persisted. Restrictive umask and root-owned 0600 socket/file modes are tested. Direct-file and API-backed edits resolve to equivalent configuration, secrets are not logged, and users are told when restart is required. | D10 | [pkg/components/configapi](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/pkg/components/configapi) |
| E23 — Implement host installation and service management CLI | Installer and service-manager fixtures cover each init system; representative disposable Linux hosts reach API readiness and survive reboot. Proxy settings, sudo-preserved Edge settings, custom URLs and architecture/libc selection reach the intended artifact/configuration. An air-gapped installation from a prepared bundle works with egress denied; target-specific bundle metadata and executables prevent accidental cross-architecture/OS misuse. | D01/D02/D04/D05 | [internal/cli/root.go](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/cli/root.go) |
| E24 — Implement container-mode developer lifecycle | Two named clusters coexist with distinct API ports and persistent volumes; reboot/recreate preserves the intended cluster state. Linux and macOS client fixtures, and the supported WSL2 workflow, produce correct Engine requests; invalid/conflicting port mappings fail before resource creation. Failed creation and uninstall clean only the selected instance; readiness exposes the host-reachable API address consumed by the access epic. | D06 | [internal/cli/service/container.go](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/cli/service/container.go) |
| E25 — Implement kubeconfig and client access workflows | Existing multi-cluster kubeconfigs retain unrelated contexts/users/clusters; repeated merges are idempotent and permissions/ownership are correct. A host kubectl command reaches each named container through its random published port, and cleanup removes only the selected instance entries. Sudo/doas/normal-root-shell fixtures follow supported identity sources without loginuid guessing; exported D2K keys remain private and disabled/unavailable services report clearly. | D06 | [internal/cli/kubeconfig](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/cli/kubeconfig) |
| E26 — Implement upgrade, reset and uninstall with data ownership | A matrix of host and named-container install/upgrade/reset/uninstall/purge tests verifies retained and removed paths, resources and kubeconfig entries. Legacy flag migration resolves the same effective configuration, handles version gates deliberately and cannot overwrite valid configuration with failed output. Upgrade/reset preserve applicable image, arguments, workload-port mappings and network MTU. Interrupted upgrades and repeated cleanup protect installer/self, unrelated port holders, external runtimes and neighboring data while reporting failures. | D02/D11 | [internal/cli/cmd_upgrade.go](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/cli/cmd_upgrade.go) |
| E27 — Build release artifacts and secure publication pipelines | Every promised artifact builds and has archive/layout/install smoke validation; unsupported combinations are explicit rather than silently missing. CI builds from a clean checkout with pinned dependencies, validates version metadata and catches asset naming/libc/architecture mismatches. A candidate release can be assembled with checksums and reproducible inputs; PR code cannot access publication secrets, and release jobs publish only validated artifacts. | D01/D02/D05/D06 | [Makefile](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/Makefile) |
| E28 — Qualify Kubernetes compatibility and failure recovery | Workload/networking, persistent storage, config/identity, controllers, DNS/LoadBalancer and LoadBalancer-update suites pass with no silently skipped required cases. Selected upstream conformance tests report their actual test count and zero unexplained failures; results explicitly avoid claiming full Kubernetes certification. A versioned report maps historical regressions and supported environments to passing runs; soak/restart tests meet declared bounds and unresolved required failures block release. | None | [test/e2e](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/test/e2e) |
| E29 — Establish and meet measured footprint and performance budgets | Repeatable amd64/arm64 reports compare pinned Go and Rust candidates on declared hardware/workloads; secondary architecture limitations are explicit. The E01 startup/memory/distribution budgets are met or an explicit scope decision is recorded before release; no unmeasured sub-200-MB or under-60-second claim is made. Performance CI is actually gated by committed baselines, including correct direction for density, and repeated workload/idle cycles show bounded memory growth. | D09 | [test/perf/README.md](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/test/perf/README.md) |
| E30 — Validate Go-to-Rust transition and ship the supported release | A rehearsed Go-to-Rust transition preserves the promised cluster/client identities, workload definitions and persistent data, with before/after assertions and a working recovery path. A new operator can follow the documented fresh-install and migration paths using the candidate artifacts; every compatibility deviation and unsupported platform is explicit. All preceding epic acceptance criteria are satisfied; the final versioned release includes checksums, test/performance reports, migration notes and accurate dependency/license attribution. | D11 | [README.md](https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/README.md) |

## Measurable engineering gates

These are selected **provisional engineering gates**, not measurements or claims of feasible release
performance. E29 must collect and commit the paired baseline and candidate evidence. No baseline
means “not qualified,” never zero or pass. A failure requires investigation and an explicit reviewed
scope/budget decision before release; changing an architecture or dropping a component is not an
implicit rebaseline. No sub-200-MB or under-60-second claim is accepted from README marketing alone.

Use the exact pinned Go distribution and Rust candidate on identical dedicated/disposable Linux
hardware or VMs, image digests, filesystem, kernel, cgroups, architecture, libc, runtime mode,
component selection and workload. Record machine model, CPU/RAM/storage, resource limits and
versions. First establish amd64 and arm64 pairs; record secondary-architecture gaps explicitly.
Include retained helper/component processes, managed runtime/shims and default system-addon pods.
For external runtime record both the whole node and isolated runtime/workload contribution, with
an idle host-runtime baseline; do not make Rust appear smaller by excluding retained processes.
Record process count, cgroup memory, and summed PSS (RSS fallback clearly labeled) separately.

| Metric | Definition and sampling | Initial gate | Owner |
| --- | --- | --- | --- |
| Boot-to-API | Monotonic duration from node process launch to authenticated create/read/delete succeeding, not socket-open; 20 fresh boots per paired cell | Candidate nearest-rank p95 ≤ 1.10 × reference p95 | E01.02/E29 |
| Node Ready | Same launch origin to node Ready and usable CRI; 20 fresh boots | Candidate p95 ≤ 1.10 × reference p95 | E13/E29 |
| First pod | Launch origin to a fixed digest-pinned probe pod Ready; preloaded and cold-image paths reported separately, 20 fresh boots each | Candidate p95 ≤ 1.10 × corresponding reference p95 | E28/E29 |
| Idle footprint | Ten-minute settle after required components Ready, then one-second whole-distribution memory samples for 15 minutes; five boots | Median of per-run p95 PSS and cgroup memory each ≤ 1.10 × matched reference | E21/E29 |
| Distribution size | Exact compressed release archive bytes; extracted executable/helper bytes; required default image payload bytes measured separately | Each matched metric ≤ 1.10 × reference, with identical enabled components/variant | E06/E27/E29 |
| Pod density | Maximum Ready replicas of a fixed digest/resource-request workload that maintain successful probes for ten minutes under identical node limits; five runs | Median capacity ≥ 0.90 × reference (higher is better) | E28/E29 |
| Sustained growth | 24-hour soak, alternating fixed workload/idle cycles; compare first and final settled idle hour | Final median memory ≤ 1.10 × initial median and zero OOMs/crashes/unexplained probe failures | E28/E29 |
| Shutdown | SIGTERM/SIGINT, repeated stop and injected worker failure; enumerate owned PIDs/tasks before/after, 20 runs | Graceful deadline 30 seconds, escalation/cleanup complete by 35 seconds; zero surviving owned processes and no unrelated process killed | E04/E28 |

The 600-second configurable per-component startup timeout is a compatibility default, not an
acceptable measured boot target or a global startup deadline. Timeout tests must honor overrides.
Shutdown bounds are a new explicit supervisor contract; E01.02 must test and report feasibility.
Benchmark reports retain every sample, failures, command line, input/candidate hashes, timestamp,
logs and environment. Timeouts count as failures, not discarded samples. A hardware/workload change
requires a new matched Go measurement and reviewed baseline update; CI gates committed values in
the correct direction. Run order should alternate Go/Rust to reduce environmental bias.

## Evidence boundary and next gate

This document results from static inspection of the pinned source, tests, release workflow and
planning acceptance contracts. No upstream installer, cluster, benchmark or privileged lifecycle
operation was run to produce it. All runtime/platform rows remain unqualified until their owners
provide evidence. Documentation review and pinned-path validation establish the inventory only.

E01.02 remains mandatory: compare supervised executables, a narrow helper/bridge and in-process
integration; run authenticated API CRUD across clean/abrupt datastore restart on disposable Linux;
prove cancellation/clean shutdown and record process/memory measurements. Its decision must name
Rust-owned subsystems, retained components, exact versions, protocols, credentials, packaging and
trust boundary. This contract permits investigation of those alternatives but selects none of them.
E01.03 must then pin official source revisions/hashes and generation ownership. E01.04 combines the
results into an accepted architecture before production integration depends on it.

[modules]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/go.mod
[release]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/.github/workflows/release.yml
[makefile]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/Makefile
[assets]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/build/download-deps.sh
[detect]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/cli/detect
[services]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/cli/service
[flags]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/config/flags/flags.go
[cli]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/cli/root.go
[defaults]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/config/defaults.go
[config-types]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/types/config.go
[constants]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/types/const.go
[embedded]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/config/embedded.go
[cli-config]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/cli/config/config.go
[config-api]: https://github.com/portainer/kubesolo/blob/2ef1c4787989f11f868f81bb84ae2afd4a49a81d/docs/configuration/config-api.md
