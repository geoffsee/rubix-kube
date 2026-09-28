# Pinned crane nested layer stream qualification

This fixture qualifies `verify_crane_image_layer_digests` with actual crane
v0.21.5 `Save` serialization of synthetic tiny images. It never fetches registry
payloads, imports images, extracts members, or executes image contents. It is not
production release provenance, inner-tar/path safety, encryption detection,
filesystem equivalence, or container runtime compatibility evidence.

The producer is built with pinned Go 1.26.2 and the unchanged complete module
checksums from the archive fixture (including klauspost/compress v1.18.5). It
uses Go gzip and klauspost zstd encoders, then `tarball.LayerFromOpener` with the
corresponding OCI media type. The upstream layer abstraction independently
computes stored digests and DiffIDs and decodes every compressed byte stream;
the producer compares its result to known raw input bytes before `crane.Save`.
Each image orders layers A/B/A. Crane emits two stored layer members, colon-named
config, and manifest last. Four image profiles exercise gzip and zstd, each with
one frame/member and with two concatenated frames/members. Layer A expands past
32 KiB; B contains deterministic SHA-256 blocks to cross outer read boundaries.

The Rust fixture oracle independently reconstructs the complete raw tar bytes, including
headers, padding and end markers. It checks the producer's decoded-byte sidecar
against those bytes, recomputes DiffIDs and member/config/manifest identities,
and checks ordered repeated references. It additionally decodes gzip with a bounded
flate2 path, independently of the production layer verifier. Zstd's independent
decoder remains the pinned upstream Go implementation. Raw logs contain the actual outer
archive bytes and upstream sidecar, allowing strict offline verification against
the source-bound producer. This is editable local build/run consistency, not
cryptographic authenticity against replacement of every record and receipt.

Twelve cases run in one explicitly ignored Rust consumer test:

- Four complete gzip/zstd single/concatenated stream successes.
- Rehashed inner gzip CRC and zstd checksum corruption, plus each codec's truncation.
- Rehashed wrong declared DiffID and late outer gzip CRC failure.
- Per-layer decoded and ordered-reference decoded limits.

Mutations recompute stored member names, config identities when changed, manifest
references, outer tar checksums and outer gzip framing. They reach nested checks
instead of failing an obsolete stored digest. The Rust consumer checks exact
successful shared-budget charges (outer bytes, unique decoded layer bytes and
reserved probes), retained charges after failures and retries, and a late zstd
checksum failure's conservative offered-capacity reservation. Failed buffers are
not claimed as observed bytes. These runtime assertions complement the production
private regression that demonstrates zstd writing bytes with an unchanged error
output position. Exact positive and negative observations are compared to the
independent oracle. No observation can pass for a missing, extra or swapped case.

The build compiles the ignored `layer_fixture` consumer executable and Rust helper.
A fresh nonce checksum frame binds producer, consumer and helper SHA-256 values from the
bounded plain build log to both runtime executions, retaining compilation cache.
The verifier binds workspace manifests/lock/toolchain, relevant Rust files,
complete fixture module graph, harness/helper hashes and copied-source inventory.
Capture rejects dirty relevant source before output creation. Receipts preserve
the actual source revision and never relabel prior qualification.

Two newly named linux/arm64 containers use network none, UID 65532, read-only
rootfs, no capabilities, no-new-privileges, init, 64 PIDs, 256 MiB memory, two CPUs,
and a 16 MiB nosuid/nodev temporary filesystem. There are no host bind mounts or
Docker socket mounts. Build time/output bounds are 1800 seconds/16 MiB; each
runtime is bounded to 100 seconds/2 MiB. Producer and consumer each have a
20-second deadline; consumer output is incrementally capped at 64 KiB. Fixture
input reads are capped at 1 MiB. These trusted fixture controls do not promise
production hard CPU/memory or mid-call cancellation guarantees.

Both runs must match archive bytes and observations exactly, with a final process
namespace inventory containing only init, shell and inventory helper. Cleanup
removes only the unique owned image and containers, records failures and checks
for leftovers. Image inspection and all five cleanup commands retain raw logs and
settled receipts. Verification requires exact argv, image identity, empty final
inventories, EOF, process-group absence, and no timeout, cancellation or overflow.
Unknown settlement stops further commands and retains owned context. Settled
cancellation permits owned cleanup with a fresh latch but cannot qualify success.
Builds may download pinned dependencies; runtime is offline.

```sh
# Before evidence: source/verifier mutation checks use explicitly invented records.
cargo test -p rubix-dev --bin rubix-asset-fixture --locked -- --skip published_
cargo test -p rubix-dev --bin rubix-asset-fixture --release --locked -- --skip published_
# Only after independent source review and a clean frozen commit:
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- layer capture /tmp/rubix-layer-capture
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- layer verify /tmp/rubix-layer-capture
# After publishing the entire verified capture directory under rust-evidence/:
cargo test -p rubix-dev --bin rubix-asset-fixture --locked
```

The mandatory `published_layer_requires_current_rust_capture` test requires a
current schema-3 capture at `rust-evidence/`; absent or stale evidence fails.
All 20 files are required: source inventory, capture receipt, and raw logs plus
settled command receipts for build, two runs, image inspection and five cleanup
commands. Preserve historical `evidence/` byte-for-byte. Invented mutation records
and historical raw parser tests never substitute for actual current producer execution.

Historical qualification passed all twelve cases twice at clean source
`bfb31a1914e0d8a365e5b32ab320d4131f5d8d12`. Both runs match the actual emitted
archives and independent observations. The producer SHA-256 is
`949122075fe13d50ee1588de9cd6bb6fa7233626b08c84eea89c1d3825b1e070`;
the Rust consumer SHA-256 is
`e80bf3b9ba88bee3379e1b8c27774b964ad8c51972b5af74f997bed9926387a3`.
Both match fresh builder records and each runtime observation. Capture and cleanup
error arrays were empty. Independent Docker queries confirmed both owned containers
and their image tag absent. These historical receipts do not qualify the new Rust
capture implementation; publish a fresh verified capture from reviewed source.

The first attempted capture at `b4aef28704bdd0f713ac05e16da9bdc096f950ad`
failed on a fixture encoded-probe accounting assertion. Its raw record remains
historical at `/tmp/rubix-layer-qualified-20260928-r1`; it is not passing
qualification. The historical passing capture also corrected the wrong-DiffID mutation
to preserve repeated-reference consistency and preserves bounded consumer failure
logs. Local diagnostic replays were not substituted for the two Docker runs.

Primary pinned sources:

- [crane Save and MultiSave](https://github.com/google/go-containerregistry/blob/v0.21.5/pkg/crane/pull.go)
- [tarball writer and repeated-member deduplication](https://github.com/google/go-containerregistry/blob/v0.21.5/pkg/v1/tarball/write.go)
- [tarball layer digest and upstream decoding](https://github.com/google/go-containerregistry/blob/v0.21.5/pkg/v1/tarball/layer.go)
- [upstream zstd decoder adapter](https://github.com/google/go-containerregistry/blob/v0.21.5/internal/zstd/zstd.go)
- [Go gzip multistream behavior](https://github.com/golang/go/blob/go1.26.2/src/compress/gzip/gunzip.go)
- [OCI whole uncompressed stream DiffID definition](https://github.com/opencontainers/image-spec/blob/v1.1.1/config.md#layer-diffid)

The initial API intentionally supports only gzip and zstd, excluding identity,
skippable frames and dictionary-bearing streams; it does not support foreign
layer sources. Those limitations and production pin qualification remain explicit.
