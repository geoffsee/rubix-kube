# Official API-server resolved options

This development-only fixture invokes official Kubernetes v1.35.7
`ServerRunOptions.Complete(context.Background())`, after `NewServerRunOptions()`.
It complements the constructor-only API-server flags in `tools/defaults`.
The exact source and Go 1.26.8 archives are locked in `inputs.json`; source files
supporting the independent expectations are hashed in `source-anchors.json`.

The two successful cases cover default service CIDR, explicit IPv4/IPv6 service
CIDRs, service IP selection, advertise address/external host resolution,
authorization and anonymous authentication interaction, watch-cache defaults and
overrides, watch history relative to request timeout, legacy runtime-config alias
resolution, and service-account maximum token expiration. Four error cases cover
malformed and too-small primary CIDRs, invalid watch-cache size, and subhour token
expiration. All flag values are recorded before and after completion; selected
derived values have independently source-reviewed expectations in the Rust verifier.
The entire reviewed output is also compared against hash-bound `expected.json`.

Controls set serving bind and external addresses to `127.0.0.1` and the serving
certificate directory to empty. Advertise address starts unset, so completion
uses the explicit external-address control through `DefaultAdvertiseAddress`.
This avoids host routing and hostname dependence. Serving material is generated
in memory; only its presence and empty file paths are recorded. No certificate,
private key, token, opaque callback, or issuer implementation is serialized.
Only the watch-cache entry set is sorted, because upstream constructs that list
by iterating a map. Other values are retained exactly. Diagnostic timestamps and
random certificate material are outside the JSON record; raw logs are retained.

Unlike component configuration extraction, these API-server options have no
`SetObjectDefaults_*` entry point: the authoritative path here is the constructor
followed by actual `Complete`. This tool does not replace the generated object
default dispatch exercised by `tools/defaults`. It does not call `Validate`,
`ApplyTo`, `Run`, bind a listener, connect to a datastore, or establish cluster
behavior. In particular, successful completion is not proof that these options
would pass startup validation or represent production security policy.

Run explicitly with prepared checksum-locked archives:

```sh
cargo run -p rubix-dev --bin rubix-resolved-defaults --locked -- capture \
  --source-archive /path/to/kubernetes.tar.gz \
  --go-archive /path/to/go1.26.8.linux-arm64.tar.gz \
  --output /tmp/resolved-options-new
cargo run -p rubix-dev --bin rubix-resolved-defaults --locked -- verify /tmp/resolved-options-new/run0.json
cargo test -p rubix-dev resolved_capture --locked
```

Capture builds a scratch image from the official vendored source, using the
checksum-locked toolchain and pinned bootstrap image. Build networking is disabled;
the bootstrap image must already be available or Docker may explicitly pull it.
The build is bounded to 30 minutes, and each of two runs to 90 seconds, 512 MiB,
two CPUs, 64 PIDs, a read-only root, and a 16 MiB disposable tmpfs. Runtime has no
network, host mounts, capabilities, or privilege escalation, and runs as UID
65532. Different hostnames must produce byte-identical records. Shared lifecycle
helpers live in `tools/dev/src/defaults/capture.rs` and the shared Rust process runner,
and are hash-bound in new receipts. Confirmed-settled failures still attempt every owned cleanup.
Uncertain process cleanup stops further commands and retains build context and ownership facts.
JSON receipts and public logs describe extraction and cleanup; they are local
evidence, not a signed attestation of a hostile Docker host.

The historical committed evidence is the final pre-migration `r3` capture: two successful bounded runs,
172 flags before and after completion for each successful case, four expected
completion errors, and empty error/cleanup/inventory lists in the receipt.
Its original normal and optimized regression runs passed. They exercise
resolved-value mutations, missing/error cases, flag transitions, strict JSON and
types, complete frozen comparison, frozen-file hash binding, rejection of wrong
prepared source before Docker runs, and cleanup failure reporting. Initial `r1`
and `r2` successful local captures preceded deterministic set ordering and final
format/runtime metadata changes; the committed receipt binds that original harness, not the migrated Rust driver.
Only Linux/arm64 extraction is evidenced here.

## Disposable CI coverage

The official-defaults job in `.github/workflows/integration.yml` also executes
this completed-option capture twice on a fresh hosted Linux ARM64 runner and
compares the complete output with the reviewed fixture. It reuses prepared
archives, which this capture independently checks against its own pins; ordinary
compilation still performs no upstream extraction. Inputs are cached, results
are not. Failure stops the job, and both public capture directories are uploaded
for 14 days, including failure diagnostics. Generated serving keys remain only
in the isolated process memory. Constructor extraction and completed options
are distinct steps; neither starts a server or proves startup validation.

See `tools/drift/README.md` to include completed values and errors in an adoption
report. Copy `run0.json` to `resolved.json` without changing its bytes and retain
the matching receipt; the comparator checks both repeated-output digests.

## Rust capture qualification

The source-reviewed oracle and full frozen output comparison now execute in Rust. Historical
raw evidence, expected hashes, and provenance are preserved without relabeling. A current-source
`rubix-resolved-defaults verify-evidence` gate reads the separate `rust-evidence/` directory and
requires a verified actual capture from frozen Rust tooling source. Semantic and
adversarial tests remain runnable without Docker; they do not replace actual capture evidence.

New receipts record the complete relevant Rust source inventory and individual bounded command
settlement facts. SIGINT/SIGTERM permanently cancel capture; confirmed-settled cleanup uses a
separate uncancelled runner. Unconfirmed process ownership is retained with build directories,
remaining Docker resources are reported unknown, and no further capture or cleanup command runs.
Only exact owned container names and the unique image tag are removed; a shared cached image ID
is recorded but never used to remove other tags.
