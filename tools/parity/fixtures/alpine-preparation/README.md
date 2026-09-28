# Owned Alpine preparation feasibility

Procedure: checksum-verify the official Alpine3.24.2 cloud-init arm64 image; create
a private 8GiB overlay, firmware variables, cloud-init seed and fresh SSH identities;
boot with existing QEMU/HVF (2CPUs/2GiB), restricted user networking and loopback-only
SSH. Verify the exact SSH host key, guest identity and mount privilege. Record
OpenRC, APK installed inventory and cgroup mount/controllers before any preparation.

Then stage a pinned local APK repository and additive actual-baseline Go test;
record package/service/controller state before and after opt-in preparation and
repeated calls. Two separate fresh overlays independently exercise the naturally absent-controller
precondition. Exit zero
alone is not proof of a mounted hierarchy. Kernel/release qualification remains out
of scope; no Rust executor is implemented by this fixture.

All guest writes are disposable. No existing VM, host directory/device, Docker
socket, agent credentials or bridged interface is attached. Cache and raw evidence
are the only retained artifacts. Driver-owned QEMU group exit is verified before
private writable files are deleted. Command output is256KiB perstream; serial is1MiB;
SSH boot180s, cloudinit60s, command30s unless explicitlybounded, image download600s/1GiB.
Alpine orderly shutdown uses guest poweroff before inherited QMP/TERM/KILL fallback;
forced or unconfirmed cleanup fails qualification. Source derives from the reviewed
shared VM adapter; its hash and the local changes are retained separately.

The selected cloud image naturally boots without networking packages or enabled
cgroup controllers. No artificial removal, unmount or host cgroup mutation creates
that precondition. The fixture first calls the unchanged pinned Go
`CheckAlpineNetworking(false)` and `CheckCgroups(false)` and proves package,
runlevel and controller state unchanged. Opt-in calls install the two missing
networking packages and execute actual `rc-update add cgroups boot` and
`rc-service cgroups start`. Repeated calls preserve package state. A real reboot
changes the kernel boot ID and preserves the controller, runlevel and package
state; the final non-opt-in Go cgroup check succeeds.

`baseline_test.go` is an additive observation harness around the original
`preflight.go`; the source hash is recorded before compilation. It serializes
actual errors and leaves pass/fail judgments to the independent verifier. Source,
Go builder image, Alpine resolver image, cloud image, package index, all 20
resolved APK archives, oracle binary, local QEMU/SSH tools and firmware are pinned.
The resolver uses Alpine's signed index in a networkless container. Guest APK
installation uses only the staged local repository, so package access does not
require unrestricted guest networking. The exact installed before/after inventory
shows eight added packages and no removals. Resolving dependencies does not imply
all 20 packages were installed: the cloud image already supplies the remainder.

The cloud image reports `done` with a recoverable missing
`write-ssh-key-fingerprints` helper warning. Exit2 is admitted only with that exact warning, completed status and no top-level
or stage errors. The same structured validation runs after reboot; other warnings,
missing warnings for exit2, incomplete status or errors fail qualification. Root login requires an unlocked
account on this image. A fresh random password unlocks the private guest account,
while SSH password authentication remains disabled. Seed bytes, passwords and
private keys are deleted during teardown and excluded from evidence. Only the
fresh public SSH host key is retained, with an explicit fixture comment; local
user and host names are never used as key comments. Diagnostic publication additionally rejects
private credential material.

After the QEMU leader is reaped, process-group absence is inspected without sending
any nonzero signal. Unexpected group presence makes cleanup incomplete and retains
private files. This local correction is stricter than the inherited adapter's
historical post-reap group cleanup; the inherited source is recorded for provenance,
not executed as this fixture's lifecycle implementation. Normal shutdown, command
and console deadlines remain bounded. The outer caller owns this capture process;
forced host termination itself cannot guarantee its Python finally block runs.

Reproduce on the pinned Darwin arm64 host with existing QEMU/HVF:

```sh
python3 tools/parity/fixtures/alpine-preparation/prepare.py \
  --cache /tmp/rubix-alpine-input-cache --output /tmp/alpine-prepare-fresh
python3 tools/parity/fixtures/alpine-preparation/capture.py \
  --allow-privileged-vm --image-cache /tmp/rubix-vm-image-cache \
  --input-cache /tmp/rubix-alpine-input-cache --output /tmp/alpine-first-fresh
# Repeat capture with a different output directory; it creates another fresh VM.
python3 tools/parity/fixtures/alpine-preparation/verify.py
python3 -m unittest discover -s tools/parity/fixtures/alpine-preparation
python3 -O -m unittest discover -s tools/parity/fixtures/alpine-preparation
```

The input-cache argument is mandatory: an observation-only run cannot report
preparation success. Preparation requires the checksum-pinned index already in the dedicated cache;
the URL/hash and package closure are in `inputs.json`. Cache additions and evidence
are retained, while containers/images and private guest state are removed. Receipts
identify the actual Git revision plus full relevant working-source hashes; they
are working-tree snapshots, not claims of clean committed execution.

This qualifies the selected Alpine3.24.2/aarch64/OpenRC0.63.2/kernel6.18.52 guest
and the unchanged baseline's success, opt-in refusal, idempotence and reboot path.
It does not qualify Rust prerequisite execution, rollback after partially failed
APK/service operations, arbitrary Alpine releases, an IPv6-disabled kernel, full
cluster startup, other CPU/libc targets, or production service ownership. Baseline
ignores `rc-update` failure; a future executor must retain that action failure and
recheck controller state rather than treating service exit zero as sufficient.
