# Prepared upstream inputs and CRI generation

This first E02.03 layer implements explicit preparation and offline CRI generation from the
[accepted input contract](../../docs/architecture/upstream-inputs.md). It does not implement a
container runtime or complete E02.03: direct containerd clients and the remaining published API
provenance checks follow separately. Independent drift/default/behavior oracles belong to E02.04.

Python 3.10+ handles HTTPS, SHA-256, ZIP inspection and atomic cache writes using the standard
library. The small Rust maintenance binary generates code using exact prost/tonic versions in
the workspace lockfile. The node and CRI library do not depend on that maintenance binary.

## Commands

From the repository root:

```sh
# Explicit network preparation: official CRI/schema plus checksum-locked protoc and includes.
python3 tools/upstream/upstream.py fetch
# Build the maintenance binary explicitly; this can download locked Cargo dependencies.
cargo build -p rubix-upstream-codegen --locked
# These commands do not fetch sources, invoke Cargo or install tools.
python3 tools/upstream/upstream.py verify
python3 tools/upstream/upstream.py generate-cri
python3 tools/upstream/upstream.py check-cri
python3 -m unittest discover -s tools/upstream -p 'test_*.py'
cargo test -p rubix-cri --locked
```

The default cache is `target/upstream`. `--cache-dir` chooses another cache.
`--platform` can prepare locked Linux amd64/arm64 or Darwin arm64 compiler artifacts;
generation requires the current host platform. Other hosts fail explicitly until their
compiler artifacts are reviewed and added. `--generator` selects an explicitly built generator;
`--output-dir` selects the generated directory (default `crates/rubix-cri/src/generated`).
An optional `--protoc` binary must match the selected archive's **binary digest and length**;
matching `--version` alone is insufficient. Version is also checked before generation.
The compiler version probe has a 10-second deadline and generation a 120-second deadline.
A timeout fails without replacing existing generated output; temporary partial output is removed.

`generate-cri` writes only `runtime.v1.rs`, atomically, and prints a JSON receipt containing
source, compiler, generator, Cargo lock, manifest and output hashes. `check-cri` regenerates in
a temporary directory, compares bytes and file inventory, and fails without changing committed
output. Save the receipt with validation evidence. Generation uses `BTreeMap` protobuf maps,
clients enabled, servers disabled, and no Cargo rebuild notices. All RPC routes from the source
must appear in the output, including the streaming events method.

## Preparation and ownership

`inputs.json` locks immutable source URLs, exact lengths and SHA-256 values. The official
protoc ZIP and its extracted compiler plus five required well-known imports have separate hashes.
`fetch` downloads only missing blobs, verifies bytes before atomic replacement, checks every ZIP
member for traversal/duplicates/symlinks/special files, and extracts only the locked members.
`verify` and generation independently recheck both the ZIP and its extracted files. Cached
corruption fails clearly; remove the identified corrupt cache entry and rerun `fetch` explicitly.
An earlier valid input remains intact if a later download fails. Partial preparation never counts
as a verified cache.

All internal cache/output paths are fixed or derived from validated hashes. Existing symlinks
below the selected directories are rejected. The caller chooses and owns the cache/output root;
do not share those writable directories with an untrusted concurrent process. This avoids treating
path checks as a defense against an attacker concurrently swapping directory entries.

Ordinary `cargo build` reads committed Rust. No project `build.rs` runs a protobuf compiler,
downloads upstream inputs or starts services. Offline Cargo compilation separately requires
the locked Rust dependencies to be cached/vendored. Offline generation requires prepared inputs,
the already-built maintenance binary and the matching host compiler; missing inputs fail rather
than triggering network access.

## Transitive dependency compatibility

The workspace lock preserves one version of each dependency without weakening cargo-deny's
duplicate-version policy. The selected prost 0.14.4 and tonic 0.14.6 versions remain unchanged.

| Locked transitive dependency | Compatibility reason |
| --- | --- |
| `indexmap 2.11.3` | Uses `hashbrown ^0.15.0`, sharing petgraph's 0.15.5. It satisfies petgraph's `^2.5.0` and h2's `^2`; newer indexmap releases introduce another hashbrown series. |
| `async-trait 0.1.89` | Uses `syn ^2.0.46`, sharing prost/other derive tooling's syn 2; satisfies tonic's `^0.1.13`. Versions 0.1.91/0.1.92 introduce syn 3. |
| `tokio-macros 2.7.1` | Uses `syn ^2.0`, sharing the same parser; satisfies tokio 1.53.1's `~2.7.0`. Version 2.7.2 introduces syn 3. |

These are reviewed lockfile resolutions, not dependency-ban exceptions. Broad dependency refreshes
must recheck manifest compatibility, advisories and `cargo tree --workspace --duplicates`, then run
`cargo deny check`. Once upstream dependencies converge, remove obsolete transitive constraints
through an ordinary reviewed lock update and repeat generation checks.

## Evidence limits

Repeated generation proves deterministic translation with the selected tools. It does not prove
CRI interoperability or Kubernetes behavior. The library wire tests independently hand-encode
the absent/present Linux configuration distinction and unknown enum values: consumers must check
message presence and validate enum values instead of treating a protobuf default accessor as
runtime negotiation. Real managed/external runtime tests remain E09/E10/E13 work.

The committed code's source and generator toolchain are documented in the architecture inventory.
Generated lint exceptions are scoped to that module because upstream field names and mechanical
generator patterns are outside the handwritten Rust style contract. Changes to input revisions,
generator versions or compiler hashes require a reviewed upstream-adoption change and new evidence.

The [Darwin arm64 execution record](evidence/2026-09-27-darwin-arm64.json) records two real
generation runs with identical output, a deliberately stale output rejected without modification,
and a compiler that prints the expected version rejected because its bytes are not trusted.
Focused Rust Clippy, three independent wire tests and 17 Python preparation tests passed locally.
Linux compiler archives are pinned and inspected; execution on Linux remains a separate check.
