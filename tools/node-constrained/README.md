# Read-only sysctls and external resource preservation

This fixture exercises the approved `prepare_node_host` example with explicit
`--no-container-mode --disable-ipv6` and a host-owned Unix endpoint. It adds no
Rust API, runtime execution, service installation, or production startup behavior.
Historical guest captures are preserved below. Current Rust qualification is required separately before this draft can pass its publication gates.

The Rust `rubix-node-fixture constrained` adapter reuses the reviewed container
builder, tests, command proofs and owned VM lifecycle. Its pinned builder produces
four approved candidate/test artifacts plus the static `constrained-guest` observer.
All five bytes are bound by the builder nonce, isolated runtime hashes, clean current
source inventory, image identity, command receipts and cleanup. The observer does
not require an interpreter in the guest. Exact Debian musl 1.2.3-1 arm64 package
hashes are pinned in `build-packages.sha256`; the base Rust image binds the C compiler.
Only the musl target receives `CC_aarch64_unknown_linux_musl=musl-gcc`.

The VM driver reuses the pinned Alpine 3.24.2 aarch64 image, signed 31-APK closure,
util-linux-misc 2.42.3-r1 and owned QEMU lifecycle from container qualification.
Only a fresh disposable guest receives setup effects: APK installation, cgroups
service start, IPv4 forwarding, xt_comment loading and explicitly initialized IPv6
controls. Setup is not attributed to the candidate. No ordinary host mount, sysctl,
cgroup, process, runtime, service or existing VM is changed.

Within that guest, each candidate uses a new mount namespace made recursively
private before a bind-remount of `/proc/sys` as read-only. It retains its actual PID
through exec; no PID or cgroup namespace is created. The outside observer verifies
its live PID/starttime/executable, namespace, cgroup membership, complete mount table,
read-only sysctl subtree and actual scalar values at READY, FIRST and SECOND. The
consumer's existing G/R/Q protocol releases effects and repeats in the same process.
A successful syscall/report is never substituted for independent kernel readback.

The four cases are deliberately distinct:

- `correct`: all/default/lo are initially 1; both actual passes report AlreadyDisabled,
  and independent values stay 1 without a sysctl write.
- `needs_write`: all/default/lo are initially 0; both actual passes report all three
  failed write attempts and unchanged 0 readbacks. The existing typed provider maps
  EROFS to `WriteFailed(Io)`; the independent read-only mount observation supplies
  context. Completed means the sequence settled with warnings, not IPv6 disabled.
- `guard`: a private `/usr/sbin` bind selects an explicitly labeled failing iptables
  version double. GuardStopped suppresses all module/sysctl/container effects.
- `cancel`: a private `/usr/sbin` bind selects an explicitly labeled modprobe wait
  double. After its first marker, the observer signals the actual candidate, awaits
  settlement, and requires exactly one cancelled/reaped/joined module attempt, no
  later module observations or sysctls, retained effects and the double PID absent.

The real cases use actual fixed nft-family module commands, with separately reported
loaded/built-in/index observations. Neither successful command exit nor this Alpine
kernel establishes an nftables-only kernel. That qualification remains outstanding.
The doubles do not establish actual iptables or module behavior.

An owned keeper outside every candidate namespace holds a Unix socket at the
configured endpoint. It accepts no requests and implements no CRI. The observer
compares its live PID/starttime/executable/namespace/cgroup, socket identity/type,
and config identity/content before, during and after every case. Service-directory,
CNI-directory and external-state inventories, the outside mount table and root
controller settings remain unchanged after setup. At the end only the fixture
closes and reaps its keeper and removes its socket. This proves ownership preservation,
not external-runtime readiness or adoption.

An operator on a read-only sysctl host must preconfigure required values through the
host administrator; warning completion does not repair a read-only setting. Missing
Alpine tools require the existing explicit prerequisite opt-in. Unknown observations
remain unknown. An external endpoint does not waive independent Docker-conflict or
host-network requirements. Container preparation stays NotRequested (or NotStarted
when an earlier phase stops); the fixture does not change propagation or cgroups on
behalf of the candidate.

