# Native Linux decoder qualification

The pinned Rust image builds two release integration-test executables and the
Rust fixture runner. Two freshly named Linux arm64 containers run exactly 14
decoder tests followed by exactly 10 decoded-ELF tests. Missing, repeated,
failed, swapped or additional test cases fail verification. Fixed gzip/zstd
vectors exercise native decoding without executing upstream artifacts.

Both runtime invocations must match a nonce-framed builder checksum for each of
`decode-tests`, `decoded_elf-tests` and `fixture`. A consistent substitution in
both runtime logs still fails against builder output. The namespace record must
contain only init, the shell and the Rust inventory helper after tests exit.

## Rust harness migration status

The Rust harness is under review; fresh qualification has not run yet. Existing
`evidence/` files remain the unchanged historical capture from source
`fb24f95f20d1152fc5921c4f0377add1051f42cb`. Their original source hashes and Python
harness identities are preserved. They are not current Rust-harness evidence and
schema-3 verification intentionally rejects them. Do not relabel these receipts.

The maintenance binary is `rubix-asset-fixture` in `tools/dev`. Its independent
oracles do not call the production asset parser. JSON rejects duplicate keys and
non-integer numbers; input reads reject symlinks and nonregular files and impose
byte limits. Exact test names, record order, typed values, commands, source
inventory, raw log hashes and cleanup outcomes are verified.

The default Rust test suite includes three mandatory published-evidence checks;
these remain failing until reviewed, clean-source Rust captures are published.
Synthetic mutation tests cannot satisfy those checks.

```sh
cargo test -p rubix-dev --bin rubix-asset-fixture --locked
cargo test -p rubix-dev --bin rubix-asset-fixture --release --locked
```

Capture commands below are for use after source and process-ownership review.
Every capture requires a new output directory and a clean relevant source tree.
Publication changes only the exact raw receipts/logs and factual documentation.

```sh
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- decode capture /tmp/rubix-decode-UNIQUE
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- decode verify /tmp/rubix-decode-UNIQUE
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- decode verify tools/assets-decode/rust-evidence
```

The build has an 1800-second/16 MiB limit and each runtime a 100-second/2 MiB
limit. Containers use UID65532, no network, read-only root, no capabilities,
no-new-privileges, init, 64 PIDs, 256 MiB memory, two CPUs and a 16 MiB tmpfs.
No host bind mounts are used. The driver records the copied relevant inventory,
actual revision, exact build/run commands, image identity and log hashes. It
removes only its owned container names and image and records cleanup failures
and remaining resources. Independently query the exact owned tag after capture.

The build and both runtime logs have hash-bound `*.command.json` records proving
exit zero, EOF, process-group absence and no timeout, cancellation or overflow.
Unconfirmed command settlement stops further actions, retains its input context
and records resource inventories as unknown. Settled cancellation permits only
owned cleanup with a fresh cancellation latch.

Publish fresh Rust captures under `rust-evidence/`. Keep historical `evidence/`
and its provenance unchanged. The mandatory current-capture gate reads only
`rust-evidence/`; source inventories exclude both historical and current outputs.
