# Upstream input provenance

Status: accepted input, generation and output ownership contract for
[E01.03](https://github.com/geoffsee/rubix-kube/issues/33), 2026-09-27. This follows the
[merged E01.01 compatibility contract](https://github.com/geoffsee/rubix-kube/pull/136) and
[selected supervised-component boundary](../../experiments/component-boundary/ADR.md).
E02.03–E02.04 implement the preparation/generation pipeline and independent drift checks;
production consumers and platform qualification remain their respective epics' work.

The [machine-readable inventory](upstream-inputs.json) records exact revisions, immutable
raw-source URLs, byte lengths and SHA-256 hashes of fetched files. Hashes cover HTTP response
body bytes, without newline normalization. This is an accepted source inventory, not an implemented
build lock. It includes selected direct-client protobuf imports; the pinned upstream Go module
files and sums govern extraction dependencies. Downloaded component source has not been executed
as part of this provenance work.

## Authority and revisions

| Domain | Authoritative release | Peeled commit | Annotation object |
| --- | --- | --- | --- |
| Kubernetes schemas, CRI and component defaults | [kubernetes/kubernetes v1.35.7](https://github.com/kubernetes/kubernetes/tree/v1.35.7) | `96cb9ab4201d88ce5e549fde047a686171838fdb` | `b317677b30aa81848f87b3dd36980942c4c8d832` |
| Containerd server comparison | [containerd/containerd v2.2.5](https://github.com/containerd/containerd/tree/v2.2.5) | `e53c7c1516c3b2bff98eb76f1f4117477e6f4e66` | `bafc09e8fb5c3d754f57a77cd0b1fc6b7919834d` |
| Containerd client protocol module | [containerd/containerd api/v1.10.0](https://github.com/containerd/containerd/tree/api/v1.10.0) | `8b34ce391bd114e080892cccfa956ef3807c207c` | `91b6e6810ab04a9c7bbf715a18f63a99b9cedea2` |
| Kine implementation and datastore endpoint | [k3s-io/kine v0.16.3](https://github.com/k3s-io/kine/tree/v0.16.3) | `7414fca1dc92b44d7ec6de82383ba614a86d2994` | Lightweight tag points directly to commit |
| Protobuf compiler well-known imports | [protocolbuffers/protobuf v36.2](https://github.com/protocolbuffers/protobuf/tree/v36.2) | `2c74169b34066ceb8ddb6b882fcb3fb32d737a55` | Tag resolution recorded in inventory |

GitHub's Git reference/tag APIs resolved these releases. `git ls-remote` independently
confirmed the Kubernetes annotated tag and peeled commit. These checks establish retrieved
revision identity, not signature verification or a release attestation.

The compatibility baseline remains KubeSolo
`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`. Its `go.mod` requires Kubernetes v1.35.7,
Kine v0.16.3 and containerd v2.2.3, then replaces Kubernetes with
`github.com/k3s-io/kubernetes v1.35.7-k3s1` and containerd with
`github.com/k3s-io/containerd/v2 v2.2.5-k3s2`. Those replacements are compatibility evidence,
not the authority for Kubernetes generation. Choosing official containerd v2.2.5 for this
inventory does not prove that it reproduces the replaced runtime. The selected boundary retains
official component executables; E02/E09 must characterize and implement required fork adaptations
before runtime parity is claimed.

## Prepared inputs and selected consumers

All paths in this table are relative to their pinned repository; individual hashes are in the
inventory. The whole source revision remains necessary for executable default extraction.

| Input | Purpose and selected consumer | Independent oracle / acceptance owner |
| --- | --- | --- |
| Kubernetes `api/openapi-spec/swagger.json` | OpenAPI v2 resource schema; resource serialization and API client types | Official API server round trips plus independently recorded upstream responses; E02/E11/E12 |
| Kubernetes `staging/src/k8s.io/cri-api/pkg/apis/runtime/v1/api.proto` | CRI v1 messages/RPCs used by runtime adapters | Managed containerd and external containerd/CRI-O behavior, including absent `RuntimeConfig` fields; E09/E10 |
| Kubernetes `staging/src/k8s.io/kubelet/config/v1beta1/types.go` and `pkg/kubelet/apis/config/v1beta1/defaults.go` | Kubelet configuration and `SetDefaults_KubeletConfiguration` | Execute official defaulting and separately inspect effective component behavior; E03/E13 |
| Kubernetes `staging/src/k8s.io/kube-proxy/config/v1alpha1/types.go` and `pkg/proxy/apis/config/v1alpha1/defaults.go` | Proxy configuration and `SetDefaults_KubeProxyConfiguration` | Effective proxy behavior and Service routing fixtures; E03/E16 |
| Kubernetes `staging/src/k8s.io/kube-controller-manager/config/v1alpha1/types.go` and `pkg/controller/apis/config/v1alpha1/defaults.go` | Controller-manager configuration and `SetDefaults_KubeControllerManagerConfiguration` | Workload reconciliation fixtures; E03/E12 |
| Kubernetes generated default dispatch files, component-base defaults, `cmd/kube-apiserver/app/options/options.go`, and apiserver recommended options | Nested default application and `NewServerRunOptions`; these are not a complete closure of defaults | Source-executed extraction plus authenticated API behavior; E02/E11 |
| Containerd API module services: content, images, leases, namespaces, containers, tasks, events, diff, snapshots, transfer; recursive `api/types` imports | Direct managed-containerd client for archive import, content/image metadata, leases and namespace ownership; CRI remains workload/runtime interface | Pinned containerd RPC behavior, especially offline import, cancellation and ownership; E09 |
| Kine `pkg/endpoint/endpoint.go`, `pkg/drivers/sqlite/sqlite.go`, `pkg/server/server.go` | Endpoint, SQLite and etcd-compatible implementation evidence | Authenticated API CRUD, restart, persistence and cleanup spike; E01.02/E08 |
| Each repository's `go.mod` and `go.sum`; containerd `api/go.mod` and `api/go.sum` | Dependency/extraction provenance, distinct from Rust `Cargo.lock` | Clean preparation with exact source/toolchain and resolved dependency closure; E02/E27 |

The CRI file at this revision has no protobuf imports. Containerd's root module requires the
separately versioned `github.com/containerd/containerd/api v1.10.0`, which is selected as the
direct-client protocol authority. All 18 inspected module files (ten service schemas, six local
type imports, `go.mod` and `go.sum`) at its peeled commit are byte-identical to the corresponding
files in the v2.2.5 server tree. The JSON records both hashes and the comparison outcome per file.
This reconciles source inputs without asserting runtime or K3s-fork equivalence.

The five external imports are `google/protobuf/{any,descriptor,empty,field_mask,timestamp}.proto`.
Their authoritative protobuf v36.2 source revision and hashes are in the inventory; each was
byte-compared with the include file shipped in the digest-verified protoc archive. Preparation
must reproduce this closed import set, use the source import names under
`github.com/containerd/containerd/api/`, and fail on missing/mismatched inputs. E02 implements this
verification rather than reselecting protocol authority. E09 owns its namespace metadata, import
transaction/lease cleanup and missing-content failure behavior. These are managed-runtime
operations; external CRI attachment must not adopt ownership of host images/namespaces.

Kine does not define an independent Kubernetes datastore protocol schema. Its pinned module
requires `go.etcd.io/etcd/api/v3 v3.6.12`; Kubernetes' pinned module requires v3.6.5. No Rust etcd
client is selected: supervised kube-apiserver owns that connection and Kine owns datastore
protocol handling. The passing E01.02 spike exercised this boundary. The source versions alone
do not establish complete protocol parity; watch/compaction/recovery remain E02/E08/E11 evidence.

### Selected datastore trust contract

Production API-server-to-Kine communication uses mutual TLS on `127.0.0.1:2379`, with a dedicated
datastore CA and distinct server/client leaves. This deliberately corrects the baseline/spike's
plaintext local transport: loopback binding alone does not exclude unprivileged local clients.
The Kubernetes user/client CA must not be accepted as a datastore client CA, because those users
must not bypass API authorization by connecting directly to storage.

Kine is configured with `--listen-address=127.0.0.1:2379`, `--server-cert-file=<server-cert>`,
`--server-key-file=<server-key>` and `--trusted-ca-file=<datastore-ca>`. API server uses
`--etcd-servers=https://127.0.0.1:2379`, `--etcd-cafile=<datastore-ca>`,
`--etcd-certfile=<api-client-cert>` and `--etcd-keyfile=<api-client-key>`.
Kine's `--ca-file`, `--cert-file` and `--key-file` configure its backend connection and are not
substitutes for these server flags. Disable Kine's separate metrics listener with
`--metrics-bind-address=0` unless E21 explicitly enables and protects it.

The server certificate must contain IP SAN `127.0.0.1` and server-auth usage; the API-server
datastore certificate needs client-auth usage. E07 owns persistent private CA/keys and rotation;
E08/E11 own exact argument construction and readiness. Missing/invalid credentials fail startup;
there is no automatic plaintext fallback. Process cleanup must preserve the trust roots.

Source evidence is pinned and hashed in the inventory: Kine `pkg/app/app.go` maps server flags
to `ServerTLSConfig`; `pkg/endpoint/endpoint.go` supplies gRPC TLS credentials; `pkg/tls/config.go`
passes `TrustedCAFile` into `transport.TLSInfo.ServerConfig`. Its declared etcd client/pkg v3.6.12
dependency's `client/pkg/transport/listener.go` selects `tls.RequireAndVerifyClientCert` when
`TrustedCAFile` is nonempty. Official Kubernetes
`staging/src/k8s.io/apiserver/pkg/server/options/etcd.go` registers the matching client flags.
This verifies configuration support, not a completed runtime mTLS test. E07/E08/E11 must prove
valid-client success plus missing, wrong-CA and expired-client rejection, wrong-server-CA and
SAN rejection, and restart/rotation behavior. The prior plaintext isolated spike remains evidence
only for its recorded transport; these negatives must not be represented as already passed.

## Accepted generation and output ownership

Consume published Kubernetes bindings and commit explicitly generated CRI/containerd clients
and default fixtures. This makes upstream changes reviewable and keeps custom generators out of
ordinary compilation. The following choices are accepted; implementing them belongs to E02 and
their consumers. They do not add production code to the current workspace.

| Output | Owner | Selected production path | Tool contract |
| --- | --- | --- | --- |
| Kubernetes resource bindings and handwritten serialization adapters | E02 verifies schema/generator; E11–E20 resource consumers | Published `k8s-openapi`, no duplicate local resource tree; handwritten adapters remain distinct | `k8s-openapi 0.28.0`, explicit `v1_35`; matching input and temporary compilation proven below, serialization gate remains |
| CRI messages and clients | E02 generation; E09/E10/E13 consumers | Committed client bindings, all 35 CRI RPCs, no server generation or API `build.rs` | `prost` family 0.14.4, `tonic` family 0.14.6, `protoc 36.2`; options/lock as verified below |
| Containerd-specific bindings | E02 generation; E09 consumer | Committed clients from pinned api/v1.10.0 module and closed imports; no API `build.rs` | Same selected prost/tonic/protoc versions; no custom fork schema generation |
| Official configuration/default fixtures | E02 extraction; E03/E11/E12/E13/E16 consumers | Committed JSON fixtures from development-only Go extraction invoking pinned functions, arguments/platform/feature gates recorded | Go 1.26.8; full source/module files from pinned official commits, `GOTOOLCHAIN=local` |
| Assets and retained components | E06 manifest, E27 assembly | Explicit preparation and per-target hashes, separate from source generation | Supervised Kubernetes components/Kine/containerd and required helpers/images from selected boundary; no embedding/build script currently exists |

The versions below are selected tooling, with behavioral integration still unqualified.
E02 must preserve exact generator binaries/versions/digests, Cargo dependency lock, Go extraction
toolchain, invocation, input manifest, feature gates and output hashes. It must regenerate twice
from clean prepared inputs and compare outputs, then check against an oracle not produced by
the same translator. Schema shape checks cannot establish admission, validation, reconciliation,
defaulting or storage semantics. Generated bindings do not implement Kubernetes components.

### Exact selected generators and schema comparison

Registry API metadata and published crate manifests were inspected on 2026-09-27. Exact selected
versions and registry checksums are included in `generation_tools` in the JSON inventory.
No workspace Cargo manifest/lockfile has been changed. The isolated temporary probe below compiled
these tools without adding a production consumer.

| Selected tool/library | Exact version | Verified compatibility/provenance |
| --- | --- | --- |
| `k8s-openapi` | `0.28.0`, feature `v1_35` | Published crate source commit `b2c839b8b1828e9035ff8467cdac87756190aa4e`; edition 2021, no declared `rust-version`; temporary Rust 1.97.1 compilation passed |
| `prost`, `prost-build`, `prost-types` | `0.14.4` | Published MSRV 1.85; `prost-build` depends on `prost`/`prost-types` 0.14.4 |
| `tonic`, `tonic-prost`, `tonic-prost-build`, `tonic-build` | `0.14.6` | Published MSRV 1.88; `tonic-prost-build` depends on `prost-build`/`prost-types` 0.14 and `tonic-build` 0.14.6 |
| Official protobuf compiler | `36.2` | macOS arm64 archive digest verified and `libprotoc 36.2` executed; Linux amd64/arm64 release digests recorded but binaries not executed |

The [k8s-openapi generator's version map](https://github.com/Arnavion/k8s-openapi/blob/b2c839b8b1828e9035ff8467cdac87756190aa4e/k8s-openapi-codegen/src/supported_version.rs)
selects official Kubernetes **v1.35.6** for its `v1_35` module. That is not the requested patch release,
so both schemas were fetched and compared. The v1.35.6 schema and the pinned official v1.35.7
schema are **byte-identical**: 3,846,687 bytes, SHA-256
`483500149ee52ce5753d75f5639101d985bb4f5e902cc05b1ba7627465d62446`, with 735 definitions.
This supports consuming the published library rather than maintaining a second generated tree.
It establishes identical schema input, not identical runtime semantics between those patch releases.
Use the explicit `v1_35` feature; `latest` selects v1_36 and must not be used for this contract.

The same pinned generator supports
`--generate=1.35:file:///absolute/prepared/swagger.json`, so E02 can regenerate from verified local
input in an isolated checkout and compare `src/v1_35` with the published crate. The generator's
own package version is 0.1.0 and is not published: pin the source commit, an independently committed
tool dependency lockfile, Rust toolchain and formatting tool version. Do not assume that the
published library's package checksum also locks the generator's dependencies. Record the generator's
upstream-bug and special-type fixups, including quantities, CRD JSON, watch events and metadata.

For CRI, a temporary directory held the digest-verified official macOS arm64 `protoc 36.2` and the
pinned CRI source. `protoc --proto_path=<directory> --descriptor_set_out=<directory>/cri.pb
--include_imports api.proto` succeeded without an import directory. The resulting descriptor was
30,965 bytes, SHA-256 `c5160684da68ab86968036f9efef4685c3371bc7ffd2abadbdb808ff6d6e4ff0`.
Temporary descriptor-probe files were removed. This tests parsing/descriptors only; Rust generation
and compilation were tested separately below. The descriptor path and options must remain
fixed when comparing hashes; source-info inclusion is deliberately absent from this probe.

An isolated temporary Rust 2024 package pinned all eight crate candidates with exact requirements.
Using Rust `1.97.1 (8bab26f4f 2026-07-14)` and Cargo `1.97.1`, it ran:

```text
cargo run -- out-a
cargo run --locked -- out-b
cargo check --locked --all-targets
```

The first two commands generated client-only CRI bindings into separate initially empty output
directories with the same generator binary/dependency lock. `prost_build::Config` selected the
verified `protoc` executable explicitly and `btree_map(["."])`; `tonic-prost-build` enabled clients,
disabled servers and Cargo rebuild notices, and compiled `api.proto` with `.` as its include path.
The generator also instantiated a `k8s_openapi::api::core::v1::Pod` to compile the selected bindings.
The final check included the generated file in a temporary library and passed for all targets.

Both generated `runtime.v1.rs` files were byte-identical: **178,603 bytes**, SHA-256
`cd0e32f1168232c195f29714b9bed6580e551781a0d17d11db9b07a79375c9cd`.
All **35** service method routes were present: 30 RuntimeService methods and five ImageService
methods, including streaming `GetContainerEvents`, `RuntimeConfig`, and
`UpdatePodSandboxResources`. The complete method inventory, probe manifest/source and temporary
dependency lock contents are recorded under `generation_tools.rust_probe` in the JSON.
That evidence lock's SHA-256 is
`9e973b33c897012ec6bb353cae61dac04cc4b0d64f8f2f9ccf7e9889470de13d`.

This proves local translation, route coverage and compilation on Darwin arm64. It does not prove
Linux compilation, two independent clean toolchain builds, behavior parity or runtime
interoperability. E02 must integrate these selected tools into dedicated maintenance tooling,
pin the transitive lock and builder settings, then repeat generation from clean prepared inputs.
Generated clients require the matching `tonic-prost` codec as well as `tonic` and `prost` at runtime.

Independent validation must include official API-server JSON round trips for quantities, metadata,
null/omitted fields, IntOrString, CRD JSON and watch events, with Go-generated fixtures kept separate
from the Rust generator. CRI validation should compare descriptors with official Go definitions,
then exercise real managed/external runtimes, unknown enum values and missing `RuntimeConfig`
fields. In particular, an absent Linux configuration must trigger host detection rather than
misinterpreting the protobuf-zero enum value as an explicit systemd cgroup choice. A descriptor
and bindings generated from one source/compiler are not independent behavioral oracles.

### Go extraction toolchain

Select **Go 1.26.8** for development-only default extraction. The official
[download manifest](https://go.dev/dl/?mode=json&include=all) listed stable Go 1.27.1 and Go 1.26.8
on verification, making 1.26 the previous supported release line under
[Go's support policy](https://go.dev/doc/devel/release#policy).
This pins its latest stable patch rather than adopting a moving toolchain. Kubernetes/containerd
declare Go 1.25.0 and Kine declares Go 1.25.1; the selected compiler exceeds these minimums.
This is an extraction-tool selection, not a claim that all retained component release builds
were made with that compiler.

The inventory records official archive URLs, sizes and SHA-256 values for Darwin/Linux amd64/arm64.
The Darwin arm64 archive was downloaded, verified against
`a012b25b571bd0138a03dcd25375ceba866fe5ca822f426d2c66a4de56fd3f4b`, extracted only in a temporary
directory, and reported `go version go1.26.8 darwin/arm64` with `GOTOOLCHAIN=local`. Temporary
files were removed. Linux archive hashes are pinned but those Go binaries were not executed here.
E02 runs extraction in disposable Linux environments using the pinned official Kubernetes tree,
`go.mod`/`go.sum` and its staging replacements, with `GOTOOLCHAIN=local` preventing implicit upgrades.
Record the extraction program revision, platform, feature-gate inputs and output hashes.

Extract fully defaulted kubelet v1beta1, kube-proxy v1alpha1 and controller-manager v1alpha1
configuration fixtures plus API-server resolved-option fixtures. Generated default dispatch must
run so nested defaults are preserved. Configuration wrappers remain handwritten Rust code owned by
E03 and component epics; schemas/default fixtures do not replace validation or runtime behavior.
E02 must compile and run these extractors and compare them with independent component observations.
The verified Go version probe does not establish that extraction has already passed.

## Build and refresh boundaries

The current workspace remains one placeholder executable, with no `build.rs` or generation tool.
Keep handwritten distribution code in that workspace package initially. E02 owns a separate
maintenance-tool package so prost/tonic code generators and Go extraction orchestration do not
enter the shipped node dependency graph. Committed CRI/containerd generated modules may live in
the node package until a second Rust package consumes them; split a shared protocol crate only
at that concrete boundary. Published Kubernetes bindings are a normal runtime dependency with
one selected `v1_35` feature across the workspace. No illustrative crate name is mandatory.

Explicit maintenance preparation fetches immutable sources, verifies recorded hashes, closes
imports, prepares assets, and invokes pinned generators/extractors. Ordinary `cargo build` and
node startup never perform upstream refresh or invoke Go extraction. Cargo may still download
locked Rust dependencies unless its cache or a vendor directory provides them.

No project build script generates API/CRI/containerd bindings or fixtures. The published
`k8s-openapi` package has its own local feature/metadata build script; this decision does not
misrepresent that dependency as build-script-free. It does not fetch upstream schemas.
If E06 later needs an
asset index build script, its local inputs are the prepared manifest and referenced asset files;
its output is an index under `OUT_DIR`. It must declare `rerun-if-changed` for every input and
`rerun-if-env-changed` for each explicitly supported variant/target selection variable. It must
fail when prepared input is absent or has the wrong hash, and must not download assets or start
services. Exact manifest fields and environment variable names remain an E06 design decision.

## Verification and remaining gates

All inventoried files were fetched from commit-qualified official URLs and hashed locally.
Repository tree queries confirmed the inspected paths and returned untruncated trees. No
upstream component, Go extractor or runtime test was executed for this document. The isolated
`protoc` descriptor probe and temporary Rust generation/compilation probes above passed. They do
not provide independent behavioral parity or platform qualification.

E01.03 fixes the source authority, verified revisions/hashes, selected tools, consumers and
output/build ownership above. E02 implements repeatable preparation, generation, Go extraction
and independent drift/parity checks with this contract; it must not silently reselect inputs or
drop required consumers. Containerd code generation/compilation and Linux tool execution remain
explicit implementation checks, not unresolved architecture choices. Full runtime/platform
qualification remains separate evidence. Changes to these accepted inputs require an upstream
adoption assessment, updated provenance and relevant compatibility checks.
