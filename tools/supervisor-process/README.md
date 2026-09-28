# Disposable process ownership qualification

The repository-owned capture, verification and synthetic children are Rust code in
`tools/dev/src/supervisor_fixture/`. After review and committing all source inputs:

```sh
cargo run -p rubix-dev --bin rubix-supervisor-fixture --locked -- process capture /tmp/unique-process-capture
cargo run -p rubix-dev --bin rubix-supervisor-fixture --locked -- process verify /tmp/unique-process-capture
cargo run -p rubix-dev --bin rubix-supervisor-fixture --locked -- process verify tools/supervisor-process/evidence --relevant-current
```

Thirteen cases preserve real 30-second TERM escalation, executable spawn failure,
a one-second readiness deadline, parent-reaped descendants and external sentinel
survival. The existing supervisor integration tests assert markers, heartbeat
advancement, readiness transitions, original startup errors and cleanup. Independent
Rust expectations additionally fix exact case order, types, exit/signal/ownership
facts and observed elapsed samples at or below 35,000 ms. Untimed zero samples do
not establish timing evidence. Final namespace enumeration permits only init,
driver shell and inventory helper.

The pinned Rust image builds the tests and Rust fixture. The nonroot runtime has
no network or host mounts, a read-only root, no capabilities, an init reaper,
96 PIDs, 512 MiB memory, two CPUs and private 64 MiB temporary storage. Build and
runtime bounds are 1800 seconds/16 MiB and 100 seconds/1 MiB. Both runs use the
same nonce-bound build binaries, whose hashes are checked against actual runtime
hashes. Raw output and exact command receipts bind exit status, EOF, process-group
absence and absence of timeout, cancellation or overflow.

Schema 3 requires committed relevant inputs and a complete copied source inventory.
Default verification compares all current inputs. `--relevant-current` preserves
historical provenance while requiring current workspace manifests, lock/toolchain,
Cargo configuration, all supervisor Rust files, all Rust maintenance implementation
and executable fixture files. Removed or changed relevant files require recapture.
Only elapsed measurements are omitted from repeated semantic equality.

Cancellation during cleanup remains a failed capture. Unknown process settlement
stops further commands and retains owned input directories. Settled cancellation
allows only owned Docker cleanup with a fresh latch. Cleanup failure or unknown
inventory cannot qualify success. No arbitrary host processes or containers are
removed. Docker's normal build cache remains.

Existing evidence is historical and is deliberately rejected by the new mandatory
schema-3 gate until reviewed Rust captures replace it. Never relabel old receipts.
`cargo test -p rubix-dev --bin rubix-supervisor-fixture --locked` includes mutation
checks and mandatory current evidence tests. No acceptance checks depend on debug
assertions. See `crates/rubix-supervisor/PROCESS.md`: this does not establish escaped
daemon containment, universal bounded kernel reaping or panic-abort cleanup.
