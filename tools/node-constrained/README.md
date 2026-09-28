# Read-only sysctls and external resource preservation

This fixture exercises the approved `prepare_node_host` example with explicit
`--no-container-mode --disable-ipv6` and a host-owned Unix endpoint. It adds no
Rust API, runtime execution, service installation, or production startup behavior.
No successful actual guest evidence is published yet.

The existing `tools/node-container` build verifier validates all four approved
static artifacts and their original clean build revision, nonce-delimited builder
hashes, isolated-runtime hashes, source inventory and cleanup. This fixture neither
relabels that historical build nor rebuilds it implicitly. The current compiled
source must still match that approved inventory. New fixture sources, delegated
builder/verifier sources, actual executable bytes, pinned image/APKs/namespace tool,
guest inputs and raw observations are bound separately in each capture receipt.
Publication README/provenance are metadata; executable fixture/test files are bound.

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
python3 tools/node-constrained/capture.py --allow-privileged-vm \
  --artifact-directory /tmp/rubix-container-qualified-build-20260928-r1 \
  --input-cache /tmp/rubix-container-input-cache \
  --image-cache /tmp/rubix-vm-image-cache --output /tmp/constrained-first
```

Repeat in a second fresh guest, publish complete captures as `evidence/first` and
`evidence/repeat`, bind every raw file in `provenance.json`, then run `verify.py` and
all `test_verify.py` tests normally and with Python `-O`. Mandatory published checks
must pass; synthetic mutation tests never substitute for actual captures. This is
bounded E05.03 evidence, not closure of E05.02/E05.03 or their parent integration gates.
