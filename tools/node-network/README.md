# Disposable host-network qualification

This fixture builds the explicit `prepare_host_network` example and checks shared
host-network effects only in fresh owned Alpine virtual machines. It does not
start Kubernetes or qualify an existing developer host. Production `rubix-kube`
main remains unchanged.

The Rust fixture build command uses the repository's digest-pinned Rust builder and Alpine runtime.
The pinned toolchain builds `aarch64-unknown-linux-musl`; strict Linux Clippy covers
the library, network tests and consumer. Fourteen injected host-safe tests and
help/version/print-config execute as an unprivileged user in a read-only container
without networking or capabilities. The print-config path uses the same explicit
IPv6/container/runtime arguments as the guest effects and verifies their resolved
values before any VM is launched. The artifact receipt binds the exact binary,
compiled inputs, toolchain/manifests/lockfile, example, tests and fixture sources.
Builds and VM captures reject dirty source and changed inventories.

After source review and commit, run the build and two independent captures:

```sh
cargo run -p rubix-dev --bin rubix-node-fixture --locked -- network build /tmp/network-build-UNIQUE
cargo run -p rubix-dev --bin rubix-node-fixture --locked -- network capture-vm --allow-privileged-vm \
  --image-cache /tmp/rubix-vm-image-cache \
  --input-cache /tmp/rubix-alpine-input-cache \
  --artifact-directory /tmp/network-build-UNIQUE --output /tmp/network-first-UNIQUE
cargo run -p rubix-dev --bin rubix-node-fixture --locked -- network capture-vm --allow-privileged-vm \
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

Publish the complete first/repeat capture directories under `rust-evidence/first` and
`rust-evidence/repeat` without the external binary, then hash every published raw file in
`rust-provenance.json`. `network verify-published` checks exact inventory, build/source bindings, guest
input checks, typed outcomes, independent readbacks and cleanup. Run `cargo test -p rubix-dev --bin rubix-node-fixture --locked` for independent semantic and mutation checks. Synthetic mutation tests exercise
verifier rejection and do not substitute for captured VM evidence.

Historical qualification used frozen source
`fb24f95f20d1152fc5921c4f0377add1051f42cb`. Both fresh Alpine guests completed
all nine cases and orderly shutdown, with no recorded cleanup errors. The first
real pass in each guest returned 13 successful fixed nft-family command attempts;
loaded/built-in/available observations are separately retained, so successful exit
is not treated as proof of loading. All/default/lo readbacks were `1`; later
controls skipped writes after the all-control propagation, and the real repeat
skipped all IPv6 writes. Explicit deadline/output-limit/cancellation doubles
settled cleanup and the external-runtime sentinel remained unchanged.

The static candidate SHA256 is
`e9d43d8f1105c4a22547e5e3bcb1264b80f65127cd5efb8c04192ad70930ddca`,
12,883,248 bytes, target `aarch64-unknown-linux-musl`. Its clean build binds the
same frozen revision and includes all 14 safe Rust tests plus effect-free CLI
checks with actual guest flags. Published first/repeat evidence retains exact
build receipts and source inventories along with every captured guest file.
Those historical results do not satisfy the new mandatory Rust receipt gates.

Earlier failed exploratory guests remain outside the publication at
`/tmp/rubix-network-reviewed-{first,repeat}-20260928-r1`: the fixture used invalid
boolean CLI syntax and stopped before preparation effects. Both failed guests
also shut down cleanly. The corrected capture changes only fixture expectations
and arguments; production preparation behavior was unchanged.

The current refresh binds the container preparation additions and Rust dependency
features into the complete compiled source inventory. Both guest captures were
rerun from this clean revision. This evidence qualifies the network boundary;
`tools/node-container` separately qualifies the explicit combined preparation API.

The Rust migration requires new schema 3 artifact receipts and schema 2 owned-VM
receipts from reviewed source. Historical raw evidence is retained unchanged and
currently fails the mandatory current-evidence gates. No migration capture has
run. Use `network verify DIRECTORY` for one new guest or `network verify-published`
for installed first/repeat evidence, through the same `cargo run` prefix above.
Builder nonce frames and settled raw command receipts bind every execution.
The guest lifecycle uses the shared Rust ISO seed builder and process owner;
family verification additionally binds SCP argv, exact stdin scripts/input hashes,
restoration and independent kernel observations. Guest command output is bounded
to 256 KiB. Install whole output directories under `rust-evidence/first` and `rust-evidence/repeat`, retaining historical `evidence/` bytes. Use `rust-provenance.json` for the new complete inventory. Install with every raw command receipt and
recompute the complete provenance inventory only after review.
