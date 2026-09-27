# Disposable privileged Linux VM adapter

This adapter runs the shared E02.01 artifact/suite contract inside a **new Linux VM**.
Privilege applies inside that VM, never inside an existing Docker Desktop Linux host.
The first backend supports Darwin arm64, QEMU with HVF and the local
QEMU EDK2 aarch64 firmware. It does not install host software or request host root.

```sh
python3 tools/parity/vm/run.py --allow-privileged-vm \
  --artifact /tmp/kubesolo-reference/artifact.json \
  --suite tools/parity/startup.json \
  --output /tmp/parity-new-vm \
  --image-cache /tmp/rubix-vm-image-cache
```

The explicit `--allow-privileged-vm` flag is required. Output must be a new directory.
The input cache retains the verified public base image and pinned pure-Python
`pycdlib 1.14.0` wheel for later runs. The verified wheel creates the cloud-init ISO
without installing a host package or mounting an image; native `hdiutil makehybrid`
was observed to fail with `Operation not permitted` in the initial preparation trial. It
is not a test VM and is never booted writable. Every run allocates an owned qcow2
8 GiB overlay, copied firmware variables, cloud-init ISO, SSH credentials and QMP
socket in a new private temporary directory. That directory is removed after QEMU
has exited. No existing disk, cluster, service, guest or host directory is attached.

The committed inputs pin Debian 12 genericcloud arm64 build `20260923-2610` by its
full official SHA-512 checksum. Download and cache use the same verification. This
checks official HTTPS content against recorded hashes; it is not a release-signature
verification claim. Local QEMU/tool paths, content hashes, version and firmware hashes
are recorded, so the locally installed hypervisor is not silently treated as a pinned
release dependency.

Networking uses QEMU user networking with `restrict=on` and one loopback-only random
SSH forward. There is no bridge, TAP interface, guest network access to host services,
shared filesystem, Docker socket, or host-device passthrough. SSH uses an ephemeral
client key, a separately generated guest host key, and strict verification against
that exact host key. No host SSH configuration, agent, known_hosts file or credentials
are inherited. Guest keys and the private seed are not exported as evidence.

The VM has two CPUs and 2 GiB RAM. Cloud-image download is limited to ten minutes
and 1 GiB; SSH readiness to three minutes; cloud-init completion to one minute.
Every command has a deadline and a 256 KiB output limit. The console log retains
at most 1 MiB. The guest's writable root disk is bounded by the 8 GiB overlay's
virtual size. Source/tool/image identities, VM arguments, guest environment and
all command output stay in the evidence directory.

After SSH readiness, a tmpfs mount/unmount probe verifies privileged guest host
preparation without changing the host. The artifact is copied into the guest and
its checksum reverified. The existing shared expectation evaluator remains on the
host: guest-controlled report files are never imported. Cases requesting
`privilege: "privileged"` run as guest root; ordinary cases run as guest `nobody`.
Both use the same argument and expectation format. Cases in one suite share a VM,
so stateful lifecycle suites must describe that state explicitly. A timeout aborts
the run and destroys the entire guest; it is not a successful lifecycle check.
Guest exit statuses 124 and 137 are reserved for timeout/SIGKILL failure and always
fail and abort the suite, even if a case lists one as its expected exit. The outer
host SSH deadline independently bounds a guest that interferes with its timeout tool.
Per-case elapsed time and unexecuted case IDs remain in the report.

`--inject-failure setup` stops after the guest privilege probe. `--inject-failure test`
stops after case execution. Both retain console/SSH/guest diagnostics and still tear
down only the owned VM. Cleanup first requests ACPI powerdown through the private
QMP socket, then applies bounded termination if needed. Escalation or an abnormal
QEMU exit makes the run fail. Process-group absence and private-directory removal
are recorded. The public input cache and evidence are the only retained files.

This supplies a privileged fixture boundary, not full runtime qualification. The
reference `external_deps` binary has no embedded runtime assets. Full cluster boot,
networking, recovery and conformance require their actual scenario implementations,
assets and acceptance evidence. Passing `--version` in a VM cannot satisfy those
criteria. Initial support is Darwin arm64/HVF; other hosts remain explicit gaps.

Cache entries reject symlinks, and newly verified wheel content is published by
atomic replacement. Cleanup errors are aggregated; failure of one step does not
skip remaining diagnostics/termination/report attempts. Writable VM files remain
available for recovery if QEMU termination cannot be confirmed, rather than deleting
a still-open guest disk.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/parity/vm -p 'test_*.py' -v
```
