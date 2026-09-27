# Unix signal bridge qualification

This fixture runs the real Unix signal integration test in a disposable Linux arm64
container. Twenty repetitions cover full and partial startup, SIGINT and SIGTERM,
single and mixed repeated signals (160 children), plus fatal worker completion
without a signal (20 children). Each child is reaped by its parent. Cooperative task
adapter cleanup is checked inside the child. This does not qualify the production
process adapter, detached descendants, a cluster, or a hard OS shutdown deadline.
See `crates/rubix-supervisor/SIGNALS.md` for interface and handler limitations.

Run from the repository root with Docker available:

```sh
python3 tools/supervisor-signals/capture.py --output /tmp/rubix-signals-new-capture
python3 -m unittest discover -s tools/supervisor-signals -p 'test_*.py'
python3 -O -m unittest discover -s tools/supervisor-signals -p 'test_*.py'
python3 -O tools/supervisor-signals/verify.py
```

The build uses a digest-pinned official Rust image and locked Cargo resolution.
The runtime container has no network or host mounts, runs as UID 65532, drops all
capabilities, and has a read-only filesystem, 16 MiB temporary filesystem, 512 MiB
memory, two CPUs and 128 process limit. Build output is bounded to 16 MiB and 30
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
bytes. Successful child output is summarized by the test, not individually
retained. Failure output propagates to the bounded run log.

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
