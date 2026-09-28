# Bounded merged output qualification

Use the Rust capture tool after reviewing and committing source inputs:

```sh
cargo run -p rubix-dev --bin rubix-supervisor-fixture --locked -- output capture /tmp/unique-output-capture
cargo run -p rubix-dev --bin rubix-supervisor-fixture --locked -- output verify /tmp/unique-output-capture
cargo run -p rubix-dev --bin rubix-supervisor-fixture --locked -- output verify tools/supervisor-output/rust-evidence --relevant-current
```

Thirteen adverse cases run twice in separate owned disposable Linux containers.
They preserve merged non-UTF8/NUL bytes, exact/excess budgets, simultaneous writes,
infinite output, empty/spawn-failed children, cancellation/readiness failures and
finite descendants. Safe Rust re-exec replaces the old forked fixture; the escaped
writer creates its own session, exits after two seconds, and must disappear before
namespace completion. No raw captured child bytes are published.

Independent Rust expectations require exact output status and deterministic sizes,
owner join and leader reaping facts, sub-five-second samples and namespace cleanup.
Cancelled cases independently require at most 64 bytes; cancellation can stop
retention at different prefixes. Repeat comparison omits their partial byte counts
and elapsed measurements, while raw evidence retains every observed count. Both test and
fixture binaries are bound to a fresh build nonce and both runtime hashes. Full
historical copied inputs and exact current relevant input inventories remain
verified; see the process fixture README for the source scopes and cleanup model.

The pinned Rust image runs as UID 65532 with no network or host mounts, read-only
root, no capabilities, init, 96 PIDs, 512 MiB memory, two CPUs and 16 MiB temporary
storage. Build bounds are 1800 seconds/16 MiB; each runtime is 100 seconds/1 MiB.
Commands have exact argument/log bindings and must settle successfully without
cancellation, timeout or overflow. Unsettled commands retain context and stop new
actions; cleanup inventories remain unknown and qualification fails.

Existing evidence remains historical. The mandatory Rust schema-3 evidence gate
requires published, verified captures from reviewed source. Run `cargo test -p rubix-dev
--bin rubix-supervisor-fixture --locked` for mutation checks and current evidence
gates. Ignored supervisor effects tests run only inside the disposable container.

Publish fresh Rust captures under `rust-evidence/`. Keep historical `evidence/`
and its provenance unchanged. The mandatory current-capture gate reads only
`rust-evidence/`; source inventories exclude both historical and current outputs.
