# Published Rust binding consumer evidence

`tools/upstream/tests/api_json.rs` consumes the independent official API-server capture
with `k8s-openapi 0.28.0`, explicit `v1_35`, serde 1.0.228 and serde_json 1.0.151.
Eight focused tests passed on Rust 1.97.1, along with warnings-denied Clippy and formatting.
No server recapture or generated binding edits were needed for these tests.

```sh
cargo test -p rubix-upstream-codegen --test suite api_json:: --locked
cargo clippy -p rubix-upstream-codegen --test suite --locked -- -D warnings
cargo fmt-check
```

Complete JSON values survive typed deserialize/serialize for the captured Namespace,
established CustomResourceDefinition, Pod and Service create/read documents, and
ConfigMap ADDED/MODIFIED/DELETED watch envelopes. Namespace and CRD tests use the raw
HTTP responses, retaining server metadata rather than only the normalized fixture.
Assertions additionally check independent captured semantics: canonical CPU/memory
quantities, labels and annotations, absent optional fields, token automount false,
headless Service identity, named/string versus numeric/integer target ports, the
CRD preserve-unknown schema, and the final value in the deleted watch object.

The initial CRD CREATE response demonstrates a concrete representation difference:
`status.conditions` is explicitly `null` in the server response, but the typed Rust
CRD serializes it as omitted. A dedicated regression preserves that finding and
checks that no other captured value changes. The independent raw fixture is unchanged.
`RawExtension` preserves the complete original document when exact presence matters.
This difference is not silently normalized or presented as exact typed parity.

Explicit null and omitted optional Pod fields both decode to `None` and serialize
as omitted. `Quantity` preserves its string; it does not implement the server's
Go quantity canonicalization or validation. Thus the captured server `500m` survives,
while locally decoding `"0.5"` still serializes `"0.5"`.

Arbitrary custom resources use `RawExtension` for the whole dynamic object and the
published `JSON` wrapper for the arbitrary spec subtree. Both preserve the captured
null, boolean, integer, decimal, arrays and nested user metadata. Typed built-in
objects drop unknown fields, which a separate regression explicitly characterizes.
Unknown watch event types are rejected. Consumers needing forwarding or unknown-field
retention must select a dynamic JSON path rather than assume typed structs preserve
all server fields.

This establishes a bounded published-binding serialization gate and exposes a
specific null/presence limitation. It does not prove universal resource parity,
quantity computation, arbitrary watch variants, resource lifecycle, mutation policy,
cluster conformance or platform qualification. No production adapter or policy
waiver is introduced by these tests.
