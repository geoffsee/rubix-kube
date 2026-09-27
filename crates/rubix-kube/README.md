# Startup command adapter

`cargo run -p rubix-kube -- --version`, `--help`, and `--print-config` execute the
startup-only adapter. A normal invocation resolves and validates configuration,
then exits 1 with `runtime startup is not implemented`. It does not create state,
start services, open cluster listeners, or prepare the host.

The adapter preserves the captured KubeSolo ordering: command grammar and
primitive flag parsing, version/help exit, deprecated `--full` warning, optional
configuration file, file/environment/explicit-flag resolution, validation,
warnings, and effective YAML printing. `-v` is the legacy version alias; `-h`
is an error. Boolean flags use `--name`/`--no-name`, not `--name=false`. Repeating
a canonical flag, including a positive/negative pair, is an error. Missing
configuration files are allowed, including an explicitly selected missing file.
The printed document includes effective values and can contain credentials;
`StartupAction` debug output deliberately omits the resolved configuration.

`execute` accepts an argument vector, environment snapshot, version string,
`StartupInputs`, and output writers. It returns `Exit` or a validated `Start`
action. This boundary lets tests prove that version/help do not read files or
probe the host, and that printing returns before the runtime startup boundary.
Writer failures propagate to the executable, which exits nonzero. The process
adapter only reads the selected file and host properties.

The process uses Go architecture names (`arm64`/`amd64`) and the baseline container
indicators (`/.dockerenv`, `/run/.containerenv`, or nonempty `container`). On Linux
it counts the affinity list from `/proc/self/status`, preserving Go's CPU-count
behavior when a cgroup quota is smaller than the allowed CPU set. If that list is
unavailable or malformed, and on other platforms, it uses Rust's
`available_parallelism`; that fallback has not been established as equivalent to
Go on every host. Arguments and configuration environment values must be UTF-8.
The actual Linux qualification is arm64; other operating systems and architectures
have not received this blackbox comparison.

Compatibility evidence is the pinned Go startup suite at
`tools/parity/fixtures/cli-startup` (the copied immutable suite is
`tests/fixtures/startup.json`) and the 51-case configuration command suite at
`tools/parity/fixtures/config-command.json`. Tests replay their independent exit,
stream, and semantic expectations. This establishes those observable cases, not
byte-identical diagnostics or every hidden Kingpin completion/help command.
Diagnostics preserve their expected classification and useful legacy wording,
while using the Rust configuration decoder's details. Structured log object key
order and timestamps are not a compatibility assertion. The approved effective
YAML renderer preserves escaped NEL data where the baseline's JSON-to-YAML path
loses it; see the configuration layer's compatibility notes.

Run focused checks with:

```sh
cargo test -p rubix-kube --locked
cargo clippy -p rubix-kube --all-targets --all-features --locked -- -D warnings
```

Runtime startup, live CRI connection, host installation, and cluster lifecycle
remain separate implementation and qualification work.

`tests/evidence/linux-arm64.json` records the disposable comparison after the lower-layer review fixes:
Go and Rust each passed all 39 startup and 51 configuration cases, and all 90
corresponding stdout files were byte-identical. Its compressed public archive
contains raw streams, runner/process cleanup receipts, exact suite inputs,
artifact descriptors, build commands, and the copied source hash inventory.
All owned test containers, images, and evidence volumes were independently
checked absent after the runs. The Rust artifact is honestly identified as an
uncommitted implementation over base `57856b6cea182ec8f052affe30e6bcb592b42baf`,
including the reviewed configuration decoding, secret-debug, and hostname fixes.
No binary or private keys are committed. Reproduce with a release Linux arm64
binary described by the checksum-bound artifact schema, then run:

```sh
python3 tools/parity/run.py --artifact /path/to/artifact.json \
  --suite crates/rubix-kube/tests/fixtures/startup.json --output /tmp/startup-run
python3 tools/parity/run.py --artifact /path/to/artifact.json \
  --suite tools/parity/fixtures/config-command.json --output /tmp/config-run
```

Run those commands separately for the pinned Go and new Rust artifacts, using
fresh output directories. The suite arguments contain only print/version/help
and deliberate failure paths; do not substitute a normal Go startup invocation
on the host.
