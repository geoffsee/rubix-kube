# Live integration evidence consumption

Rust maintenance binaries replace the former script entry points. Retained captures remain
historical: current-source validation requires new schema-2 Rust receipts, both source
inventories, raw command receipts and confirmed cleanup. API and boundary qualification
tests intentionally fail until fresh `evidence-rust` captures are published.

Run the official API-server/Kine fixture in a new disposable container, validate its
fresh output, then point the published-binding Rust tests at that same capture:

```sh
cargo run --locked -p rubix-dev --bin rubix-api-json -- capture --output "$RUNNER_TEMP/api-json-capture"
cargo run --locked -p rubix-dev --bin rubix-api-json -- check-capture "$RUNNER_TEMP/api-json-capture"
RUBIX_API_JSON_CAPTURE_DIR="$RUNNER_TEMP/api-json-capture" \
  cargo test -p rubix-upstream-codegen --test suite api_json:: --locked
```

The capture directory must be new for each execution; do not cache successful evidence
in place of a live run. Outside CI, replace `$RUNNER_TEMP/api-json-capture` with an explicit
new temporary path. Follow `tools/api-json/README.md` for the isolated Docker boundary,
preparation/network requirements and existing platform limitations. This helper starts
no processes or services and changes no fixture, capture, cache or expectation files.

`rubix-api-json check-capture CAPTURE_DIR` requires `fixtures.json`, `result.json` and
`runner-result.json`. It checks independent semantic assertions and strict, complete,
type-sensitive equality against the reviewed frozen fixture, whose content digest must
match its committed provenance. It also checks successful raw result and runner status,
exact current harness/source hashes in both receipts, component pins, raw HTTP parsing,
raw-to-normalized resource/watch consistency, API authentication/RBAC rejection, datastore
TLS rejection reasons and clean component shutdown. Missing, duplicate-key, malformed or
nonfinite JSON fails. Each input JSON is bounded to 8MiB. Checks use explicit Rust errors
and remain active in release builds. The CLI returns nonzero on failure and does not print
raw responses, TLS diagnostics or credential-like input bytes.

Runner exit zero with no errors is the current runner's evidence that Docker cleanup and
owned-resource inventories succeeded; zero component exit codes, no escalation and absent
process groups are separately required. These are trusted harness records, not signed
attestations or protection against a malicious host rewriting a capture. CI must preserve
ownership and immutability between capture validation and Rust consumption. The helper
checks existing evidence; it does not independently re-query Docker or recapture a cluster.

`RUBIX_API_JSON_CAPTURE_DIR` is optional for the Rust consumer. When absent, all eight tests
continue using the committed frozen fixture and raw capture. When present, both JSON inputs
come from that directory. A missing or malformed explicitly selected input fails with its
path; it never silently falls back to historical evidence. This selection does not itself
perform the complete capture provenance checks, so live CI must execute the helper first.

Local verification used the real corrected r7 capture at
`/tmp/rubix-api-json-20260927-r7`: the helper passed and all eight Rust tests passed using
that capture. All eight also passed with the frozen fallback. The historical twelve verifier regressions
cover current source hashes, frozen provenance, full-document differences beyond selected
semantic anchors, raw boolean/integer mismatches, raw watch drift, failed shutdown/TLS,
missing data and optimized-mode failure. Rust mutation and CLI tests retain these checks and add raw command-log, process-settlement and cancellation bindings. An explicitly selected empty capture directory
was also rejected by the Rust test with a clear input-path error.

This gate retains the published binding's documented limits: typed initial CRD null
conditions become omitted, quantities remain strings, typed built-ins discard unknown
fields and dynamic JSON preserves arbitrary custom payloads. It is bounded serialization
and protocol-boundary evidence, not universal API parity or cluster conformance.

## Disposable CI jobs

