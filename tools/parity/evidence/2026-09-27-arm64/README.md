# Startup driver evidence

Executed on 2026-09-27, Darwin arm64 host / Docker Linux arm64. The same `startup.json`
suite ran against the pinned KubeSolo baseline and the real Rubix placeholder built
from `fa5e4ce8092849efd7e95b70c2e3c7ed1fc2208d`. Artifact manifests, build evidence,
source/suite hashes, raw diagnostics and verified cleanup are retained here. Large
executables are identified by hashes and reproducible preparation, not committed.

| Run | Exit / result | Meaning |
| --- | --- | --- |
| go-final | 0 / two cases pass | Pinned Go version and effective-config startup behavior satisfy the shared smoke assertions |
| rust-final | 1 / two cases fail | Actual placeholder lacks both behaviors; zero process exit alone does not pass parity |
| setup-failure | 1 / injected failure | Setup diagnostics preserved, no cases falsely pass, owned resources removed |
| test-failure | 1 / injected failure | Captured outputs retained, test failure propagated and owned resources removed |
| privileged-gap | 2 / explicit gap | Privileged case not executed by container adapter; unsupported is not pass |

Command pattern: `python3 tools/parity/run.py --artifact <artifact.json> --suite
 tools/parity/startup.json --output <new-directory>`, with `--inject-failure setup|test`
for fault runs and a suite requesting `privilege: privileged` for the gap run.
Reports record suite hashes; the startup suite is committed beside the driver. Every runner has zero orchestration
errors and records removal of its container, image and evidence volume. Independent
review checked source hashes, read-only root filesystems, bounded scratch, UID/capability
separation, safe evidence import and cleanup inventories. Eight Python regressions passed.

Reference preparation: `python3 tools/parity/prepare-upstream.py --output <new-dir>`.
Rust preparation: exact Git archive of the stated revision, digest-pinned official
Rust image `rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`,
`RUSTUP_TOOLCHAIN=1.97.1-aarch64-unknown-linux-gnu`, then
`cargo build --release --locked --offline -p rubix-kube`, without runtime execution.

This is partial E02.01 startup infrastructure. It does not close the issue, establish
configuration/manifests/certificates/lifecycle fixture coverage, or qualify a distribution.
The privileged VM adapter is separate work. E02.02 owns independent comprehensive oracles.
