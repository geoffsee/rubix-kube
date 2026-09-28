# Unix signal bridge qualification

This fixture runs real Unix signal tests in a disposable Linux arm64 container.
It retains twenty repetitions of the cooperative task cases (160 signal cases and
20 fatal-worker cases). A separate test exercises the production signal bridge and
owned-process adapter together: twenty repetitions each of full startup, partial
startup and fatal readiness failure, plus one real 30-second TERM-ignore escalation.
Full and partial cases alternate SIGINT/SIGTERM and send a second mixed signal.
A separate owned sentinel stays alive throughout; external-service stop must leave
its heartbeat advancing. Every launched owner is joined and its leader reaped.
A final PID-namespace inventory rejects residue. This does not qualify escaped
process groups, a cluster, or a hard kernel shutdown deadline.
See `crates/rubix-supervisor/SIGNALS.md` for interface and handler limitations.

Run from the repository root with Docker available:

```sh
python3 tools/supervisor-signals/capture.py --output /tmp/rubix-signals-new-capture
python3 -m unittest discover -s tools/supervisor-signals -p 'test_*.py'
python3 -O -m unittest discover -s tools/supervisor-signals -p 'test_*.py'
python3 -O tools/supervisor-signals/verify.py
```

The build uses a digest-pinned official Rust image and a pinned Python runtime image and locked Cargo resolution.
The runtime container has no network or host mounts, runs as UID 65532, drops all
capabilities, and has a read-only filesystem, 16 MiB temporary filesystem, 512 MiB
memory, two CPUs and 128 process limit, with an init reaper. Build output is bounded to 16 MiB and 30
minutes; runtime output is bounded to 1 MiB and three minutes. Metadata and each
cleanup command have separate 30-second bounds. The test's line reader consumes a
trusted finite fixture; it does not offer hostile-output containment. The outer
capture helper enforces its byte limit while streaming.

The committed `evidence/` contains the raw build/run logs, full historical copied
source inventory and cleanup receipt. `provenance.json` binds their exact file set
and hashes. The receipt records the actual test binary hash, image identity,
platform, source revision and source/helper hashes. Its
`uncommitted_implementation: true` accurately records that the added signal code
was tested before its implementation commit; copied source hashes identify those
bytes. Cooperative child output is summarized by the original test, not individually
retained. The combined test emits all 61 owner results, timing and completion flags. Failure output propagates to the bounded run log.

Verification reads the real historical evidence and independently requires its
expected repetition counts, completion marker, binary digest and empty cleanup
inventories. It binds current supervisor Rust inputs, all copied Cargo manifests,
the complete lockfile, toolchain and Cargo config to the tested bytes. Changes to
these inputs require recapture, including dependency changes elsewhere in the
workspace. Unrelated implementation source remains visible in the historical
inventory without requiring current equality. Documentation added after capture
is not presented as an input to the captured binary. Mutation tests reject changed
counts, cleanup status, raw completion/binary claims, dependency/source drift and
nonfinite or duplicate-key JSON; the tests also exercise receipt publication when
every Docker cleanup/inspection fails. Checks remain active under `python -O`.

`verify.py` validates trusted repository evidence; it does not authenticate an
external publisher or execute artifacts. To refresh evidence, review a new
successful capture, replace the four evidence files and regenerate the four-file
SHA-256 provenance inventory. Never rewrite a failed receipt as success.

The combined cases reuse `tools/supervisor-process/fixture.py` unchanged. The receipt
binds both executed binaries; the source inventory binds the shared fixture and
namespace helper. Force timing is checked between 29 and 38 seconds as an observed
scheduler tolerance, not a guarantee that every kernel will finish cleanup.
Both process and signal evidence must be refreshed after supervisor source, tests,
features or dependency inputs change. Runtime signal delivery occurs only inside
the disposable container; ordinary host test discovery leaves the combined test ignored.

Full-startup signal delivery waits for a one-shot graph gate that depends on the
owned dependent process: this proves the coordinator accepted its readiness,
not merely that the Python fixture wrote a marker. Partial and fatal cases prove
that gate and dependent owner never start. Fatal cases preserve the exact primary
adapter error; force cases require the explicit `Forced` cleanup record rather
than claiming ordinary graceful cleanup. The slim runtime's source-bound `kill.sh`
invokes the shell's kill builtin for the unchanged original signal fixture.

The ordinary Python acceptance tests separately require every recorded elapsed
sample to be at most 35,000 ms, and reject a 35,001 ms mutation. They first verify
the frozen evidence and its current relevant inputs. The 37/38-second Rust
watchdogs and the structural verifier's wider timing tolerance retain overdue
results for diagnosis; they do not replace this stricter engineering gate.
This qualifies the observed samples, not a universal kernel cleanup bound.
