# PKI generation and restart oracles

`pki_capture_test.go` invokes baseline `GenerateAllCertificates` and
`InvalidateIfStale` inside the disposable container described in
[the capture procedure](../resources/README.md). It writes only Go test-owned
temporary directories. No private key bytes leave the container.

The ten certificate kinds include both trust roots and optional D2K credentials.
The harness parses actual X.509 certificates and PKCS1 RSA keys, validates RSA
private keys, compares public moduli/exponents, verifies chains using the intended
CA, records subjects/SANs/usages/2048-bit key size/validity duration/0600 key modes,
and checks positive <=128-bit random serials. Timestamp, serial and generated key
values are omitted; their properties are checked instead. No subjects, SANs,
permissions, usages or chain failures are normalized away. Network-none execution
makes local address discovery loopback-only; API node address is explicitly pinned.

An ordinary restart preserves all certificate fingerprints. Four actual restarts
change the node IP, add an extra SAN, overwrite the API leaf with corrupt PEM, or
replace it with a correctly signed but expired leaf. Baseline invalidation removes
stale leaves; regeneration preserves both roots and request-header credentials.
Fingerprints are compared within the run and exported as booleans, so random keys
do not make golden files unstable. Invalid extra SAN strings do not trigger a
rotation. A separate valid IPv6 extra SAN also does not trigger rotation in this
baseline; this is a distinct baseline gap, not an invalid SAN. Both probes inspect
leaf absence without aborting before recording the result. Deletion rejects empty, root, dot and symlink PKI paths.

Two separate negative characterizations expose an existing baseline weakness:
`GenerateAllCertificates` returns success when an already-existing admin private
key is unparsable, and also when that file contains a valid unrelated RSA key whose
modulus independently fails to match its certificate. The baseline checks only
file existence before skipping issuance. These flags do **not** say that the
credentials validated or that Rust should preserve unsafe behavior. E07.02 (#52)
must decide and document an explicit compatibility deviation for existing-key
validation, with safe restart repair in E07.03 (#53) and trust-root protections in
E07.01 (#51). The healthy captures separately require successful matching and
chain verification.

`expected.json` is public normalized output from real cryptographic execution;
`tools/dev/src/fixture_oracles/pki.rs` independently constructs policy and rotation
expectations from the reviewed source. Mutation tests reject wrong
trust-root preservation, mismatched healthy keys, missing SANs and weak file modes.
The Rust CLI rejects invalid captures in debug and release builds.
See the resource provenance for exact baseline, harness and capture hashes.

This captures PKI file lifecycle. [Credential fixtures](../credentials/README.md)
separately cover service-account signing keys, kubeconfigs, reuse and failure cases.
Atomic interruption during issuance, missing/mismatched CA repair, renewal scheduling
and complete cluster lifecycle remain later component integration and qualification.
See the [coverage ledger](../README.md#foundation-coverage-and-later-qualification);
these observations do not establish complete distribution parity.
