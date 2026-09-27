# Runtime mapping from the pinned distribution

This bounded E02.02 fixture calls actual `config.Defaults`, `cri.Resolve`, and
`config.BuildEmbedded` from KubeSolo revision
`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`. An additive Go test is compiled in a
fresh source copy. The vendored reference is read-only. No service, CRI connection,
certificate issuance, host discovery, filesystem preparation or cluster startup
is performed. Paths are strings in the returned structure, never accessed.

The public `evidence/mapping.json` is an ordered array of 13 records. Each has:

- `input`: stable case ID, desired state path/node name, supplied hostname,
  endpoint text, and a `changed` variant selector.
- `resolved`: the actual `cri.Resolve` result (`URL`, `SocketPath`, `External`).
- `error`: exact endpoint error text, empty on success.
- `embedded`: the complete actual `types.Embedded` JSON structure, null when
  resolution failed. All path fields and zero/unset fields are retained.

The source-specified verifier checks the whole envelope, not selected snapshots.
Defaults, path suffixes, certificate names, endpoint rules, zero values and changed
settings are independently specified from the reviewed Go source. Captured output
never produces its own expected map. Only raw diagnostic timing/random build names
are outside the fixture; no returned values are normalized.

| Cases | Distinction |
| --- | --- |
| default/custom/empty-path/relative-path | Every PKI, runtime, CNI, config, image and state path; Go lexical path cleaning |
| custom/raw-hostname/empty-hostname | Explicit node name trims/lowercases; injected fallback is preserved verbatim, even empty |
| external-path/external-url/whitespace-endpoint/root-endpoint | Bare absolute paths and unix URLs; trim surrounding spaces; no endpoint path cleaning; root path accepted |
| relative-endpoint/non-unix-endpoint/unix-host-endpoint | Exact errors; zero endpoint and no runtime mapping on resolution failure |

`custom` and `external-path` set `changed=true`. They supply IPv6 node/LB addresses,
1280 MTU, explicit-address/MTU flags, and container mode true while the desired
configuration deliberately contains conflicting addresses, MTU and container mode.
The configured runtime endpoint also conflicts with the explicitly resolved probe.
The actual mapper uses those supplied probes. The variant additionally changes
SANs, CPU policy/options/reservations, local storage, load balancing, IPv6, D2K,
metrics and Portainer selection. `mapping_capture_test.go` is the complete input
recipe; `verify.expected()` independently specifies every resulting field.
Synthetic Portainer ID/key are only used to exercise the activation boolean;
no input key value or generated credential is exported. Certificate fields contain
path strings only.

For a Rust consumer, map `input.path`, `node_name`, `hostname` and `endpoint` to
its model and supplied probe; apply the documented `changed` recipe when true.
Compare the relevant `embedded` fields or construct this complete envelope when
that consumer owns the whole mapping. Particularly useful field anchors are
`NodeName`, `RuntimeExternal`, `RuntimeEndpoint`, `RuntimeSocketPath`,
`RuntimeCgroupDriver`, `NodeIPSpecified`, `MTUSpecified`, all `*Certs`,
`ContainerdConfigFile`, `KubeletConfigFile`, `KineSocketFile`, and `ControllerDir`.
Do not infer that the empty/relative managed endpoint strings are usable sockets.

The baseline mapper assumes `system.GetHostname` has normalized its fallback;
it does not itself validate a supplied hostname. Rust's configuration runtime
boundary deliberately trims/lowercases fallback names and rejects an empty result.
The `raw-hostname` and `empty-hostname` records preserve the original observations;
a Rust consumer must test that named policy deviation explicitly rather than
rewrite the oracle. The baseline resolver also accepts `unix:///` without checking
whether a socket exists. These are pure mapping observations, not endorsements of
invalid runtime configurations. Host discovery and managed/external runtime
interoperability remain separate component work.

```sh
python3 tools/parity/fixtures/runtime-mapping/capture.py --output /tmp/new-runtime-mapping
python3 tools/parity/fixtures/runtime-mapping/verify.py /tmp/new-runtime-mapping/mapping.json
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/parity/fixtures/runtime-mapping -p 'test_*.py'
PYTHONDONTWRITEBYTECODE=1 python3 -O -m unittest discover -s tools/parity/fixtures/runtime-mapping -p 'test_*.py'
```

The Dockerfile pins the source archive SHA-256 and Go 1.26.5 builder image digest,
uses the reference module replacements unchanged, and builds only the additive
package test with `external_deps`. This is distribution characterization, distinct
from the official Kubernetes Go 1.26.8 default extractor. Network is used for
preparation; the two actual test runs disable networking, run as UID 65532 with
read-only root, no capabilities, no-new-privileges, 512MiB memory, two CPUs,
128 PIDs and a private 64MiB tmpfs. No host paths or devices are mounted.

The shared bounded process/cleanup helper is hash-bound in `evidence/receipt.json`.
Build deadline is 1800 seconds with 8MiB output; each runtime deadline is 110 seconds
with a 90-second Go test deadline and 1MiB log/file limits. JSON/log reads are bounded
and reject duplicate keys, nonfinite values and boolean/integer substitutions.
Every run must emit exactly one capture record, and both records must match the
fixture and independent verifier. A failure still attempts removal and inspection
of every owned container/image and publishes its receipt; unknown or failed cleanup
is an error. Docker build cache remains ordinary daemon-owned cache. The harness
executes only the pinned trusted source test, not arbitrary hostile artifacts.

`provenance.json` records exact baseline source hashes and all retained fixture
files. Receipt tests require exact source/output inventories and current hashes.
Mutation tests reject altered hostname/path/probe/endpoint semantics, omitted or
reordered cases, missing/duplicate/changed repeated records, and incomplete hash
inventories, including under Python optimization. This is a future-consumer oracle,
not Rust parity, distribution lifecycle qualification, or an additional foundation
closure gate.
