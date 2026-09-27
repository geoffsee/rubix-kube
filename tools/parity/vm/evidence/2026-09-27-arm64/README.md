# Disposable VM evidence

Five fresh Debian 12 arm64 guests ran on Darwin arm64/QEMU11.1.1 HVF. The Go
startup suite passed; the same suite correctly failed the actual Rust placeholder.
The previously unsupported privileged suite ran as guest root and passed. Injected
setup and test failures retained diagnostics and returned failure. Every run records
successful guest tmpfs mount/unmount, ACPI powerdown, QEMU exit0, absence of the owned
process group and removal of the private overlay/keys/firmware directory.

Reproduce using the adapter README command and the artifact manifests from
`../../../../evidence/2026-09-27-arm64/`. Startup uses `tools/parity/startup.json`;
privileged uses `tools/parity/evidence/2026-09-27-arm64/privileged-gap-suite.json`.
Fault runs add `--inject-failure setup` or `--inject-failure test`.

Results include exact source, input, artifact, suite, tool and firmware hashes.
Diagnostics are compressed without modifying their bytes. These runs qualify the
adapter boundary on this host, not full-cluster behavior or another supported platform.

Six unit regressions exercise cache symlinks, failed cache publication, timeout suite
abort even with matching expected exits, and cleanup errors preserving failure reports.
