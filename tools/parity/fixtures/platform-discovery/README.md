# Platform discovery evidence

The Go fixture compiles the unchanged `internal/cli/detect/detect.go` from KubeSolo
`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`, fetched as a SHA-256 verified archive.
It adds a package test, preserving the actual private detection functions. Sixteen
scenarios create private filesystem landmarks inside child chroots in a disposable
Linux arm64 container. Nine `ForTarget` calls additionally capture supported and
rejected target strings. `expected.tsv` is the independently specified complete
observation list; Rust classifier tests consume it, with D01 musl extensions tested
separately. These are synthetic filesystem facts, not booted init systems or proof
of architecture compatibility. Cgroup/module/mount parsing has independent Rust
fixtures but is not claimed as actual Go execution in this capture.

The capture container has no network, host mounts or devices, a read-only root,
16 MiB private temporary filesystem, 256 MiB memory, two CPUs and 64 process limit.
It runs as root only to chroot its owned children, with SYS_CHROOT as its sole added
capability. It does not change a host namespace or start an init system. Both repeat
runs must return exactly one identical record matching the reviewed expectation.

The separate Linux Rust qualification uses a digest-pinned Rust image, locked Cargo,
strict Clippy and release test compilation. Runtime uses UID65532 with no capabilities,
network or host mounts and the same bounded resources. It exercises live read-only
namespace discovery and exact preservation of owned temporary file/symlink/missing
paths, including a relative spelling. Unit tests check permission denial and FIFO
rejection. It does not dump machine hostname, environment, mount table or credentials.
The raw log identifies both executed test binaries by SHA-256.

```sh
```

Builds are limited to 15 minutes/8 MiB output; Go runs to 45 seconds/1 MiB with a
30-second Go test deadline, Rust runtime to 60 seconds/1 MiB. Cleanup independently
attempts every owned container/image removal and inventories remaining resources;
failures retain a receipt and fail capture. Build cache may remain. Frozen evidence
contains exact source/harness/helper hashes, both raw Go records, Linux raw test
results and the full historical build inventory. The verifier binds current Rust
compiled inputs (manifests, lock, toolchain, Cargo config, platform Rust files) to the
capture; unrelated implementation source remains historical. Relevant changes require
recapture. Verification reads trusted repository evidence and does not authenticate
an external publisher. No E05 child or parent completion is claimed by this slice.

The command-landmark cases include nonexecutable files, a directory named systemctl,
and a systemctl present only in custom PATH. Baseline detection accepts the first
two and ignores caller PATH; this is an existence heuristic, not exec permission.

Rust maintenance commands replace the former scripts:

```sh
cargo run --locked -p rubix-dev --bin rubix-platform-management -- verify platform
cargo run --locked -p rubix-dev --bin rubix-platform-management -- capture-go platform --output /tmp/new-platform-go
cargo run --locked -p rubix-dev --bin rubix-platform-management -- qualify-linux platform --output /tmp/new-platform-linux
cargo run --locked -p rubix-dev --bin rubix-platform-management -- verify-go platform /tmp/new-platform-go
cargo run --locked -p rubix-dev --bin rubix-platform-management -- verify-linux platform /tmp/new-platform-linux
```

Historical provenance and raw captures remain immutable. Their Python harness hashes
identify the old execution; they do not qualify the current Rust tooling. Mandatory
current qualification tests require `evidence-rust/go` and `evidence-rust/linux`.
Fresh captures remain pending a combined source review. The current verifier binds
all copied workspace inputs, exact literal Docker commands, raw output and process
settlement receipts, independent semantic expectations, and empty cleanup inventories.
The Linux management fixture uses Rust-owned private chroot inputs and bounded
children, with no host mounts. The pinned Rust image replaces the Python runtime.
