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

Two executions must produce identical raw JSON. `receipt.json` records source/harness,
toolchain and image identity, the output digest and cleanup results; `modules.sha256`
records top-level and staging module manifests plus vendor/modules.txt. The archive hash
identifies every remaining transitive vendored byte. Execution output records GOOS/GOARCH,
Go version, emulation version 1.35, minimum compatibility 1.34 and the empty feature-override
map. Both runs must independently satisfy `verify.py` and exact `expected.json` comparison.
Checks never refresh expected output automatically; changed defaults, gate history or gate
removals require a reviewed compatibility decision.

The six cases are zero and explicit inputs for kubelet, kube-proxy and controller-manager.
Actual functions are `SetDefaults_KubeletConfiguration`, `SetDefaults_KubeProxyConfiguration`
and `SetDefaults_KubeControllerManagerConfiguration`. Explicit cases preserve a false
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

Remaining scope includes API-server options/defaults, additional feature override/emulation
cases, other platforms, binary-specific gate registrations and real component behavior.
These outputs do not qualify a live cluster or close E02.03/E02.04 on their own. Source-derived
schema/protocol drift remains the complementary `tools/drift` gate.

The durable `evidence/` receipt binds the final executed sources to output SHA-256
`60c8c2f3c7b74c860ae82ac8e2e38df76cf226a9eab3f0df20a1250bd8ac556d`.
Both executions passed independent and exact comparisons; all owned resources were removed.
Seventy top-level/staging module files are hashed in `evidence/modules.sha256`. Eleven
local tests include recursive boolean-versus-integer drift, malformed JSON refusal,
bounded output and receipt preservation when all Docker cleanup/inspection calls fail.
