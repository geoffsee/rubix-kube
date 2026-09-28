# Explicit identity ELF qualification

The fixture downloads only the four API/Kine URLs and SHA-256 values pinned in
`experiments/component-boundary/inputs.json`. Cached regular files must match
before use. Downloads are capped at 256 MiB with HTTPS-only redirects, a
30-second connection timeout and a 300-second transfer deadline. Publication
never overwrites an existing cache entry.

The release-mode `elf_real` integration test inspects all four files twice
without executing them. An independent Rust byte parser checks ELF64 little
endian headers, program segments, interpreter and dynamic dependencies. The
fixture qualifies arm64 and amd64 observations only. Loader/ABI compatibility,
publisher authenticity, execution and install safety remain outside this proof.

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
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- elf capture /tmp/rubix-elf-UNIQUE /tmp/rubix-elf-cache
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- elf verify /tmp/rubix-elf-UNIQUE
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- elf verify tools/assets-elf/rust-evidence
```

Each Cargo invocation has a 900-second deadline and an 8 MiB output cap. The
receipt binds all four pinned digests, independent parsed fields, both raw logs,
relevant source hashes and actual revision. Sources and cached bytes are checked
again after inspection. Publish the receipt, two logs and their `*.command.json`
records; each command must prove exit zero, EOF, process-group absence and no
timeout, cancellation or output overflow. Downloaded
executables remain in the explicit temporary cache.

Publish fresh Rust captures under `rust-evidence/`. Keep historical `evidence/`
and its provenance unchanged. The mandatory current-capture gate reads only
`rust-evidence/`; source inventories exclude both historical and current outputs.
