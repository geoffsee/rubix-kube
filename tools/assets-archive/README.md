# Pinned crane archive serialization fixture

This fixture qualifies `DecodeSession::inspect_crane_image_archive` against actual
`crane.Save` serialization of two **synthetic, tiny** images. It does not fetch a
registry image, establish baseline release payload provenance, execute an image,
extract members, or import into a runtime. Existing historical runs are described under migration status below.

The producer imports `github.com/google/go-containerregistry` **v0.21.5**, whose
module proxy origin identifies commit `5b80281da727dae218e1697ab8529b631b9efa64`.
`go.mod`, `go.sum`, and `modules.json` pin all 54 selected modules with zip and
module-file checksums. Docker verifies the downloaded module cache, unchanged
module files, and exact selected graph before admitting the producer into the
final image. Runtime build metadata must identify Go **1.26.2**, the exact crane
version and module checksum, without a replacement module. No workspace Rust
manifest or dependency changes are involved.

Primary sources:

- [Pinned release Go version](https://github.com/google/go-containerregistry/blob/v0.21.5/.go-version)
- [Default pull format and writer selection](https://github.com/google/go-containerregistry/blob/v0.21.5/cmd/crane/cmd/pull.go)
- [Save delegates to MultiSave and tarball.MultiWriteToFile](https://github.com/google/go-containerregistry/blob/v0.21.5/pkg/crane/pull.go)
- [Tar writer config naming, deduplication, layer bytes and manifest order](https://github.com/google/go-containerregistry/blob/v0.21.5/pkg/v1/tarball/write.go)
- [LayerFromOpener compression](https://github.com/google/go-containerregistry/blob/v0.21.5/pkg/v1/tarball/layer.go)

The pinned Go builder image is
`golang:1.26.2-bookworm@sha256:47ce5636e9936b2c5cbf708925578ef386b4f8872aec74a67bd13a627d242b19`.
The Dockerfile also pins Rust images by digest. Runtime qualification
is Linux arm64 only; the image configuration fields include amd64 and ARM and do
not cause execution or emulation of either image.

## Cases and independent oracle

The producer makes valid small tar/gzip layers A and B, then asks crane to save
references A, B, A for each image. The independent Rust oracle verifies four
archive members: one `sha256:<config>` member, two unique `<hash>.tar.gz` members,
and `manifest.json` last. It checks actual stored hashes, sizes, config platform,
ordered references, tag metadata, gzip completion, and tar structure. For these
known synthetic layers only, it also independently checks the expected inner
file content and its hash against declared DiffIDs.

The real Rust consumer runs one explicitly ignored integration test with eight
cases in exact order. Its two successes cover amd64 declared match and ARM with
missing variant unresolved. Six rehashed inputs must fail for corrupt outer CRC,
wrong stored layer digest, unsafe member name, missing referenced member,
duplicate config JSON key, and internally rehashed wrong platform. The consumer
rebinds each encoded input into a temporary declared inventory and asserts budget
charges survive failures. All expected observations are computed independently
of the Rust consumer. Every emitted encoded archive is included as bounded base64
in the raw log so the verifier can recompute the oracle and exact mutations.

The product observation still treats DiffIDs as **declared, unverified metadata**;
the fixture's known-content check does not extend that contract to arbitrary
nested layer codecs or tar contents. Docker archive manifest JSON is not the
original registry manifest. No registry manifest digest, publisher authenticity,
role authenticity, ABI compatibility, import permission, or deployment readiness
is established.


## Rust harness migration status

The Rust harness is under review; fresh qualification has not run yet. Existing
`evidence/` files remain the unchanged historical capture from source
`fb24f95f20d1152fc5921c4f0377add1051f42cb`. Their original source hashes and Python
harness identities are preserved. They are not current Rust-harness evidence and
schema-3 verification intentionally rejects them. Do not relabel these receipts.

The maintenance binary is `rubix-asset-fixture` in `tools/dev`. Its independent
oracles do not call the production asset parser. JSON rejects duplicate keys and
non-integer numbers; input reads reject symlinks and nonregular files and impose
byte limits. Exact test names, record order, typed values, commands, source
inventory, raw log hashes and cleanup outcomes are verified.

The default Rust test suite includes three mandatory published-evidence checks;
these remain failing until reviewed, clean-source Rust captures are published.
Synthetic mutation tests cannot satisfy those checks.

```sh
cargo test -p rubix-dev --bin rubix-asset-fixture --locked
cargo test -p rubix-dev --bin rubix-asset-fixture --release --locked
```

Capture commands below are for use after source and process-ownership review.
Every capture requires a new output directory and a clean relevant source tree.
Publication changes only the exact raw receipts/logs and factual documentation.

```sh
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- archive capture /tmp/rubix-archive-UNIQUE
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- archive verify /tmp/rubix-archive-UNIQUE
cargo run -p rubix-dev --bin rubix-asset-fixture --locked -- archive verify tools/assets-archive/rust-evidence
```

The build binds all three executable hashes (`producer`, `archive-tests`,
`fixture`) in one fresh nonce-delimited checksum frame. Both runtime hashes must
match that frame. Source inventory, pinned full Go graph, exact commands, image
ID, raw hashes and independently recomputed observations are receipt-bound.
Two runs must have identical input bytes and observations. This proves local
build/run consistency, not authenticity against an author who can replace all
receipts and logs.

Builds can fetch pinned dependencies. Runtime containers use UID65532, no
network, read-only root, no capabilities, no-new-privileges, init, 64 PIDs,
256 MiB memory, two CPUs and a 16 MiB tmpfs. No host mounts are used. Build
limits are 1800 seconds/16 MiB and runtime limits are 100 seconds/2 MiB. Producer
and consumer each have a 20-second/64 KiB bound; archive input and decoded tar
are capped at 1 MiB. These fixture limits make no production hard-RSS/CPU claim.

Namespace completion requires only init, shell and the Rust helper. Failed
builds/runs/source rechecks/cleanup remain failed receipts. Only owned container
names and image are removed; independently query their absence after capture.

Publish the build and runtime `*.command.json` records with their logs. Verification
requires exact arguments and hashes, exit zero, EOF, process-group absence and no
timeout, cancellation or overflow. Unconfirmed settlement stops further actions
and retains input contexts; resource inventories remain unknown. Settled
cancellation permits only owned cleanup with a fresh cancellation latch.

Publish fresh Rust captures under `rust-evidence/`. Keep historical `evidence/`
and its provenance unchanged. The mandatory current-capture gate reads only
`rust-evidence/`; source inventories exclude both historical and current outputs.
