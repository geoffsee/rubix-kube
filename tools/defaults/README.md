# Official Go default and feature-gate oracle

This E02 extraction slice invokes the actual Kubernetes v1.35.7 defaulting functions at
commit `96cb9ab4201d88ce5e549fde047a686171838fdb`. It does not derive defaults from Rust,
protobuf defaults or OpenAPI annotations. The development-only Go program is added to a
fresh official source copy; the read-only KubeSolo reference is not involved.

The source archive and official Go 1.26.8 Linux arm64 archive are pinned by URL, size and
SHA-256 in `inputs.json`. Explicitly download those two archives, then run:

```sh
python3 tools/defaults/capture.py \
  --source-archive /path/to/verified-kubernetes.tar.gz \
  --go-archive /path/to/go1.26.8.linux-arm64.tar.gz \
  --output /tmp/new-official-default-capture
python3 tools/defaults/verify.py /tmp/new-official-default-capture/run0.json
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/defaults -p 'test_*.py' -v
```

The capture validates both input hashes before building. Docker may fetch the pinned
builder image. Compilation itself has `--network none`, `-mod=vendor`, `GOPROXY=off`,
`GOSUMDB=off`, `GOTOOLCHAIN=local`, `CGO_ENABLED=0` and parallelism two; the complete
upstream archive supplies its Go workspace, staging modules and vendor graph. Go 1.26.8
is explicitly unpacked and selected ahead of the image's bootstrap Go. No host packages,
services, Go cache or environment configuration are modified.

Each run uses a uniquely owned scratch container, no network, UID 65532, read-only root,
no capabilities, no-new-privileges, 512 MiB RAM, two CPUs, 64 PIDs and 16 MiB temporary
storage. There are no host mounts, host device access or existing cluster credentials.
The build deadline is 30 minutes, execution deadline 60 seconds; host capture retains at
most 8 MiB of build log and 2 MiB per execution. Exceeding output or time limits fails and
terminates the owned Docker client process group. Cleanup separately attempts every
owned container/image removal and inventory query, aggregates errors and writes a receipt
when Docker is unavailable. An unavailable inventory is null, never an empty-success claim.
Docker's ordinary build cache remains reusable. This is a trusted official-source fixture,
not an arbitrary-program security qualification.

Two executions per executable variant must produce identical raw JSON. `receipt.json` records source/harness,
toolchain and image identity, the output digest and cleanup results; `modules.sha256`
records top-level and staging module manifests plus vendor/modules.txt. The archive hash
identifies every remaining transitive vendored byte. Execution output records GOOS/GOARCH,
Go version, emulation version 1.35, minimum compatibility 1.34 and the empty feature-override
map. Both runs must independently satisfy `verify.py` and exact `expected.json` comparison.
Checks never refresh expected output automatically; changed defaults, gate history or gate
removals require a reviewed compatibility decision.

The six cases are zero and explicit inputs for kubelet, kube-proxy and controller-manager.
Actual functions are `SetObjectDefaults_KubeletConfiguration`, `SetObjectDefaults_KubeProxyConfiguration`
and `SetObjectDefaults_KubeControllerManagerConfiguration`. Explicit cases preserve a false
pointer, custom ports, client QPS and controller selection. The registered gate inventory
comes from `DefaultMutableFeatureGate.GetAllVersioned()` after importing `pkg/features`
and the component default packages: 215 registered gates, their versioned default/lock/stage/
minimum-compatibility history and effective enabled state. This is the registered set for
that import graph, not a claim that every binary-specific gate in Kubernetes is imported.
Upstream represents GA as an empty prerelease string; that exact value is retained.
Controller-manager configuration uses its actual capitalized JSON field names.

Independent checks anchor authentication, authorization, bind addresses, ports, client
limits, timing, explicit-value preservation and reviewed rotation/sidecar gate histories
to official source assignments. Mutation tests reject altered security defaults, lost
explicit false, gate-default changes and gate removals. The complete frozen output catches
changes outside those selected independent anchors. No Rust generator produces expectations.

Remaining scope includes completed host-derived API-server configuration, additional feature override/emulation
cases, other platforms, binary-specific gate registrations and real component behavior.
These outputs do not qualify a live cluster or close E02.03/E02.04 on their own. Source-derived
schema/protocol drift remains the complementary `tools/drift` gate.

The durable `evidence/` receipt binds the final executed sources to output SHA-256
`d936b24ca4fe4e789e92980ae70b9cbf5ba56231548f151b3a0022e85b75fe8e`.
Both executions passed independent and exact comparisons; all owned resources were removed.
Seventy top-level/staging module files are hashed in `evidence/modules.sha256`. Seventeen
local tests include recursive boolean-versus-integer drift, malformed JSON refusal,
bounded output and receipt preservation when all Docker cleanup/inspection calls fail.

## API-server option construction

A second build-tagged executable imports the official API-server options package, calls
`NewServerRunOptions()` and its `Flags()` method, and records all exposed flag defaults,
types, sections, deprecation and hidden status. Duplicate flag names fail extraction.
System namespaces, kubelet address preference and service port range are captured directly.
The base executable retains its original import graph; the receipt lists gates added,
removed or changed by the API-server import graph. Both variants run twice with distinct
explicit container hostnames (`fixture-0`, `fixture-1`). No hostname is normalized out.

```sh
python3 tools/defaults/verify.py /tmp/new-official-default-capture/apiserver0.json \
  --expected tools/defaults/apiserver.expected.json
```

This exercises option construction and flag exposure, not completion or server startup.
Official `pkg/controlplane/apiserver/options.Options.Complete` derives an external hostname
when no advertised address is available. `GenericServerRunOptions.DefaultAdvertiseAddress`
resolves an unset address from serving/network configuration. Those later host-derived
values remain unresolved constructor defaults here; they are not replaced with fixture
values or omitted from the flag inventory. TLS generation, listener creation, live feature
gate behavior and host-specific completion remain component/runtime qualification.
Opaque option callbacks and non-flag internals are not claimed as serialized full state.

The API-server variant captured 172 flags and the same 215 registered gates: no added,
removed or changed gate entries. Four final executions (two per variant) completed with
identical per-variant bytes and clean teardown. API-server output SHA-256 is
`a7538b7fc55682c4ad920085a12cb53451ad371786b79e957d8c79f29e1ee539`.
The API variant retains the same corrected component cases as the base variant. Independent API anchors and
mutation tests cover secure port, storage media type, watch cache, event lifetime,
anonymous authentication, kubelet communication and unresolved host-derived defaults.

Generated default dispatch is required and executed. The controller dispatch defaults
`KubeCloudShared` (5s node monitor, 10s route reconciliation, cluster name `kubernetes`,
cloud routes true); explicit cluster name, 7s period and false routes remain unchanged.
The kubelet dispatch walks `ReservedMemory` resource lists: the explicit `1.0001` memory
quantity rounds upward to milli precision (`1001m`) through official core defaulting.
This small quantity is a focused defaulting input, not a validated runtime memory budget.
Earlier direct-setter captures omitted these nested defaults and are superseded by the
corrected dispatch capture. Negative tests specifically reject bypassed nested dispatch.

The verifier validates the selected expected fixture and checks its byte digest against
committed provenance before comparing. `--expected` may select a copy of the reviewed
fixture; changing both capture and expectation does not bypass the frozen-output gate.
Upstream adoption updates the expectation and its provenance through review.
