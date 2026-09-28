# Unix signal bridge qualification

The Rust fixture qualifies real signals exclusively in owned disposable Linux
containers. Each of two runs retains twenty cooperative repetitions (160 signal
and 20 fatal-worker cases), twenty repetitions each of full/partial/fatal owned
startup and one actual 30-second TERM-ignore escalation. A separate sentinel must
survive external-service stop with advancing heartbeat. Every owner is joined and
leader reaped; namespace inventory rejects residue. This is no cluster, escaped
daemon or hard kernel shutdown guarantee. See `crates/rubix-supervisor/SIGNALS.md`.

After reviewing and committing source inputs:

```sh
cargo run -p rubix-dev --bin rubix-supervisor-fixture --locked -- signals capture /tmp/unique-signal-capture
cargo run -p rubix-dev --bin rubix-supervisor-fixture --locked -- signals verify /tmp/unique-signal-capture
cargo run -p rubix-dev --bin rubix-supervisor-fixture --locked -- signals verify tools/supervisor-signals/evidence --relevant-current
```

Rust signal streams and synthetic process children replace the interpreter fixture.
Existing production tests still require the full-startup coordinator gate, partial
and fatal dependent absence, exact primary adapter error and explicit Forced
cleanup for escalation. Independent expectations require exact ordered 61 owner
records, both completion summaries and every measured sample at or below 35,000 ms;
forced escalation must also take at least 29 seconds. Wider production watchdogs
retain diagnostic failures and cannot replace this engineering gate.

Both supervisor test binaries and the fixture are nonce-bound to builder hashes and
actual runtime hashes. Source inventory binds all workspace manifests, lock/toolchain,
Cargo configuration, supervisor Rust inputs and Rust maintenance implementation.
Default verification compares every copied input; `--relevant-current` permits
unrelated historical source changes while preserving exact relevant inventories.
Every raw log, command and command settlement record is hash-bound. The verifier
requires the exact eight-file evidence set; old four-file provenance remains
historical and must never be relabeled as new Rust execution.

The pinned Rust container runs as UID 65532 with no network or host mounts,
read-only root, no capabilities, init reaping, 16 MiB temporary storage, 512 MiB
memory, two CPUs and 128 PIDs. Build bounds are 1800 seconds/16 MiB; each run is
180 seconds/1 MiB. Control commands have independent 30-second/64 KiB bounds.
Cancellation, incomplete settlement and failed cleanup cannot publish success;
unknown settlement retains context and stops further actions. Only owned resources
are removed. The source-bound `kill.sh` preserves the shell builtin signal helper.

Historical evidence remains untouched until review and fresh Rust capture. The
mandatory schema-3 evidence gate currently rejects it. Run `cargo test -p rubix-dev
--bin rubix-supervisor-fixture --locked`; mutation checks remain active in release.
Never rewrite failed receipts or generate expected semantics from captured results.
