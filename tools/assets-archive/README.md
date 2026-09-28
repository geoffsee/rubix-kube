# Pinned crane archive serialization fixture

This fixture qualifies `DecodeSession::inspect_crane_image_archive` against actual
`crane.Save` serialization of two **synthetic, tiny** images. It does not fetch a
registry image, establish baseline release payload provenance, execute an image,
extract members, or import into a runtime. Two successful runs are recorded below;
the mandatory `PublishedEvidence` test verifies their published raw proof.

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
The Dockerfile also pins Rust and Python images by digest. Runtime qualification
is Linux arm64 only; the image configuration fields include amd64 and ARM and do
not cause execution or emulation of either image.

## Cases and independent oracle

The producer makes valid small tar/gzip layers A and B, then asks crane to save
references A, B, A for each image. The independent Python oracle verifies four
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

## Capture and verification

After the harness and source are reviewed and committed, use a new output path:

```sh
python3 tools/assets-archive/capture.py --output /tmp/rubix-archive-UNIQUE
python3 tools/assets-archive/verify.py /tmp/rubix-archive-UNIQUE
python3 -O tools/assets-archive/verify.py /tmp/rubix-archive-UNIQUE
python3 -m unittest discover -s tools/assets-archive -p 'test_*.py'
python3 -O -m unittest discover -s tools/assets-archive -p 'test_*.py'
```

The driver rejects dirty relevant source before creating output. It copies a
source snapshot, records every copied file, and rechecks that snapshot after two
runs. Current verification binds the workspace manifests/lockfile/toolchain,
Cargo configuration, assets/platform Rust and fixture inputs, this complete
harness including Go graph pins, and the capture/namespace helpers. Publication
README and evidence files are excluded from current-input comparison; copied
README files must still remain unchanged during capture. The receipt records the
actual commit, full copied inventory hash, image ID, build log hash, commands,
per-run raw log hashes and independently recomputed observations.

A fresh per-capture checksum build step binds both producer and Rust consumer
bytes to both runtime invocations; compilation cache is retained. Two runs must
have identical archive bytes/hashes and observations. This is local
build/run-consistency evidence, not an authenticity guarantee against someone
who can rewrite all raw records and receipts.

Builds may fetch pinned dependencies and images. Runtime containers have no
network, use user 65532, read-only rootfs, all capabilities dropped,
no-new-privileges, Docker init, 64 PIDs, 256 MiB memory, two CPUs and a 16 MiB
`nosuid,nodev` temporary filesystem. No host bind mounts are used. The host driver
bounds build time/output to 1800 seconds/16 MiB and each runtime to 100 seconds/
2 MiB; producer and consumer also have 20-second subprocess deadlines. Consumer output
is drained incrementally into a file capped at 64 KiB by the source-bound helper. The tiny
fixture oracle caps decoded and encoded inputs at 1 MiB. These are fixture
controls, not production archive cancellation or hard-memory guarantees.

After each run the namespace inventory must contain only init, the shell and the
inventory helper. The driver's `finally` cleanup removes only its unique owned
containers/image and records failures plus remaining-resource queries. A failed
build, run, source recheck or cleanup cannot produce passing evidence. After
capture, independently inspect Docker for that exact owned tag before release.

The `Evidence` Python mutation class uses explicitly invented records to test verifier rejection;
they never substitute for a real capture. The separate `verify.py` command and `PublishedEvidence` class are mandatory
published-evidence gates. Run the complete test and verification commands above,
including the published-evidence gate. Investigate any failure or missing evidence;
the invented mutation records cannot replace the actual capture.

Actual pinned-crane writer qualification passed twice on source
`90a352c1b720b4a99a235f5bc3ce5d1c6533603f`, including the decoder error-budget
fix. Each run passed all eight archive cases. The actual producer serialized two
synthetic tiny images; archive bytes and independent observations matched across
runs. This is serialization compatibility evidence, not registry payload or
baseline release provenance. Declared DiffIDs remain unverified by the archive API.

Builder and both runtime hashes match producer SHA-256
`7434ad637e4fb31b7c89fb4db001eb5678530ebe8c00780a9f4d9ef5b0cb9b9b`
and Rust consumer SHA-256
`a5df32ef460ec2f5c9f48a9e59dded4d4a959335b1d4e51c2428357d659a8189`.
Capture and cleanup errors are empty; independent Docker queries confirmed both
owned containers and their image tag absent. All 14 Python evidence tests and
current-source verification pass normally and under optimization.
