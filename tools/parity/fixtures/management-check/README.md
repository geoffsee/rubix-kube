# Actual management parser oracle

The pinned KubeSolo source and official Go image are checksum-bound in
`Capture.Dockerfile`. Cobra1.10.2/pflag1.0.10 and logging dependencies are fixed in
`go.mod` and checked against baseline `go.sum`. `extract.go` selects original Go
AST declarations for root/check/version commands, bool environment defaults,
logging configuration and the CLI Config type. UI source is copied unchanged.
Original file hashes and the executable digest are retained separately.

The test harness registers inert sibling commands and replaces only `runCheck`
with a public two-boolean marker. This qualifies actual parsing/defaults/precedence
and captured help/version/error channels without invoking host preparation. It is
not full management-command, root-help sibling-content or host-check execution
qualification. All 56 inputs have independently specified semantic outcomes in
`expected.json`; the actual stdout/stderr/exit observations remain in both raw logs.
No observed outputs are used to generate Rust expectations.

Each run is inside an owned nonroot Linux arm64 container with no host mount or
network, read-only root, bounded tmpfs,256MiB memory,2CPUs and64PIDs. Each subprocess
has3seconds, the Go test45seconds, and outer run60seconds/1MiB output. Build has
900seconds/8MiB. The capture helper attempts every owned cleanup and records unknown
inventories as failure. No credentials or host services are used. Ordinary Docker
build cache remains.

Rust parser tests consume cases plus independent expected outcomes. Separate injected
execution tests cover unsupported-preparation rejection, no probes on early exits,
port-phase guarding and truthful failure/success diagnostics. Real executable checks
require the subsequently integrated supplemental probes and disposable qualification.

```sh
```

Rust maintenance commands replace the former scripts:

```sh
cargo run --locked -p rubix-dev --bin rubix-platform-management -- verify management
cargo run --locked -p rubix-dev --bin rubix-platform-management -- capture-go management --output /tmp/new-management-go
cargo run --locked -p rubix-dev --bin rubix-platform-management -- qualify-linux management --output /tmp/new-management-linux
cargo run --locked -p rubix-dev --bin rubix-platform-management -- verify-go management /tmp/new-management-go
cargo run --locked -p rubix-dev --bin rubix-platform-management -- verify-linux management /tmp/new-management-linux
```

Historical provenance and raw captures remain immutable. Their Python harness hashes
identify the old execution; they do not qualify the current Rust tooling. Mandatory
current qualification tests require `evidence-rust/go` and `evidence-rust/linux`.
Fresh captures remain pending a combined source review. The current verifier binds
all copied workspace inputs, exact literal Docker commands, raw output and process
settlement receipts, independent semantic expectations, and empty cleanup inventories.
The Linux management fixture uses Rust-owned private chroot inputs and bounded
children, with no host mounts. The pinned Rust image replaces the Python runtime.