Each consumer retains its existing absolute 180-second cancellation deadline and
30-second gates. The observer has a 480-second budget inside the driver's 600-second
command bound, capped files/records and finite process waits. Signals request owned
cancellation and await settlement; no active preparation future is abandoned. The
approved quiet exit2 cleanup-observer grace remains unchanged. Such an outcome cannot
pass this fixture. Synchronous kernel I/O still has no hard interruption guarantee.
All final VM cleanup is checked through the original owned QEMU process/group and
exact private resources; cleanup errors remain failures.

After independent review and a clean source commit, run:

```sh
cargo build -p rubix-dev --bin rubix-node-fixture --locked
# Builds and verifies all five current-source artifacts in an owned builder.
target/debug/rubix-node-fixture constrained build /tmp/constrained-build
target/debug/rubix-node-fixture constrained capture-vm --allow-privileged-vm \
  --artifact-directory /tmp/constrained-build \
  --input-cache /tmp/rubix-container-input-cache \
  --image-cache /tmp/rubix-vm-image-cache --output /tmp/constrained-first
target/debug/rubix-node-fixture constrained verify /tmp/constrained-first
```

Repeat in a second fresh guest, publish complete captures as `rust-evidence/first`
and `rust-evidence/repeat`, and bind every raw file in `rust-provenance.json` using
the shared evidence inventory. Then run `constrained verify-published` and the
workspace debug/release tests. Five mandatory build/guest/provenance gates fail
when current evidence is missing or stale. Synthetic regression tests never replace
actual captures. The constrained command alone has an 8 MiB SSH output cap, recorded in its raw command receipt and checked by the verifier. Other commands retain the 256 KiB default. This preserves bounded failure diagnostics after the 235,635-byte historical observation stream; excess output fails rather than truncates success.
This is bounded #47/E05.03 evidence, not closure of cluster/runtime readiness or
nftables-only-kernel qualification.

The following publication is historical and is never accepted as Rust execution
evidence. Its bytes and original revisions remain unchanged.

Published qualification uses clean capture source
`895ff1e92ad079afb08abb14df0ea7c4f1e7501d`. Both fresh guests passed all four
cases; each real read-only case completed two preparations in the same observed
live process. Already-disabled values stayed 1; failed writes stayed 0 with all
three truthful warning outcomes. The live external keeper, socket/configuration,
service/CNI inventories, outside mounts and cgroup observations remained unchanged
through real success, failed guard and in-flight cancellation. These observations
do not establish CRI readiness or an nftables-only kernel.

The executed consumer was built at original revision
`bfb31a1914e0d8a365e5b32ab320d4131f5d8d12`, SHA256
`9f23c47bb4a87b12380c1886c55142efef641fe7debb7a2b7a5f134c9d09459c`,
13,156,744 bytes. Its historical build receipts and current compiled-source bindings
are retained separately from the new capture revision. Both QEMU processes exited
0 after guest poweroff, with no cleanup errors; owned process groups and private
directories were independently absent. All 27 safe regressions and five mandatory
published-evidence gates pass normally and with Python `-O`, along with the current
verifier.

The earlier first attempt remains preserved at
`/tmp/rubix-constrained-qualified-first-20260928-r1` as a failed capture. Its real
read-only cases passed, but copying large `/usr/sbin` executables to stage a double
hit the unchanged file-size bound before the guard candidate started. The corrected
fixture uses a bounded symlink view over a private read-only original bind, resolving
relative aliases before overlay and replacing only the fixed double. Regression
tests cover large originals under the original limit and internal/external aliases.
No production behavior or output/file limit was changed; r1 is not qualifying evidence.

The aligned refresh incorporates the qualified image-layer inventory through its
approved layer build and two new guests. Earlier successful r2 captures remain at
`/tmp/rubix-constrained-qualified-{first,repeat}-20260928-r2`, with their actual
capture revision `cb96f5d6ddb8d7ad492b51820104446a04449d0c` and original build
`fb24f95f20d1152fc5921c4f0377add1051f42cb` unchanged. Their records were not relabeled.
The consumer bytes are identical; the current publication binds the new complete
compiled-source inventory and actual fresh observations.
