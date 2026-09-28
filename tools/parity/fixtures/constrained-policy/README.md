# Constrained-host source oracle

This fixture captures 23 independently expected records from KubeSolo revision
`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`, twice across four Go binaries.
The official Go 1.26.5 builder image and source archive are digest-pinned in
`Capture.Dockerfile`; original source hashes and all four executable hashes are
retained in `evidence/source.sha256`. Baseline `go.sum` supplies dependency checksums;
the small fixture go.mod fixes the required logging dependency versions.

`ipv6.go` and `modules.go` compile unchanged. `extract.go` uses Go's AST parser to
select `detectProxyMode`, CNI warning helpers and their exact variable/constant
declarations from original files. It preserves their ASTs and changes only package
layout/import sets. This avoids compiling unrelated runtime initialization; it is
explicit source-subset qualification, not a whole baseline binary. No vendor file
is edited and no replacement implementation supplies an observed result.

The network cases invoke the actual IPv6 helper in private chroot children dropped
to UID 65534. Owned synthetic proc paths exercise correct read-only values,
incorrect read-only values, absent/unreadable controls, prefix-1 acceptance and a
successful write. Only their public outcome and exact synthetic resulting bytes are
exported. After the child, the parent restores read permissions on its own test
file to inspect bytes. Proxy cases likewise use owned chroots. Module selection
invokes a synthetic iptables-version executable (the test binary's dedicated
version entry point), while the actual baseline selector parses its output.
CNI warning helpers see default plugin absence and private directory entries;
other CNI files are verified unchanged. Neither CRI nor real routing is exercised.

Each of eight runtime containers has no network or host mounts, a read-only root,
32MiB private tmpfs, 256MiB memory, two CPUs, 64 PIDs, 1MiB per-file limit and only
SYS_CHROOT/SETUID capabilities. Each test has a 30-second internal and 45-second
outer deadline with 1MiB output; build is limited to 900 seconds/8MiB. The shared
capture helper attempts each owned cleanup and publishes a receipt even on failure.
Unknown cleanup inventories are failures. Docker's ordinary build cache remains.

Independent `expected.json` records baseline weaknesses deliberately: permission
failures skipped by IPv6 setup, any leading `1` accepted, stat errors selecting nft,
and failed version execution assuming nft. Rust's stricter uncertainty policy is
covered separately; these records must not be reinterpreted as safe Rust passes.
The exact owned CNI filename is `10-bridge.conflist`. Default-location plugin warnings
do not establish that a runtime lacks plugins in another configured directory.

```sh
cargo run --locked -p rubix-dev --bin rubix-fixture -- capture constrained-policy --output /tmp/new-constrained-policy
cargo run --locked -p rubix-dev --bin rubix-fixture -- verify constrained-policy /tmp/new-constrained-policy
cargo run --locked -p rubix-dev --bin rubix-fixture -- verify-evidence constrained-policy /tmp/new-constrained-policy
cargo test --locked -p rubix-dev --lib fixture_oracles::policy
```

Rust verifier regressions bind exact source and output inventories, original source
pins, settled commands, repeated records and cleanup. They reject typed/semantic
mutations, duplicate/nonfinite JSON, rehashed false cleanup and altered commands.
Shared process tests cover setup and cleanup failures. Provenance hashes bind the independent expectations and retained build/raw evidence.
Fixtures contain synthetic values only. No credentials, host sysctls, services,
external runtime configuration or real CNI files are modified.

Historical `evidence/` captures and provenance remain unchanged. Current maintenance
uses the Rust runner and independent `tools/dev/src/fixture_oracles/policy.rs`
expectations. Fresh captures belong in `rust-evidence/`; current qualification
requires the complete Rust source inventory, exact owned command arguments, raw
image and cleanup inventories, and settled child processes.
