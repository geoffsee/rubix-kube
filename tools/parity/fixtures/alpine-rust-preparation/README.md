# Actual Rust preparation in an owned Alpine guest

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
python3 tools/parity/fixtures/alpine-rust-preparation/capture.py \
  --allow-privileged-vm --image-cache /tmp/rubix-vm-image-cache \
  --input-cache /tmp/rubix-alpine-input-cache \
  --artifact-directory /tmp/rubix-output-corrected-prerequisite-20260927-r1 \
  --output /tmp/alpine-rust-fresh
python3 tools/parity/fixtures/alpine-rust-preparation/verify.py
python3 -m unittest discover -s tools/parity/fixtures/alpine-rust-preparation
python3 -O -m unittest discover -s tools/parity/fixtures/alpine-rust-preparation
```

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

Both stored guest captures were refreshed for corrected bounded process-output error precedence and
bind source revision `aad310ae06150b68cab4bbc3f9f9c5191a3084d7`. The verified candidate
build receipt binds `5f4064d987ba5dd4f334c3d6ac0bd18c71d3e0f1`, including the updated
supervisor implementation and workspace inputs. Its SHA-256 is
`1989a32474bcbc1e6064e0c7093903da769d240021d66b51aa1fa31ac47515fa`
and its size is 5388504 bytes. Earlier temporary receipts remain historical; their
revisions and observations were not relabeled.
