# Source-derived resource builder oracles

This E02.02 slice records nine variants produced by the actual baseline CoreDNS,
local-path, Portainer and D2K resource constructors at KubeSolo revision
`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`. The added package-local Go test files
invoke unexported constructors without editing any upstream implementation.
Kubernetes fake clients capture typed JSON before API admission/defaulting; this
is an in-memory builder oracle, not server reconciliation or running-addon parity.

`expected.json` retains full typed object fields (including null empty collections).
`verify.py` independently specifies source-derived identities, references, images,
DNS variants, storage semantics, synthetic Edge configuration and D2K TLS mounts.
Each constructor sequence executes twice. Injected client errors must propagate;
Portainer existing configuration/secrets remain unchanged after replacement inputs;
D2K missing TLS inputs fail and changed synthetic certificate bytes update its
Secret. D2K input strings are intentionally synthetic: its secret builder copies
bytes without parsing certificates. Genuine crypto is exercised in `../pki/`.

Reproduce from the repository root with Docker available:

```sh
python3 tools/parity/fixtures/resources/capture.py --output /tmp/unique-oracle-capture
python3 tools/parity/fixtures/resources/verify.py /tmp/unique-oracle-capture
python3 tools/parity/fixtures/pki/verify.py /tmp/unique-oracle-capture/pki.json
python3 -m unittest discover -s tools/parity/fixtures/resources -p 'test_*.py'
python3 -m unittest discover -s tools/parity/fixtures/pki -p 'test_*.py'
```

Build uses the digest-pinned Go 1.26.5 parity toolchain and SHA256-pinned source
archive; module resolution is `-mod=readonly` against baseline go.sum. Go 1.26.8
remains the separately accepted generation-extractor pin. Build has a 30 minute
outer deadline; each execution has a 90 second Go deadline and 110 second Docker
client deadline. Execution drops all capabilities, disables network, uses UID65532,
a read-only root, 64MiB private tmpfs, 512MiB memory and 128 PID limits. No host
mounts, host keys or Kubernetes endpoints are used. Private test directories vanish
with the container. Dedicated containers and image tags are removed and inventoried;
Docker's ordinary build cache remains reusable. This trusted pinned test harness
is not a sandbox for arbitrary submitted programs. Capture output is restricted to
public resource JSON, synthetic secret strings and normalized PKI metadata.

`provenance.json` records immutable source links/hashes, additive harness and durable
fixture hashes. `capture-receipt.json` and `evidence/` preserve execution output.
The unit negatives deliberately change storage policy, selectors and DNS behavior;
they must fail the independent checks. A future Rust builder should emit this
object envelope and pass both full fixture equality and semantic checks; no Rust
implementation is claimed here.

Related foundation captures now cover [webhooks](../webhooks/README.md),
[kubelet/runtime configuration](../node-config/README.md), and
[kubeconfigs and service-account credentials](../credentials/README.md).
Live API admission, full node restart/shutdown and workload lifecycle remain later
component integration and release qualification. Resource ownership follows E11–E20;
fixture maintenance belongs to E02. See the [coverage ledger](../README.md#foundation-coverage-and-later-qualification)
for the distinction between foundation evidence and complete distribution parity.
