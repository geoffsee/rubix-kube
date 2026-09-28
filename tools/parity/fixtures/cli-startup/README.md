# Startup parser ordering oracle

These 39 real invocations use the pinned KubeSolo binary built for revision
`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`, SHA256
`67348b2560d0f831de20ef722c53cc317e385ab7cfbf406633fbbb6a7b73f81e`.
The shared disposable parity runner executes only version, print, help or
parser-error paths, as UID65532 without capabilities, network or host mounts.
No bare no-argument startup is executed. An *empty argument* case means a
literal empty positional string accompanying `--print-config`.

The hand-written suite follows `cmd/kubesolo/main.go`, config flags and loader,
and pinned Kingpin2.4.0 `app.go`, `flags.go`, `parser.go`, `envar.go`. It independently
asserts exit codes and semantic output. Complete reviewed observations add exact
comparison: only the top-level `time` field in stderr JSON log lines is removed;
plain stderr, stdout YAML/help, version and all other fields remain unchanged.
Both current-runner captures r3/r4 must match. Raw streams, hashes, process cleanup,
runner diagnostics and artifact provenance are retained. No actual secrets are used.

Observed behavior important to E03.02:

- `--version` and `-v` bypass malformed boolean/integer/map environment, config-file
  access, and the deprecated `--full` warning. Explicit invalid integers, unknown
  flags and repeated aliases still fail before the version branch.
- `--help` and `--no-help` emit help to stderr and bypass malformed environment and
  explicit integer conversion errors. An unknown flag still fails. `-h` is unknown.
- Boolean flags accept `--name`/`--no-name`; `--name=false`, `=true`, an empty equals
  value and a following positional `false` fail. Positive/negative aliases share
  duplicate detection. Repeated scalar flags also fail.
- Empty string arguments and empty equals values work for string flags, while an
  empty integer fails. A terminal `--` after print succeeds; following `--version`
  is positional and fails. An empty positional string fails.
- Explicit primitive flags suppress parser environment defaults, but the later
  config loader still applies every present environment value before explicit
  flags. Invalid environment therefore fails printing even when overridden.
- A malformed primitive environment value fails before reading a directory as
  config; a malformed map environment value fails after file reading, so the
  directory error wins. Empty `KUBESOLO_FULL` is skipped by Kingpin; empty
  `KUBESOLO_CONFIG` selects its default path. Explicit `--config=` acts as missing.
  `--no-version --print-config` skips parser defaults but resolves loader defaults.

The baseline's source order is parse/context defaults/value conversion, custom
version branch, deprecated-full warning, file/env/flag loading, derive/validate,
configuration warnings, then print before bootstrap. Kingpin's environment-default
iteration is a Go map: competing malformed environment values need separate
characterization, not an invented stable ordering. Hidden completion/man-page
helpers, shell integration, startup with no command, native non-Linux behavior,
and actual runtime startup remain outside this bounded suite.

```sh
cargo run --locked -p rubix-dev --bin rubix-platform-fixture -- startup capture --driver /path/to/linux/rubix-parity \
  --artifact /path/to/pinned/artifact.json --output /tmp/new-startup-capture
cargo run --locked -p rubix-dev --bin rubix-platform-fixture -- startup verify --directory /tmp/new-startup-capture
cargo test --locked -p rubix-dev --bin rubix-platform-fixture
```

The Rust capture command runs the repository's parity orchestration directly and
requires `--driver` to identify the Linux Rust driver binary. Its digest must match
the driver observed inside the container. Current source hashes, all raw files and
both runner and driver identities are bound in `capture.json` schema 2.
Capture never refreshes expectations. Historical r1 used an older runner and four
help-fragment assertions missed Kingpin's `--[no-]` display form; r2 corrected those
assertions. Historical r3/r4 remain immutable prior evidence. New Rust captures
belong in `evidence-rust/first` and `evidence-rust/repeat`; mandatory tests fail until
both are freshly captured and verified. This covers startup/configuration compatibility.
