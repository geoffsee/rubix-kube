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
python3 qualify.py --output /tmp/new-preparation-evidence
python3 verify_linux.py /tmp/new-preparation-evidence
python3 -m unittest discover -s . -p 'test_*.py'
python3 -O -m unittest discover -s . -p 'test_*.py'
```

The real Alpine package/OpenRC oracle is a separate disposable VM qualification.
This slice does not claim full node readiness, broader installers, modprobe or
external runtime/CNI ownership. Strict replay requires the current relevant
compiled source; a changed dependency/manifest legitimately requires recapture.
