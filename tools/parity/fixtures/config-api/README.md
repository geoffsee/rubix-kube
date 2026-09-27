# Configuration persistence and API lifecycle oracles

This bounded E02.02 slice executes the pinned Go baseline's actual `config.Write`
and `configapi.Service` in a disposable, unprivileged container. It covers filesystem
replacement, backups, 21 real HTTP request scenarios over a Unix socket, and owned
socket startup/shutdown behavior. There is no live Kubernetes API, host service,
installer or whole-node lifecycle execution.

`file_capture_test.go` and `api_capture_test.go` are the only additions to a fresh
KubeSolo source copy at `2ef1c4787989f11f868f81bb84ae2afd4a49a81d`. The API harness
uses baseline test helpers to construct real services and Unix HTTP clients. No
implementation functions are patched. Every configuration and socket path is in
a test-owned container temporary directory; all secrets are fixed synthetic strings.

```sh
python3 tools/parity/fixtures/config-api/capture.py --output /tmp/unique-config-api-capture
python3 tools/parity/fixtures/config-api/verify.py /tmp/unique-config-api-capture
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/parity/fixtures/config-api -p 'test_*.py'
```

The Dockerfile pins the same Go 1.26.5 builder digest and reference archive SHA256
as the earlier parity captures (`-mod=readonly`, baseline go.sum). This is distinct
from the accepted Go 1.26.8 default-extraction tool pin. Build has a 30 minute
deadline. Test execution has a 90 second Go timeout and a 110 second Docker client
timeout, network disabled, all capabilities dropped, no-new-privileges, UID65532,
read-only root, private 64MiB tmpfs, 512MiB memory and 128 PID limits. No host bind
mounts are used. Cleanup attempts each owned container/image removal and inventory
independently; failed inspection produces `null`, an error and a failed result,
not a false empty inventory. Receipt publication is attempted even after failures.
Ordinary Docker build cache remains reusable. The driver runs these trusted pinned
test binaries and is not a general sandbox for arbitrary submitted artifacts.

`config.json` retains actual first/replacement YAML and observations: previous
bytes are backed up, a backup-path directory causes write failure without changing
the original, no staging files remain, and target mode is 0600 under umask0277.
Fresh ordinary backups are 0600. Two explicitly characterized baseline weaknesses
remain implementation decisions: restrictive umask0277 yields a 0400 backup, and
an existing 0644 backup stays 0644 when overwritten with configuration containing a
secret. Baseline `backup` uses `os.WriteFile` without chmod. E03.03 (#41) should
harden backup permissions as an explicit compatibility deviation; these observations
are not permission policy recommendations. Failure preservation is demonstrated for
this concrete pre-rename error, not power-loss durability or every filesystem fault.

`configapi.json` retains status, parsed body, raw HTTP body and ETag per scenario.
Independent `verify.py` checks complete expected configuration objects, source-derived
defaults, redaction, unredacted ETag hashing, stored-versus-environment-effective state,
PATCH merge/null, PUT replacement, DELETE defaults, restart-required paths, no-op
writes, malformed/type/content/body-limit errors, immutable/invalid edits, rejected
redacted credentials, stale If-Match, validation without writes, disjoint concurrent
patches, health and the exact 30-setting schema inventory. Raw bodies retain Go's
JSON member order to independently hash the actual unredacted document; no ETag
helper is called by the verifier. Only synthetic credentials appear in explicit
showSecrets responses.

Socket observations include 0600 permissions, refusal of a live listener, removal
on clean shutdown, removal of a real stale socket inode, and preservation/refusal
of a regular file. These exercise actual baseline socket functions. HTTP responses
and filesystem observations are deterministic; no response values are normalized
away. Raw evidence logs preserve timestamps and temporary paths; fixture JSON omits
those diagnostics. `provenance.json` pins source/harness/output hashes and the capture
receipt records the exact image and cleanup inventories.

Mutation tests reject original-file corruption, accepted stale ETags, leaked secrets,
lost concurrent updates and omitted socket cleanup. A daemon-failure mock verifies
failure receipt publication. Future Rust implementations should satisfy both full
public-fixture comparison and independent semantic checks; no Rust parity is claimed.

Remaining E02.02 gaps include service-account credentials, additional generated
component configurations/kubeconfigs/manifests, installer/reset behavior, interrupted
writes and power loss, full cluster startup/readiness/shutdown, failed-component
supervision and restart/workload preservation across the supported platform/runtime
matrix. Those require their own disposable execution and evidence; this slice does
not close #36 or qualify full node lifecycle.
