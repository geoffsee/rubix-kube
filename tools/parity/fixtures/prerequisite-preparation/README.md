# Disposable preparation qualification

This harness builds the actual musl rubixctl executable and executes fixed-path
synthetic prerequisite helpers in fresh chroots inside one owned Linux container.
No host mounts/network or real package/service operations occur. It exercises
opt-out, both ordered actions, unchanged repeat, successful commands without
repair, failed registration suppressing service start, nonroot suppression, and
20 alternating real SIGINT/SIGTERM trials while a helper child is active. A second
signal is delivered if the CLI remains alive. Each signal trial requires the owned
child PID to be absent after CLI completion. These are trusted synthetic helpers,
not a proof of arbitrary daemon containment or real APK/OpenRC behavior.

The build image/toolchain and runtime image are digest pinned. Complete copied
source hashes are retained; current source binding includes manifests, lock,
toolchain/config and all platform/supervisor/management Rust inputs and consumed
fixtures. Five exact executable digests and all Rust test completions are checked.
Output, container CPU/memory/PIDs, private tmpfs and command deadlines are bounded.
The runtime has only SYS_CHROOT and SETUID capabilities, no network, no host mounts,
and a read-only root. Teardown records exact owned container/image identities and
errors. Historical management/parser captures are preserved separately.

```
cargo run --locked -p rubix-dev --bin rubix-prerequisite-fixture -- qualify --output /tmp/new-preparation-evidence
cargo run --locked -p rubix-dev --bin rubix-prerequisite-fixture -- verify --directory /tmp/new-preparation-evidence
cargo test --locked -p rubix-dev --bin rubix-prerequisite-fixture
cargo test --locked -p rubix-dev --bin rubix-prerequisite-fixture --release
```

The real Alpine package/OpenRC oracle is a separate disposable VM qualification.
This slice does not claim full node readiness, broader installers, modprobe or
external runtime/CNI ownership. Strict replay requires the current relevant
compiled source; a changed dependency/manifest legitimately requires recapture.

The Rust helper is statically cross-compiled in the pinned builder and enters each
private chroot through a single-threaded reexec. Its sole process waiter retains
uncertain owners and their directories. Historical `evidence-linux` remains an
immutable archive; the mandatory current-source gate requires schema-2 evidence
under `evidence-rust` and fails until a reviewed fresh qualification is published.
