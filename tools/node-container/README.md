# Disposable container host-preparation qualification

This fixture exercises the explicit `prepare_node_host` example. Production startup
still does not prepare a node. Two fresh disposable Alpine captures are published
from clean source `fb24f95f20d1152fc5921c4f0377add1051f42cb`.

The fixed preparation order is fresh assessment, actual network attempts, container
root propagation, actual process migration to `init`, and controller delegation.
The consumer and provider require quiescent startup; all runtime threads inherit the
intended namespace. The example uses a current-thread Tokio runtime.

`build.py --output DIRECTORY` builds four static aarch64 Linux artifacts with the pinned
Rust builder and Alpine runtime. Its isolated unprivileged runtime executes 38 injected
unit/integration tests plus complete help/version and the exact guest flags with
`--print-config`. Two additional builder-executed policy tests prove quiet exit2 and
terminal cancellation. A random builder nonce frames independent hashes of all four
executables; the verifier compares these with runtime hashes and exact artifact bytes.
Source, lockfile, toolchain, builder, consumer, harness and test inventories are bound.
A dirty exploratory build requires `--allow-dirty-hostsafe-build` and cannot qualify.

`capture.py --allow-privileged-vm --artifact-directory DIRECTORY --output DIRECTORY
--image-cache CACHE --input-cache CACHE` creates only a new owned QEMU overlay, keys,
firmware variables and seed. It requires a reviewed committed source snapshot and
verified artifacts. Its signed pinned APK closure includes util-linux-misc2.42.3-r1;
BusyBox's unshare does not provide the required cgroup namespace option. APK verification
and effect-free util-linux help/version evidence are retained in
`package-verification.txt`. No sibling fixture pins are changed.

Guest prerequisite effects are labeled setup: packages, cgroup mount, IPv4 forwarding,
xt_comment, parent controller enablement, two delegated non-root cgroups and a sibling
sentinel. Unchanged-state comparisons begin after this setup. Full preparation may
attempt guest-global modules; “unchanged outside the delegated root” applies to mount
and cgroup state plus external-runtime sentinel files, not to those intentional modules.
IPv6 disablement is not requested by this fixture.

The launcher writes its own PID to the delegated root, execs the pinned util-linux
`unshare --mount --cgroup --propagation private` without fork or PID namespace options,
records the complete recursively private mount tree before bind-remapping the delegated
root at `/sys/fs/cgroup`, and execs the candidate. Namespace setup is fixture work and
is not attributed to preparation under test.

The fixture-only nonblocking stdin protocol is READY/G, FIRST/R, SECOND/Q. An outside
observer verifies the candidate executable hash, PID/starttime, private mount and new
cgroup namespaces, non-root domain bind identity, initial PID membership and empty
subtree_control before G. After each pass it independently checks the same live PID
in init, absence from the delegated root, every recursive mount shared, complete
controller readback, empty init subtree_control and stable outer namespace/parent/
sibling/runtime sentinels. Starttime parsing handles the parenthesized process name.
R repeats through the full public API in the same live process; Q allows exit.

All gates and both preparations share one absolute 180-second cancellation deadline.
Individual gates allow at most30seconds. EOF/wrong byte/signal/timeout cannot authorize
the first preparation; late protocol failures preserve prior reports/effects. Signals
and timeouts feed the owned cancellation future, and preparation is always awaited
through cleanup. A non-Completed result never authorizes R. CleanupIncomplete retains
reports/listeners/observers through at most one aggregate second of observation-only
cleanup grace, then exits2 quietly without rendering or further observations, barriers,
signals, PID reacquisition or repeat. Later observer settlement does not promote success.
These deadlines do not preempt a synchronous kernel operation.

The guest separately labels the exact38 static injected tests, including mount failure/
uncertainty, failed migration, partial controller fallback and cancellation between
effects. These are doubles, never evidence of actual kernel mutation. Pre-G EOF,
wrong-byte and signal scenarios additionally exercise the consumer protocol. Actual
first/repeat reports require independent kernel-state observations; command exit0
is not treated as proof of a loaded module.

Verification is read-only and rejects duplicate/missing records, incomplete topology,
PID/namespace/root drift, report/readback disagreement, incorrect controller ownership,
changed source/artifacts, unresolved VM cleanup and credential leakage. Two fresh VM
captures and mandatory frozen-evidence checks are required before publication. The
inherited lifecycle helper owns QEMU by its original process handle/group, records
bounded logs, suppresses private credentials and removes the exact private directory.

Both captures completed first and repeat preparation in the same independently
observed live process. The kernel readbacks confirmed recursive shared mounts,
actual PID migration into `init`, and all seven advertised controllers enabled:
`cpu`, `cpuset`, `dmem`, `hugetlb`, `io`, `memory`, and `pids`. The repeat reused
`init`. Parent/sibling cgroup controls, the observer mount namespace, and external
runtime sentinel remained unchanged. Each capture also passed all 38 separately
labeled injected tests and the pre-effect protocol rejection cases.

The static consumer SHA256 is
`9f23c47bb4a87b12380c1886c55142efef641fe7debb7a2b7a5f134c9d09459c`,
13,156,744 bytes, target `aarch64-unknown-linux-musl`. All four artifact hashes,
clean build receipts, complete source inventories, and 62 captured files are
bound by the published evidence. All 25 synthetic rejection tests and five
mandatory published-evidence tests pass under normal Python and `-O`, as do the
current build and guest verifiers. Both guests powered off with QEMU exit 0;
owned process groups, private directories, and builder resources were independently
confirmed absent. This qualifies the explicit preparation boundary on this pinned
Linux fixture; production startup and full cluster operation remain outside scope.
