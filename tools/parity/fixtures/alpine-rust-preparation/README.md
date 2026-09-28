# Actual Rust preparation in an owned Alpine guest

The capture and verifier now run in Rust. Historical `evidence/` receipts remain
unchanged. Current schema 2 qualification requires fresh `evidence-rust/first` and
`evidence-rust/repeat`, plus a current Rust prerequisite artifact-build receipt.
Mandatory tests fail until those independently checked captures exist.

This sibling fixture preserves the frozen Go baseline evidence. It runs the actual
static `rubixctl` candidate through the complete `check` command, including Linux
host discovery, ordered policy, preparation, reobservation and port probes. It uses
the same pinned Alpine image, local signed APK repository and isolated QEMU/HVF
boundary. The frozen baseline adapter supplies cleanup/cache helpers; their exact
source hash and the executed artifact-build verifier/support hashes are bound.

Before boot, the candidate binary's exact hash/size/target/build revision and
complete current relevant compiled-source inventory must agree with the original
Linux build receipt. The build verifier also checks its synthetic qualification
logs and cleanup. Those original build records are copied unchanged into guest
evidence; their explicit working-tree qualification flag is preserved. No binary
is silently rebuilt or accepted based only on its filename.

Each fresh guest exercises:

1. Opt-out refusal, with package, service registration and controller state unchanged.
2. Real APK failure against an empty guest-local repository, then repository restoration.
3. Real OpenRC registration failure with the guest cgroups script temporarily absent.
   Package installation has already succeeded; the exact eight-package partial
   effect is retained and reported, with no rollback claim.
4. An explicitly injected no-effect service script. Actual `rc-update` and
   `rc-service` binaries run, but the test script returns success without mounting
   cgroups. The CLI must return failure after reobserving missing controllers.
   The original script hash comes from the pinned official OpenRC APK; its exact
   bytes are restored and the injected service state is reset before continuing.
5. Genuine OpenRC cgroup setup with the original script, then an idempotent repeat.
6. An actual guest reboot followed by a successful opt-out check, unchanged
   installed package inventory and persistent controllers/service registration.

The no-effect case is a labeled test double, not evidence that a genuine OpenRC
service mounted successfully. Genuine mount/reboot cases remain separate. The
baseline ignored rc-update failure; Rust deliberately stops and reports the
preparation error. Rust also rechecks all prerequisites and never turns a zero
command exit into readiness without observing the resulting host state.

Actual package additions and final controllers are compared with the independently
frozen Go guest observations. No host software, service, cgroup, networking or
existing VM is modified. Fresh private overlays, vars, keys and seed are deleted
after owned QEMU exit; ambiguous post-reap group identity is never signaled. Secrets
are excluded from durable evidence. Output/time/resource limits are inherited from
the reviewed Alpine boundary, with a180-second guest script limit and40-second
individual CLI limit. The production command's own deadlines remain active.

```sh
cargo run --locked -p rubix-dev --bin rubix-platform-fixture -- alpine-rust capture \
  --allow-privileged-vm --image-cache /tmp/rubix-vm-image-cache \
  --input-cache /tmp/rubix-alpine-input-cache \
  --artifact-directory /tmp/current-verified-prerequisite \
  --baseline-directory /tmp/current-verified-alpine-first \
  --output /tmp/alpine-rust-fresh
cargo run --locked -p rubix-dev --bin rubix-platform-fixture -- alpine-rust verify \
  --directory /tmp/alpine-rust-fresh --baseline-directory /tmp/current-verified-alpine-first
cargo test --locked -p rubix-dev --bin rubix-platform-fixture
cargo test --locked -p rubix-dev --bin rubix-platform-fixture --release
```

The prerequisite capture must pass its complete current-source verification and
its artifact revision must match the guest capture revision. Artifact bytes, size,
musl target and the verified Docker build recipe remain pinned. An old build
revision is not a substitute for a fresh qualified build.

The explicit baseline directory is fully verified before VM creation and again
before successful publication. Its exact `result.json` digest is bound in the
Rust guest report. This permits qualification outside the source tree. Read-only
verification accepts the same external input; without the option it uses the
published baseline at `alpine-preparation/evidence-rust/first`. Repeated Rust
captures should use the same verified first baseline.

This is one pinned Alpine/aarch64/musl/OpenRC kernel environment. It does not close
whole E05.02, qualify other distributions/ABIs, establish runtime/API readiness,
reserve checked ports, prove package rollback, or replace the separate disposable
cancellation/owned-command cleanup qualification. The real `rubixctl` executable
requires explicit `--install-prereqs`; the fixture's VM privilege opt-in authorizes
only these newly owned disposable guests.

The input cache is required at argument parsing, before artifact inspection or output
creation. Both initial boot and reboot require completed structured cloud-init JSON;
exit 2 is accepted only with the exact pinned missing fingerprint-helper warning and
no stage errors. The verifier binds each raw JSON response independently to its receipt.
Fresh SSH keys use fixed fixture comments rather than local account/hostname comments.

The previous guest captures qualified bounded asset-decoder integration and
bound source revision `d7bd7d0ac66ca6347e66b12f344e7a31c77f5bfd`. Their candidate
build receipt bound `ec6f19e6c32ef5cdb5276d6b5794ccfd9bdfd12f`, including the updated
decoder dependencies and workspace inputs. Its SHA-256 was
`1989a32474bcbc1e6064e0c7093903da769d240021d66b51aa1fa31ac47515fa`
and its size was 5388504 bytes. Earlier temporary receipts remain historical; their
revisions and observations were not relabeled.

Network integration qualification: two fresh guests on source `5799b93cf85ff5e28f8f0da18472122e0fa424b3` passed all seven prerequisite paths and reboot checks. The executed rubixctl candidate was built on `45e727574ff8b2c8ec0c760a16e4f110a188343a` (SHA256 `1989a32474bcbc1e6064e0c7093903da769d240021d66b51aa1fa31ac47515fa`, 5,388,504 bytes). Both guests powered off cleanly; owned groups and private directories were independently absent. This sibling prerequisite evidence does not substitute for the separate host-network effect fixture.

Container integration refresh: two fresh guests captured source
`0671aa2942e723bdcd2a350e4668b80a814b9a7b` and executed the clean `rubixctl` candidate built on
`fb24f95f20d1152fc5921c4f0377add1051f42cb`. The candidate remains
SHA256 `1989a32474bcbc1e6064e0c7093903da769d240021d66b51aa1fa31ac47515fa`,
5,388,504 bytes. Both passed all seven prerequisite paths and independent reboot
checks, then powered off with QEMU exit 0 and no cleanup errors. Owned process
groups and private directories were independently confirmed absent. Published
receipts retain the distinct build and capture revisions without relabeling
historical runs. Actual container mount/PID/controller preparation is separately
qualified by `tools/node-container`.
