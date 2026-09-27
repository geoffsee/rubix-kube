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
python3 tools/parity/fixtures/cli-startup/capture.py \
  --artifact /path/to/pinned/artifact.json --output /tmp/new-startup-capture
python3 -O tools/parity/fixtures/cli-startup/verify.py /tmp/new-startup-capture
python3 -m unittest discover -s tools/parity/fixtures/cli-startup -p 'test_*.py'
```

`--runner PATH` selects an explicit existing parity runner; the default is the
repository's `tools/parity/run.py`. Tests normally bind that runner's current source.
When inspecting from an older detached tree, `RUBIX_PARITY_RUNNER` can select the
exact current-main runner used for capture. This does not bypass hash checks.
Capture never refreshes expectations. Initial r1 used an older runner and four
help-fragment assertions missed Kingpin's `--[no-]` display form; r2 corrected those
assertions. Final r3/r4 use the current main runner, and only those are qualification
evidence. This is startup/configuration compatibility, not runtime conformance.
