# Live integration evidence consumption

Run the official API-server/Kine fixture in a new disposable container, validate its
fresh output, then point the published-binding Rust tests at that same capture:

```sh
python3 tools/api-json/run.py --output "$RUNNER_TEMP/api-json-capture"
python3 tools/integration/check_api_capture.py "$RUNNER_TEMP/api-json-capture"
RUBIX_API_JSON_CAPTURE_DIR="$RUNNER_TEMP/api-json-capture" \
  cargo test -p rubix-upstream-codegen --test api_json --locked
```

The capture directory must be new for each execution; do not cache successful evidence
in place of a live run. Outside CI, replace `$RUNNER_TEMP/api-json-capture` with an explicit
new temporary path. Follow `tools/api-json/README.md` for the isolated Docker boundary,
preparation/network requirements and existing platform limitations. This helper starts
no processes or services and changes no fixture, capture, cache or expectation files.

`check_api_capture.py CAPTURE_DIR` requires `fixtures.json`, `result.json` and
`runner-result.json`. It checks independent semantic assertions and strict, complete,
type-sensitive equality against the reviewed frozen fixture, whose content digest must
match its committed provenance. It also checks successful raw result and runner status,
exact current harness/source hashes in both receipts, component pins, raw HTTP parsing,
raw-to-normalized resource/watch consistency, API authentication/RBAC rejection, datastore
TLS rejection reasons and clean component shutdown. Missing, duplicate-key, malformed or
nonfinite JSON fails. Each input JSON is bounded to 8MiB. Checks use explicit exceptions
and remain active under `python -O`. The CLI returns nonzero on failure and does not print
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
perform the Python provenance checks, so live CI must execute the helper first.

Local verification used the real corrected r7 capture at
`/tmp/rubix-api-json-20260927-r7`: the helper passed and all eight Rust tests passed using
that capture. All eight also passed with the frozen fallback. Twelve Python regressions
cover current source hashes, frozen provenance, full-document differences beyond selected
semantic anchors, raw boolean/integer mismatches, raw watch drift, failed shutdown/TLS,
missing data and optimized-mode failure. An explicitly selected empty capture directory
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
