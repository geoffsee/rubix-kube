# Credential file behavior fixtures

These E02.02 fixtures execute the pinned KubeSolo baseline's API-server
`generateServiceAccountKey` and `generateKubeConfig` (including its actual read,
constructor, and write helpers), and kubelet `generateKubeletKubeconfig`.
Package-local tests are added to a disposable source copy; vendored code is untouched.
No API server or kubelet is started.

The signing key is genuinely generated, parsed as PKCS1 RSA, validated and checked
for 2048 bits and mode 0600 inside the container. A second call must preserve its
bytes. No generated private key or token is exported. Kubeconfig copy tests use
explicit `SYNTHETIC-*` strings; those bytes are not certificates and this slice
makes no TLS authentication claim. The baseline's static token is checked inside
the harness, then removed from exported configuration; its user/context remain
observable. Kubelet credentials are fixed file references whose absence is also
characterized. Full structures are independently specified in verify.py, including
context selection, certificate references and synthetic embedded bytes.

Two independent containers per executable must produce identical public records.
Tests also exercise replacement certificate/node identity, missing inputs, directory
read/write failures, and missing parent directories. Existing corrupt service-account
keys are accepted and preserved by the baseline's existence-only check: that result
records a weakness, not a validated credential or a requirement to reproduce it.
E07 should explicitly reject or repair corrupt keys with trust preservation.

Run:

```sh
python3 tools/parity/fixtures/credentials/capture.py --output /tmp/unique-credentials
python3 -O tools/parity/fixtures/credentials/verify.py /tmp/unique-credentials
python3 -m unittest discover -s tools/parity/fixtures/credentials -p 'test_*.py'
```

The immutable source archive and Go 1.26.5 builder digest match the distribution
resource oracle. Baseline go.mod replacements and go.sum remain authoritative;
`-mod=readonly`, `external_deps`, and `-trimpath` are used. This is a distribution
behavior capture, separate from the official Go 1.26.8 generation extractor.
The build is limited to 30 minutes and 8 MiB diagnostics; each trusted runtime to 90 seconds
plus 110-second client deadline, 1 MiB output, 512 MiB RAM, 2 CPUs, 128 PIDs and 64 MiB private
tmpfs. Runtime has no network, host mounts, capabilities or writable root filesystem.
The shared defaults capture helper is imported read-only and hashed in the receipt;
it bounds subprocess output and independently attempts each owned-resource cleanup
and inventory before publishing errors. Docker build cache remains reusable.

Receipt and provenance bind executed harness/source/helper inputs to public outputs.
Semantic mutations cover identity, trust reference, permissions, key size, restart,
missing checks and forbidden token export; explicit checks remain active under -O.
No Rust consumer, live authentication/token issuance, host-user kubeconfig merging,
atomic interruption/repair, or full node lifecycle is claimed. These remain separate
component or fixture work. Webhook fixtures are a separate family.
