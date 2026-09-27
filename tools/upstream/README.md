# Prepared upstream inputs and runtime client generation

These E02.03 layers implement explicit preparation, offline CRI/containerd client generation and
published Kubernetes resource-binding provenance from the
[accepted input contract](../../docs/architecture/upstream-inputs.md). They do not implement a
container runtime. Independent drift/default/behavior oracles belong to E02.04; API serialization
and live compatibility qualification remain separate gates.

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
python3 tools/upstream/upstream.py generate-containerd
python3 tools/upstream/upstream.py check-containerd
python3 -m unittest discover -s tools/upstream -p 'test_*.py'
cargo test -p rubix-cri --locked
cargo test -p rubix-containerd-api --locked
```

The default cache is `target/upstream`. `--cache-dir` chooses another cache.
`--platform` can prepare locked Linux amd64/arm64 or Darwin arm64 compiler artifacts;
generation requires the current host platform. Other hosts fail explicitly until their
compiler artifacts are reviewed and added. `--generator` selects an explicitly built generator;
`--output-dir` selects the generated directory (default `crates/rubix-cri/src/generated` for CRI,
`crates/rubix-containerd-api/src/generated` for containerd).
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

`generate-containerd`/`check-containerd` use the same verification and deadlines for thirteen
generated files: eleven service packages, shared containerd types and task types. Every expected
output and method route must be present. All generated files are prepared and validated before
destination writes; each file replacement is atomic. All existing destinations must be regular files before
publication begins. This is not a whole-directory crash transaction: an interruption during
publication can leave a mixture of old and new files, which the next check rejects. Generation refuses unrelated files in the
selected output directory, and check mode never writes to that directory. Receipts include a
per-source and per-output hash map; CRI receipts retain their original single-file fields.

Containerd inputs come from the own-upstream `api/v1.10.0` module at commit
`8b34ce391bd114e080892cccfa956ef3807c207c`, reconciled with server v2.2.5. The ten contracted
services are containers, content, diff, events, images, leases, namespaces, snapshots, tasks and
transfer. Version is included for the runtime's handshake. Its additional schema was separately
fetched at the same API commit and verified byte-identical to the pinned v2.2.5 server file.
The manifest pins all seventeen protocol files: eleven services and six recursive local imports.
The five Google well-known imports come from the locked protoc archive. Preparation reconstructs
the upstream `github.com/containerd/containerd/api/` import paths, rejects duplicate import names,
and checks the complete import closure before invoking the compiler. Compiler implicit include
paths cannot hide a missing prepared import.

Content provides streaming archive-byte upload, images hold descriptors/metadata, leases bound
temporary content retention, and namespaces scope managed-runtime state. These generated RPCs
are building blocks; transaction semantics, namespace metadata, cancellation, archive validation,
image import and cleanup still belong to E09. Generic `Any` transfer payloads do not establish
an implemented high-level transfer workflow. External CRI mode remains independent and must not
adopt the host runtime's namespaces/content.

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
The first-layer record includes focused Rust Clippy, three independent CRI wire tests and seventeen
Python preparation tests. Containerd adds import-closure and method-coverage negatives plus wire
fixtures for commit bytes, unknown actions, field-mask presence and the actual STAT protobuf zero.
Linux compiler archives are pinned and inspected; execution on Linux remains a separate check.

The [containerd Darwin arm64 execution record](evidence/2026-09-27-containerd-darwin-arm64.json)
records two identical generations of thirteen files containing all sixty-five RPC routes from the
eleven selected services. A deliberately stale output and a missing transitive import both fail
without changing committed output. The expanded suite passes twenty-three preparation tests and
four independent containerd wire tests; focused Clippy passes for the generator and bindings.
Existing CRI output remains byte-identical. These checks establish provenance, reproducibility and
bounded wire behavior, not live containerd interoperability or completed image-import workflows.

## Published Kubernetes bindings provenance

The maintenance package consumes exact `k8s-openapi 0.28.0` with `v1_35` as a dev dependency.
No second generated resource tree is introduced. Python 3.11+ is required for this gate and the
complete Python test suite (`tomllib` parses Cargo manifests and lockfiles).

```sh
# Explicit network preparation, separate from the offline gate.
cargo fetch --locked
python3 tools/upstream/upstream.py fetch
python3 tools/upstream/upstream.py check-kubernetes-bindings
```

The check reads the checksum-locked published crate archive in memory, rejects traversal,
duplicate paths, links and special entries, and bounds archive sizes and entry count. It never
extracts archive paths to disk. The published manifest, `v1_35` module and VCS metadata must match
the selected version and source commit. The immutable generator version map must select the
verified official v1.35.6 schema, whose bytes must equal the accepted v1.35.7 schema. This includes
all 735 definitions; patch schema equivalence does not imply patch runtime equivalence.

`cargo metadata --offline --locked` verifies the actual resolved registry package and unified
features. Only explicit `v1_35` is accepted among version selectors; `latest`, `earliest` and other
version features fail. Cargo.lock's package checksum must equal the verified published archive
checksum. This invokes no build or network operation; missing Cargo dependencies must be prepared
explicitly. The JSON receipt includes artifact/schema hashes, selected features and lock digest.
The gate does not need a compiler executable or generator binary, though the shared `fetch`
command prepares other generation inputs too.

The published library applies upstream generator fixups and special handling beyond raw schema
translation. This gate establishes artifact/source/schema provenance, not independent generation
reproduction or semantic parity. Official API-server/Go JSON fixtures and Rust round trips still
need qualification for quantities, metadata, CRD JSON, watch events, presence and unknown fields.
The existing source drift gate remains a separate check. No API-server serialization parity or
live Kubernetes compatibility is claimed by this layer.

The [Darwin arm64 provenance record](evidence/2026-09-27-kubernetes-provenance-darwin-arm64.json)
records a successful offline check and real archive/schema corruption rejected before Cargo runs.
The expanded preparation suite passes 31 tests, including unsafe archive entries, wrong source
revision, feature aliases, duplicate package selection, checksum mismatch and schema drift.

The published binding gate checks the maintenance dependency declaration as well as
its locked resolution: an exact version, disabled defaults and explicit `v1_35` are
required even when a loosened manifest would retain the same current Cargo.lock.

The [fresh-cache execution record](evidence/2026-09-27-clean-refresh-darwin-arm64.json)
records two initially absent preparation directories, verified network fetches, and identical
offline CRI/containerd generation checks against committed output in each. Published Kubernetes
artifact checks also agree. Actual missing and corrupted CRI blobs fail clearly without a hidden
refresh; the original bytes were restored after the negative probes. This complements the
existing stale-output, archive integrity and import-closure regressions.
