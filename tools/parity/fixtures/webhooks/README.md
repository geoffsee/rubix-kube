# Webhook distribution behavior fixtures

This E02.02 source oracle invokes the actual pinned KubeSolo webhook implementation
in an additive package-local test in a disposable source copy. It captures two
registration variants, nineteen actual HTTP-handler request/response cases, and
six explicitly fake-client status-update cases. It does not start an API server,
serve TLS, or qualify real admission/reconciliation.

`createConfiguration` is exercised with LoadBalancer on/off and a missing certificate.
The CA bundle is clearly synthetic copy data. `serveMutate` receives actual httptest
HTTP requests; the complete returned AdmissionReview, base64 JSONPatch, UID, HTTP
status, content type and errors are retained. Request and response presence is not
normalized away. Every Service input names default/svc. The harness inspects all scheduling-lock
entries immediately after each request; dry-run/disabled/no-address cases must leave
none. This point-in-time observation does not establish concurrent shutdown safety.
`updateLoadBalancerStatusWithRetry` receives the upstream Kubernetes fake client:
exact GET/PATCH actions, status subresource, merge patch, counts, successful retries,
and five-attempt exhaustion are recorded. Fake actions establish builder/control
flow behavior, not actual API validation or networking.

The Rust `fixture_oracles::webhook` module constructs the complete expected envelope from reviewed
source behavior, with exact inventories and strict JSON types; captures never
supply their own expected component or case list. Only log timestamps and private
test-directory names remain outside the comparison record; no JSON field is removed.
Two independent runtime containers must emit identical JSON. Synthetic certificates
are not validated as real trust material; no keys or tokens are generated/exported.

Baseline weaknesses remain visible: PVC patches replace all annotations; Job patches
replace the whole nodeSelector; malformed typed objects are allowed without a patch;
the handler itself mutates a direct Pod UPDATE even though registration is CREATE-only;
Content-Type is not checked, and body reads have no application-level size bound.
The fixture does not send oversized bodies. These observations require explicit
compatibility/security decisions by E14, not silent reproduction as security policy.
The registration's Ignore failure policy and NoneOnDryRun side-effect declaration
are preserved exactly. A status patch targets only services/status, excluded from
registration resource matches.

Run from the repository root:

```sh
cargo run --locked -p rubix-dev --bin rubix-fixture -- capture webhooks --output /tmp/unique-webhooks
cargo run --locked -p rubix-dev --bin rubix-fixture -- verify webhooks /tmp/unique-webhooks
cargo run --locked -p rubix-dev --bin rubix-fixture -- verify-evidence webhooks /tmp/unique-webhooks
cargo test --locked -p rubix-dev fixture_oracles
```

Build uses the SHA256-locked baseline archive, digest-pinned Go 1.26.5 image,
unchanged go.mod replacements/go.sum with -mod=readonly, and external_deps. Source,
harness, helper and public evidence identities are recorded. The shared defaults
lifecycle helper is Rust: 30-minute/8MiB build bounds, 110-second/1MiB
runtime client bounds and independent guarded cleanup/inventory receipt publication.
Each trusted test process has a 90-second Go deadline; runtime is UID65532, network
none, read-only, no capabilities or privilege escalation, 512MiB, 2CPUs, 128PIDs,
and a 64MiB private tmpfs. No host mounts or external credentials. Only owned named
containers/images are removed; Docker build cache remains reusable.

Historical evidence is retained unchanged. Current Rust captures live in
`rust-evidence/`; their mandatory gate binds tooling, baseline source identities,
complete raw artifacts, exact public records and owned process cleanup.

Fixture maintenance belongs to #36. Consumers are E14.01 #73 (transport/registration),
E14.02 #74 (placement), E14.03 #75 (LoadBalancer status), and E11.03 #65 (live API
integration). Actual TLS/admission reconciliation, create-or-update registration
against an API, concurrent status events, shutdown/race qualification, full workload
and node lifecycle, and Rust parity remain separate gates. No full #36 closure claim.
