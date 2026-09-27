# Official source semantic drift gate

This E02.04 slice compares official Kubernetes CRI and containerd descriptors and OpenAPI definitions
with a reviewed source-derived inventory. It never reads generated Rust as its oracle.
`inventory.json.gz` contains deterministic, sorted JSON compressed with gzip mtime zero;
all original definition keywords, nested properties, required fields, enums, defaults,
extensions and descriptions are retained. Description edits therefore also require review.

```sh
# Explicit preparation; checks never fetch or build tools implicitly.
python3 tools/upstream/upstream.py fetch
python3 tools/drift/drift.py check > /tmp/rubix-source-diff.json
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/drift -p 'test_*.py' -v
# Inspect the accepted inventory without a special viewer.
gzip -dc tools/drift/inventory.json.gz | python3 -m json.tool
# Only during a reviewed upstream adoption, regenerate the acceptance baseline.
python3 tools/drift/drift.py extract
```

`--cache-dir` selects a prepared upstream cache. Sources, the protoc archive, compiler
and imports are reverified against `tools/upstream/inputs.json` before execution. CRI and
OpenAPI records must also agree with `docs/architecture/upstream-inputs.json`. Their exact
official revision, URLs, lengths and hashes are included in the inventory. No new host
packages, services, containers, root privileges or network access are needed for checking.
`--input-manifest` and `--architecture-manifest` explicitly select alternative reviewed
contracts during adoption; defaults are the repository's current contracts. Both must agree
on each source. The containerd graph includes 17 official API files, 11 services and 65 RPCs,
plus the compiler's five locked well-known imports. `--include_imports` retains their descriptor
semantics too. Protoc resolves imports exclusively from this verified temporary graph; missing
imports fail instead of consulting host include directories. The actual STAT=0 content action
and bidirectional Write stream have independent descriptor assertions.

The checksum-locked protoc parser produces a descriptor set directly from the official
proto. A bounded standard-library protobuf wire decoder translates descriptor names,
field numbers, types, labels, defaults, nested messages, enum values and complete RPC
request/response/streaming signatures. Descriptor options and unknown fields are retained
as bytes/wire values, so new syntax cannot silently disappear from comparisons.
Unknown varints retain their integer meaning, not noncanonical byte encodings; fixed-width
and length-delimited unknown values retain exact bytes. JSON rejects duplicate keys and
non-JSON numeric constants such as NaN and Infinity.
Malformed/truncated values, unsupported wire types and excessive recursion fail closed.
Repeated values remain arrays (including scalar descriptor names), preserving ordering.
This uses protoc's parser, independently of prost/tonic translation into Rust; it does
not independently implement or prove protoc correctness or runtime interoperability.

`check` is read-only and exits 1 for any difference, printing reviewable JSON Pointer
paths with additions, removals and before/after values. `extract` explicitly refreshes
the inventory atomically. Review the diff before accepting an input or inventory update;
updating both automatically would defeat the compatibility decision. Run this source
gate alongside `check-cri`: freshness and semantic compatibility are separate checks.
Hand-encoded descriptor tests and known real CRI signatures check the decoder. The
prepared-parser regression freshly parses changed field numbers/types and RPC names/
streaming and verifies drift despite fresh parser outputs. That integration test reports
an explicit skip if the compiler cache was not prepared; CI must prepare it first.

Scope is official CRI/containerd plus all OpenAPI definitions. API operations outside definitions,
independently executed Go default extraction and
feature-gate tables are remaining work. Schema defaults are retained but are not a
substitute for executable Go defaulting behavior. This gate does not close E02.04 or
establish live protocol, cluster, conformance or Rust parity.
