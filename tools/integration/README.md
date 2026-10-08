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

### Receipt structure

Receipts are formatted as JSON files named `criterion-<NN>-<slug>.json` (e.g.
`criterion-01-epic-ledgers.json`) located under `target/release-qualification/` or an explicit
qualification receipt directory. Each receipt contains the top-level envelope:

```json
{
  "schema_version": 1,
  "criterion": 1,
  "payload": {
    "criterion": 1,
    "candidate": {
      "commit": "0123456789abcdef0123456789abcdef01234567",
      "version": "v1.31.1",
      "target": "x86_64-unknown-linux-gnu",
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "environment": {
      "runner": "github-actions",
      "os": "ubuntu-24.04",
      "arch": "x86_64",
      "kernel": "6.8.0-1014-azure"
    },
    "commands": [
      {
        "name": "run-qualification-suite",
        "command": "cargo test --locked",
        "exit_code": 0,
        "stdout_sha256": "...",
        "stderr_sha256": "..."
      }
    ],
    "assertions": [
      {
        "name": "epic-ledgers-complete",
        "passed": true,
        "details": "All epic ledger entries verified"
      }
    ],
    "skips": [
      {
        "name": "optional-hardware-acceleration",
        "reason": "Hardware acceleration is not available on virtualized runner"
      }
    ],
    "cleanup": {
      "confirmed": true,
      "remaining_containers": 0,
      "remaining_images": 0,
      "details": "Docker pruning removed all temporary resources"
    },
    "timestamps": {
      "started_at": "2026-10-08T12:00:00Z",
      "completed_at": "2026-10-08T12:05:00Z"
    }
  },
  "payload_sha256": "<64-hex SHA-256 digest of canonicalized payload>"
}
```

### Identity and integrity binding

1. **Candidate Identity Binding**: The candidate block must match the cell candidate registered
   in `docs/release/cell-inventory.json` (or `SHA256SUMS`). The reader validates that `commit`,
   `version`, and `target` align with the cell inventory and that the artifact hash matches.
2. **Payload Integrity Binding**: `payload_sha256` must equal the canonical SHA-256 digest of the
   contained payload object, computed via deterministic sorted-key serialization. If any field
   inside `payload` is tampered with, integrity verification fails closed.
3. **Command Verification**: All listed commands must have completed with `exit_code: 0`.
4. **Assertion Verification**: All assertions must have `passed: true`.
5. **Skips Verification**: Any skipped step must have a non-empty, justified `reason`.
6. **Cleanup Verification**: `cleanup.confirmed` must be `true`, and both `remaining_containers`
   and `remaining_images` must be strictly `0`.

### Trusted reader constraints

The reader enforces fail-closed parsing and resource limits:
- **Maximum Receipt Size**: Bounded to 8 MiB (`MAX_RECEIPT_BYTES = 8 * 1024 * 1024`). Files exceeding
  this size are rejected immediately.
- **Symlink Protection**: Symlinks are rejected on open; receipts must be regular files.
- **Strict JSON Parsing**: Duplicate JSON keys, unknown fields, and nonfinite numbers (`NaN`, `Infinity`)
  are rejected.
- **Schema Version**: `schema_version` must equal 1.
