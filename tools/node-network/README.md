# Disposable host-network qualification

This fixture builds the explicit `prepare_host_network` example and checks shared
host-network effects only in fresh owned Alpine virtual machines. It does not
start Kubernetes or qualify an existing developer host. Production `rubix-kube`
main remains unchanged.

`build.py` uses the repository's digest-pinned Rust builder and Alpine runtime.
The pinned toolchain builds `aarch64-unknown-linux-musl`; strict Linux Clippy covers
the library, network tests and consumer. Fourteen injected host-safe tests and
help/version/print-config execute as an unprivileged user in a read-only container
without networking or capabilities. The print-config path uses the same explicit
IPv6/container/runtime arguments as the guest effects and verifies their resolved
values before any VM is launched. The artifact receipt binds the exact binary,
compiled inputs, toolchain/manifests/lockfile, example, tests and fixture sources.
`--allow-dirty-hostsafe-build` is only for exploratory compilation. VM captures
reject dirty build receipts and changed inventories.

After source review and commit, run the build and two independent captures:

```sh
python3 tools/node-network/build.py --output /tmp/network-build-UNIQUE
python3 tools/node-network/capture.py --allow-privileged-vm \
  --image-cache /tmp/rubix-vm-image-cache \
  --input-cache /tmp/rubix-alpine-input-cache \
  --artifact-directory /tmp/network-build-UNIQUE --output /tmp/network-first-UNIQUE
python3 tools/node-network/capture.py --allow-privileged-vm \
  --image-cache /tmp/rubix-vm-image-cache \
  --input-cache /tmp/rubix-alpine-input-cache \
  --artifact-directory /tmp/network-build-UNIQUE --output /tmp/network-repeat-UNIQUE
```

The sibling Alpine preparation fixture supplies pinned image, signed offline APK
closure, firmware, host tools, seed builder and reviewed ownership-aware lifecycle
helpers. Each capture owns its overlay, firmware variables, seed, fixture-only SSH
keys, restricted loopback forwarding and QEMU process. Guest input hashes are
checked before execution. Setup explicitly installs networking prerequisites,
starts the cgroup service, enables IPv4 forwarding and loads `xt_comment` for the
fresh preflight guard. These are fixture prerequisite effects, separate from the
API under test. The network operation still performs its real fresh assessment.

Nine cases run in each VM: effect-free help/version/print-config; an explicitly
substituted failed backend probe; failed-module continuation; five-second deadline
and 4096-byte output-limit handling; cancellation during the first module attempt;
then real preparation and real repetition after restoring original executables.
Doubles are explicitly hashed and labeled. Original file types, symlink targets,
resolved command bytes and paths must match after restoration. Doubles never count
as evidence of actual module loading. Deadline and cancellation cases must settle
owned cleanup before any further permitted work; cancellation suppresses later
module attempts and IPv6 writes.

Real first preparation starts all three guest IPv6 controls at `0`, records each
fixed module attempt and bounded after-observations, then requires `ObservedDisabled` for `all`, followed by `AlreadyDisabled` for `default`
and `lo`, and independent reads of all three control values as `1`. Linux propagates
the `all` write to `default` and each existing interface; subsequent fresh reads
correctly avoid redundant writes. See the [kernel sysctl documentation](https://www.kernel.org/doc/html/latest/networking/ip-sysctl.html#conf-all-disable-ipv6-boolean).
Reading `all=1` alone is not proof that IPv6 is disabled everywhere. Real repetition requires `AlreadyDisabled` for
all controls. Module failures can be warnings: successful command exit is never
interpreted as proof of loading. An external-runtime sentinel remains byte-identical
throughout. No reboot or persistence claim is made.

The process execution deadline plus supervised cleanup is an engineering bound,
not a hard kernel or whole-operation deadline. The outer guest command has a
420-second budget. Logs, file readers and binary reads have explicit size bounds.
Successful evidence requires orderly QEMU exit, absent owned process group, removed
private directory and no cleanup errors. The reviewed helper refuses destructive
post-reap signaling and retains uncertain resources instead of claiming cleanup.

Publish the complete first/repeat capture directories under `evidence/first` and
`evidence/repeat` without the external binary, then hash every published raw file in
`provenance.json`. `verify.py` checks exact inventory, build/source bindings, guest
input checks, typed outcomes, independent readbacks and cleanup. Run Python tests
and the verifier under both normal Python and `python3 -O`; checks use explicit
exceptions rather than removable assertions. Synthetic mutation tests exercise
verifier rejection and do not substitute for captured VM evidence.

Published qualification uses frozen source
`7be6768673263c26cf6fc3407b6650b239e41cf8`. Both fresh Alpine guests completed
all nine cases and orderly shutdown, with no recorded cleanup errors. The first
real pass in each guest returned 13 successful fixed nft-family command attempts;
loaded/built-in/available observations are separately retained, so successful exit
is not treated as proof of loading. All/default/lo readbacks were `1`; later
controls skipped writes after the all-control propagation, and the real repeat
skipped all IPv6 writes. Explicit deadline/output-limit/cancellation doubles
settled cleanup and the external-runtime sentinel remained unchanged.

The static candidate SHA256 is
`0214a3de0383f0ec03427bf922e27913577808125f8f372420bb78e1de9a8377`,
12,883,656 bytes, target `aarch64-unknown-linux-musl`. Its clean build binds the
same frozen revision and includes all 14 safe Rust tests plus effect-free CLI
checks with actual guest flags. Published first/repeat evidence retains exact
build receipts and source inventories along with every captured guest file.
Twenty synthetic rejection tests and four mandatory frozen-evidence tests pass
under normal Python and `-O`; current build and guest verifiers also pass.

Earlier failed exploratory guests remain outside the publication at
`/tmp/rubix-network-reviewed-{first,repeat}-20260928-r1`: the fixture used invalid
boolean CLI syntax and stopped before preparation effects. Both failed guests
also shut down cleanly. The corrected capture changes only fixture expectations
and arguments; production preparation behavior was unchanged.

The current refresh binds the decoder error-budget correction into the complete
source inventory. The network binary is byte-identical to the earlier qualification;
both guest captures were nevertheless rerun from this clean revision. This evidence
qualifies the network boundary and does not substitute for decoder-specific tests.