`.github/workflows/integration.yml` keeps live execution separate from the six established
required checks. It runs on relevant pull-request and main changes, weekly, and by explicit
workflow dispatch. Each job receives a fresh GitHub-hosted `ubuntu-24.04-arm` VM; see the
[official runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
No existing machine, cluster, host device or host mount is a test fixture.

The defaults job verifies the locked official source/Go archive lengths and SHA-256 hashes,
then invokes the actual default extractors twice per variant. Each result must match the
reviewed expected fixture. It runs the generated component dispatch and API constructor
variant; later host-derived option completion is separate evidence. The API job starts the
real isolated API/Kine pair, validates its fresh output and feeds it to Rust as described above.
A cold cache follows the same verification path. The defaults archive cache and Rust cache
save only after successful main-push execution; captures themselves are never cached.

Both jobs upload their public capture directories even on failure, retain artifacts for 14 days,
and use read-only repository permissions and commit-pinned actions. Generated keys and SQLite
state remain inside the removed API container; the upload paths contain only public JSON,
inspection metadata and component logs. The default extractor exports no key material.
Network is needed for preparation/tool installation; component execution disables container
networking. The Docker build cache remains runner-owned and disposable.

Hosted Linux arm64 execution must pass before this workflow is considered integration verified.
These jobs do not qualify other distribution targets, managed/external runtimes or a full node.
Privileged VM lifecycle checks remain explicit local/manual disposable-environment work; the
Darwin/HVF VM adapter is not silently replaced with a Linux host test. The existing required
Format, Clippy, debug/release Tests, Dependencies and Security protections are unchanged.

## Candidate-bound qualification receipt schema

Candidate-bound qualification receipts establish tamper-evident, candidate-bound evidence
for release qualification criteria (1–11). Qualification verification evaluates candidate
receipts using `tools/dev/src/release_qualification/receipt.rs` and `criteria.rs`.

### Receipt file location and resolution

Receipts are formatted as JSON files named `criterion-<NN>-<slug>.json` (e.g.
`criterion-01-epic-ledgers.json`). Path resolution follows these precedence rules:
1. Per-criterion override via environment variable `RUBIX_RECEIPT_PATH_<N>` (e.g. `RUBIX_RECEIPT_PATH_1`).
2. Directory override via environment variable `RUBIX_RECEIPTS_DIR`, appending `criterion-<NN>-<slug>.json`.
3. Default repository location: `docs/release/receipts/criterion-<NN>-<slug>.json` (`DEFAULT_RECEIPTS_DIR`).

### Receipt structure

Each receipt contains a top-level `CandidateReceipt` structure serialized as JSON:

```json
{
  "schema_version": 1,
  "criterion": 1,
  "description": "Sample valid qualification run",
  "candidate": {
    "source_revision": "2ef1c4787989f11f868f81bb84ae2afd4a49a81d",
    "binary_digests": {
      "rubix-kube": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "payload_digests": {
      "bundle.manifest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    }
  },
  "environment": {
    "host": "linux-arm64",
    "kernel": "6.6.137",
    "runner": "github-hosted-ubuntu-24.04-arm"
  },
  "commands": [
    {
      "command": [
        "rubix-kube",
        "--version"
      ],
      "exit_code": 0,
      "duration_ms": 15
    }
  ],
  "assertions": [
    {
      "name": "startup_verified",
      "passed": true,
      "detail": "clean startup confirmed"
    }
  ],
  "skips": [
    {
      "name": "musl_dynamic",
      "reason": "glibc host platform"
    }
  ],
  "cleanup": {
    "cleaned_paths": [
      "/tmp/test"
    ],
    "remaining_containers": [],
    "remaining_images": [],
    "status": "complete"
  },
  "timestamps": {
    "started_at": "2026-10-08T12:00:00Z",
    "completed_at": "2026-10-08T12:01:00Z"
  },
  "integrity_hash": "e53e9fe4d16e18734a4f399e7b110f9a164242332a91278254e10e0a58bb5983"
}
```

### Identity and integrity binding

1. **Candidate Identity Binding**: The `candidate` block binds the qualification evidence to authoritative
   candidate metadata loaded from `docs/release/cell-inventory.json` (or `RUBIX_CANDIDATE_INVENTORY_PATH`)
   and `docs/release/SHA256SUMS`. The reader validates:
   - `source_revision` matches the candidate git commit hex.
   - Every declared entry in `binary_digests` matches the authoritative SHA-256 digest in `CandidateInventory`.
   - Every declared entry in `payload_digests` matches the authoritative SHA-256 digest in `CandidateInventory`.
   Unknown artifact keys or mismatched hashes cause validation to fail closed.
2. **Payload Integrity Binding**: `integrity_hash` must equal the SHA-256 digest of the canonical
   JSON-serialized `ReceiptPayload` (the unsigned receipt fields). Serialization uses `serde_json::to_vec`
   in field declaration order (`schema_version`, `criterion`, `description`, `candidate`, `environment`,
   `commands`, `assertions`, `skips`, `cleanup`, `timestamps`), with `BTreeMap` maps (`binary_digests`,
   `payload_digests`) serializing keys in sorted order. If any field is modified, integrity verification fails closed.
3. **Command Verification**: All commands in `commands` must have executed with `exit_code: 0`. Optional
   `stdout_sha256` and `stderr_sha256` digests, if present, must be valid lowercase SHA-256 hex strings.
4. **Assertion Verification**: All assertions in `assertions` must have `passed: true`. An optional `detail`
   string may provide context or diagnostics.
5. **Skips Verification**: Any skipped step in `skips` must have a non-empty, justified `reason`.
6. **Cleanup Verification**: `cleanup.status` must be `"complete"`, and both `remaining_containers` and
   `remaining_images` must be empty arrays (`[]`), confirming no leaked resources on the host.

### Trusted reader constraints

The reader enforces fail-closed parsing and resource limits:
- **Maximum Receipt Size**: Bounded to 8 MiB (`MAX_RECEIPT_BYTES = 8 * 1024 * 1024`). Files exceeding
  this size are rejected immediately.
- **Symlink Protection**: Symlinks are rejected on open via bounded reading; receipts must be regular files.
- **Strict JSON Parsing**: Duplicate JSON keys, unknown fields (`#[serde(deny_unknown_fields)]`), and
  nonfinite numbers (`NaN`, `Infinity`) are rejected.
- **Schema Version**: `schema_version` must equal 1.
