# Official API-server JSON serialization fixtures

The runner, transport orchestration, normalization and independent verifier now live in
`tools/dev/src/api_json` and `tools/dev/src/component_boundary`. The existing evidence,
fixture and provenance bytes remain historical records. Current qualification requires a
new actual Rust capture under `evidence-rust`; the mandatory test fails until it exists.
Source-frozen recapture has not yet been performed for this migration.

Rust owns child-process groups, request state and assertions. OpenSSL supplies certificate
operations; curl performs verified TLS HTTP; SQLite CLI executes fixed read-only queries.
No response parser or lifecycle code calls an interpreter. Bounded command receipts bind
raw logs, exact Docker arguments, successful settlement and owned-resource removal.
Cancellation stays latched through cleanup and publication. Unconfirmed process ownership
retains temporary inputs and stops further capture or cleanup commands.

This bounded E02 serialization gate runs official Kubernetes v1.35.7 and Kine v0.16.3
in a new Docker container. Requests and semantic expectations are independent of
`k8s-openapi` code generation. No existing cluster, kubelet, scheduler, container runtime
or workload participates. It characterizes API admission/storage/JSON behavior, not
whole-cluster or conformance behavior.

```sh
cargo run --locked -p rubix-dev --bin rubix-api-json -- capture --output /tmp/new-api-json-capture
cargo test --locked -p rubix-dev --lib api_json
```

Preparation uses checksum-locked official component binaries and a pinned Rust image
shared with the Rust qualification tools. OpenSSL, curl and SQLite installation use
Debian's live package repository; image identity is retained rather than claiming a fully
reproducible image build. Build is bounded to 900 seconds, execution to 600 seconds, HTTP
requests to 10 seconds, readiness to 120 seconds, and cleanup operations to 30 seconds.
The unprivileged UID65532 container has no network, host mounts or published ports, all
capabilities dropped, no-new-privileges, 1GiB memory, 2CPUs and 256 PID limits. Containers,
images and child process groups have unique ownership and bounded cleanup. Docker build
cache remains managed by Docker. Credentials and SQLite state remain inside the removed
container; evidence exports no private keys or service-account tokens.

API clients verify the serving certificate and use client-certificate authentication;
unauthenticated and authenticated-but-unauthorized requests must fail. The datastore uses
a dedicated CA and mutual TLS on loopback. Both an absent certificate and the Kubernetes
administrative certificate signed by the separate API CA must fail datastore TLS. The API
server itself demonstrates successful datastore authentication through readiness and CRUD.
These are direct handshake negatives, not outage recovery qualification.

The capture explicitly creates a Namespace and default ServiceAccount because no controller
runs. Pod token automount is explicitly false to avoid unrelated random admission volume
names and token projections. Fixtures cover:

- Pod quantity canonicalization (`0.5` CPU becomes `500m`, `1.5Gi` becomes `1536Mi`),
  resource limits, labels/annotations, explicit null removal and omitted fields.
- Service named/string and numeric/integer `IntOrString` target ports.
- A real namespaced CRD preserving arbitrary nested JSON, including null, boolean,
  integer, decimal, arrays, and user-owned nested metadata.
- Actual ConfigMap ADDED/MODIFIED/DELETED watch envelopes and the final deleted value.
  Watching starts from a recorded list resourceVersion; three history events are read,
  then the connection closes. The server deadline is 10 seconds and client deadline 15.

Raw HTTP responses, request objects, statuses and watch lines are retained in
`evidence/result.json` before normalization. Requests use compact serde_json encoding from source-reviewed JSON values.
The only normalized paths are each observed object's top-level `metadata.uid`,
`metadata.resourceVersion`, `metadata.creationTimestamp`, and `metadata.managedFields`.
No recursive key removal occurs: CRD user data named `metadata.uid` remains unchanged.
All other defaulted and serialized values remain in `fixtures.json` for future Rust
round trips. Removing volatile metadata establishes a portable serialization fixture;
it does not establish those metadata fields' lifecycle semantics.

The independent verifier checks semantic invariants as well as create/read equality.
Mutation controls change both create/read observations together, so equality alone cannot
hide altered quantities, lost nulls, numeric/string port confusion or boolean/integer
confusion. Watch mutations must also fail. Frozen evidence and source hashes protect the
record, and raw observations must regenerate the committed fixtures.

Future Rust consumers must round-trip these complete API objects using the selected
published bindings and explicitly qualify any representation loss. CRD arbitrary JSON
and watch event envelopes need appropriate generic wrappers. These fixtures do not claim
unimplemented Rust parity, generated-tree reproduction, all Kubernetes resource coverage,
credential lifecycle, power-loss durability or product platform qualification.

The retained Darwin arm64 Docker runs r6/r7 produced byte-identical normalized fixtures
from fresh datastores. `evidence/result.json` is the final raw capture;
`repeat-capture.tar.gz` retains the earlier independent run. `provenance.json` records
component source revisions, commands, complete file digests and independent post-run
Docker absence checks. Eleven local regressions pass. Both final API/Kine process pairs
exited zero without escalation. This executes Linux arm64 binaries on Docker Desktop;
Linux amd64 inputs are pinned but not executed by this record.

`preparation-failures.tar.gz` retains initial truthful failures: the build allowlist
omitted new harness files, then Pod admission rejected a missing default ServiceAccount.
The fixture now explicitly creates that account. A preparatory successful run exposed
random projected-volume names; selecting token automount false avoids that unrelated
admission behavior while preserving the four documented metadata normalization rules.

Rust verification uses explicit Result checks and remains active in release builds.
A CLI regression rejects matching captures with a boolean changed to integer without echoing supplied data.
The retained live repeats predate the Rust verifier and remain historical evidence. See [Rust consumer evidence](CONSUMER.md).
