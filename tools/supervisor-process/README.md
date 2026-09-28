# Disposable process-ownership qualification

Run the real Linux arm64 cases with:

```sh
python3 tools/supervisor-process/capture.py --output /tmp/unique-process-capture
python3 tools/supervisor-process/verify.py /tmp/unique-process-capture
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/supervisor-process -p 'test_*.py'
```

The driver builds the current Rust source using the checksum-pinned Rust 1.97.1
image in `Capture.Dockerfile`, then runs the ignored process test twice in the
pinned Python runtime image. It records the actual source base plus complete
copied working-tree hashes, image ID, executable digest, commands, raw records,
exit statuses, repeat comparison and cleanup. It does not label uncommitted
implementation as committed code. Compilation may fetch Cargo.lock dependencies;
runtime containers have no network and no user-provided host mounts.

Each run has a 100-second outer deadline and 1MiB streamed output budget. The build
has a 30-minute deadline and 16MiB streamed log budget. Control-command output is
also incrementally bounded. Runtime has init reaping, an isolated PID namespace,
512MiB memory, two CPUs, 96 PIDs, a read-only root and a private 64MiB /tmp. All
fixtures use synthetic values and ordinary unprivileged subprocesses. No real
Kubernetes components, credentials, host services or cluster startup are used.

Eleven cases include actual 30-second TERM escalation. Readiness markers,
parent-reaped descendant markers, heartbeat advancement and process exit statuses
are asserted by the Rust integration test. Independent Python assertions fix the
expected status/signal/cleanup facts for every case. Final namespace enumeration
must contain only init, the driver shell and its inventory helper. A driver
failure still triggers container/image removal and a cleanup receipt; unknown
inventory is failure. Docker's ordinary build cache remains.

The verifier rejects missing or duplicate cases, bool/int confusion, duplicate
JSON keys, nonfinite numbers/overflow, excessive input, failed run/build status,
stale driver/verifier/source hashes, raw-to-normalized drift, changed binary
identity, missing containment flags and incomplete cleanup. Elapsed time is the
only case-record field omitted from repeat equality; ignored TERM must still
respect the real grace interval. Random container/PID names and diagnostic timing
are retained in raw logs and receipts. They are not normalized into behavioral
claims. Frozen evidence lives under `evidence/`, excluded from its own source
inventory to avoid recursive hashes. Do not regenerate expected semantics from
observed outputs.

See `crates/rubix-supervisor/PROCESS.md` for ownership assumptions and limitations.
In particular, these tests do not establish escaped-daemon containment, universal
bounded kernel reaping, arbitrary hostile-child containment or panic-abort cleanup.

Verification has two explicit source scopes. The default CLI and a fresh capture
require every copied source hash to match the current checkout. For stored
qualification evidence after unrelated workspace work, use:

```sh
python3 tools/supervisor-process/verify.py --relevant-current tools/supervisor-process/evidence
```

This mode still validates the entire historical inventory against the separately
hash-bound `source-inventory.json`, its recorded source revision, the binary,
raw outputs, commands, repeated semantics and cleanup. It additionally requires
exact current hashes and inventories for workspace Cargo.toml/Cargo.lock/toolchain,
the supervisor package manifest and every supervisor src/tests file, and the
capture/verifier/runtime/inventory scripts and Dockerfile. It does not compare
unrelated crates, maintenance tools, general README files, PROCESS.md, or the
Python unit-test source to their historical contents. Their captured hashes remain
in the full historical inventory. This scope reflects what builds or executes the
process test; it does not assert historical files are the current checkout.

Normal Python test discovery verifies the real frozen evidence in this mode and
rejects mutated raw output, failed runs, missing historical inventory entries,
changed current process inputs and removed process tests. A narrower source scope
never bypasses historical provenance or relaxes runtime assertions.

The ordinary Python acceptance tests separately require every recorded elapsed
sample to be at most 35,000 ms, and reject a 35,001 ms mutation. They first verify
the frozen evidence and its current relevant inputs. The 37/38-second Rust
watchdogs and the structural verifier's wider timing tolerance retain overdue
results for diagnosis; they do not replace this stricter engineering gate.
This qualifies the observed samples, not a universal kernel cleanup bound.
Some process records use zero as an untimed placeholder. Those cases establish
behavior and cleanup, not a measured shutdown deadline; the gate does not turn
these placeholders into timing evidence.
