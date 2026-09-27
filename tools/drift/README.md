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

## Combined upstream adoption report

`report.py` compares explicit before/after snapshots of the independent source inventory
and executed Go defaults. It never runs a generator, fetches inputs, starts components,
changes accepted fixtures, or treats newly generated Rust as its oracle. Each directory
must contain all three files:

| Snapshot file | Producer |
| --- | --- |
| `inventory.json.gz` | `drift.py extract`, schema version 2 (CRI, containerd and OpenAPI definitions) |
| `defaults.json` | `tools/defaults/capture.py` base `run0.json`, schema version 1 |
| `apiserver-defaults.json` | Same capture's `apiserver0.json`, schema version 1 |

For a local review, first preserve the accepted snapshot in a new directory:

```sh
mkdir /tmp/adoption-before /tmp/adoption-after
cp tools/drift/inventory.json.gz /tmp/adoption-before/inventory.json.gz
cp tools/defaults/expected.json /tmp/adoption-before/defaults.json
cp tools/defaults/apiserver.expected.json /tmp/adoption-before/apiserver-defaults.json
```

In the candidate worktree, prepare its proposed checksum-locked inputs explicitly with
`python3 tools/upstream/upstream.py fetch --cache-dir target/upstream`. Then create a
candidate source inventory at the new destination; this does not replace the accepted one:

```sh
python3 tools/drift/drift.py extract --cache-dir target/upstream \
  --inventory /tmp/adoption-after/inventory.json.gz
cp /tmp/candidate-default-capture/run0.json /tmp/adoption-after/defaults.json
cp /tmp/candidate-default-capture/apiserver0.json /tmp/adoption-after/apiserver-defaults.json
python3 tools/drift/report.py --before /tmp/adoption-before --after /tmp/adoption-after \
  --format json > /tmp/adoption-report.json
python3 tools/drift/report.py --before /tmp/adoption-before --after /tmp/adoption-after \
  --format markdown > /tmp/adoption-report.md
```

The candidate defaults must come from a separately executed, source-pinned capture using
`tools/defaults/capture.py`; see its README for explicit Go/source archive preparation.
The reporter itself needs only Python and these local snapshots, with no compiler or
prepared cache. Use distinct snapshot directories and review each producer's receipts;
source pins here are reported claims from those inputs, not independent attestation that
a newly supplied snapshot was produced by its claimed source. Comparing candidates does
not authorize updating the accepted baseline or waive changed security/runtime behavior.

Exit codes are **0** for no semantic changes, **1** for a reviewable change report, and
**2** for missing, malformed or unsupported snapshots. A changed report is expected during
an adoption review; shell automation should distinguish exit 1 from invalid input. JSON
and Markdown output are deterministic and include both source/toolchain pins, snapshot
SHA-256 digests, categorized changes and explicit removal counts. Categories cover API
schema fields/keywords, protobuf fields and RPC signatures for both protocols, component
defaults from both import graphs, API-server options, feature-gate histories/effective
state, and remaining snapshot metadata. Unknown metadata changes are retained.

Named protobuf files/messages/fields/services/methods align by their source names, so a
removed field does not shift every later field's comparison. Reordering those named
collections is semantically ignored; duplicate names and malformed named collections
fail. Other arrays retain index order, including feature histories and schema arrays.
Missing and explicit null values differ, and boolean/integer or other type changes are
reported. Empty named collections preserve their presence separately from absent ones.
Whole required snapshot domains missing from an input are invalid, not an unchanged report.
Each file and expanded gzip inventory is limited to 32MiB; duplicate JSON keys, nonfinite
constants and malformed gzip fail closed. The report covers the producers' documented
scope and cannot establish runtime compatibility from a clean diff.

### Include completed API-server options

Constructor flags and actual `Complete()` results remain separate report categories.
For an adoption review, include both resolved captures explicitly:

```sh
python3 tools/drift/report.py --before /tmp/before --after /tmp/after \
  --before-resolved /tmp/resolved-before --after-resolved /tmp/resolved-after \
  --format markdown
```

Each resolved directory contains `resolved.json` copied byte-for-byte from capture
`run0.json`, plus its `receipt.json`. The receipt must bind those bytes to both
successful repeated runs, identify the official source and toolchain, and record
successful cleanup. Both directory arguments are required together; missing or
unbound selected evidence fails with exit 2. Omitting both retains the original
constructor/schema report and does not claim completed-option coverage.

Three additional categories show completed values/flags, completion errors, and
resolved source/control metadata. Changed or removed defaults yield exit 1 even
when constructor defaults stay unchanged. Success-to-error transitions appear as
removal from one domain and addition to the other. The standalone comparator is
`tools/resolved-defaults/report.py` with the same `--before`, `--after`, and
`--format` arguments. These commands compare explicit snapshots without fetching,
executing components, accepting a new baseline, or writing evidence.
