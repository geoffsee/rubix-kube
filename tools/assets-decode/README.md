# Native Linux decoder qualification

This fixture builds only `cargo test -p rubix-assets --release --locked --test decode
--test decoded_elf --no-run` in the exact pinned Rust image. A second pinned Python image contains
two copied test executables. Two newly named linux/arm64 containers run all14
independent decoder tests and all10 decoded-executable ELF tests with network disabled, UID65532, read-only root,
capabilities dropped, no-new-privileges,2CPUs,256MiB memory and64PIDs. No production
artifact is executed; these tests include small fixed gzip/zstd vectors and native
zstd library code. A Python process-namespace inventory must contain only init,
the driver shell and that helper after tests exit. Both runs bind the same two
binary SHA256 values and the exact14+10 source-reviewed test names in separate
ordered suite sections; skipped/failed/missing or swapped cases cannot pass.
Receipt schema2 records named binary hashes and case sets, retaining the original
native decoder checks. A rehashed substitution of either repeated binary fails.

The capture uses the existing bounded trusted-command helper (source hashed), with
1800second/16MiB build and100second/1MiB per-run bounds. Metadata/helper inputs are
resolved before output creation. The driver always attempts independent owned
container/image removal, records cleanup failures and writes a receipt. This is
not an arbitrary untrusted-executable sandbox; no host directories or Docker
socket are mounted into test containers. No host software/settings are changed.

The current verifier binds all manifests/lock/toolchain, .cargo, relevant asset and
platform sources/fixtures, native dependency graph through Cargo.lock, exact build
recipe/helper and runtime namespace helper, complete copied-source inventory,
run commands, raw logs and binary hashes. The builder prints the SHA-256 of the
copied `/out/decode-tests` and `/out/decoded_elf-tests` executables. Builds use `--progress=plain` and pass the
unique owned image tag as `QUALIFICATION_NONCE`. That build argument is declared
and consumed only in a separate checksum step after compilation/copying, forcing
fresh checksum output while preserving compilation cache reuse. The receipt
binds the bounded build log's SHA-256 and exactly one builder-produced digest
per suite; each runtime digest in both runs must equal its suite's builder digest. This is build/run consistency,
not cryptographic authenticity of editable local evidence. JSON duplicate/nonintegral values and
oversized/nonregular evidence are rejected. Evidence revision and dirty-state
metadata describe the actual snapshot; later commits never relabel receipts.

```sh
python3 tools/assets-decode/capture.py --output /tmp/rubix-assets-decode-linux
python3 tools/assets-decode/verify.py /tmp/rubix-assets-decode-linux
python3 -m unittest discover -s tools/assets-decode -p 'test_*.py'
```

After reviewed source freeze and commit, publish build.log, first.log, repeat.log,
source-inventory.json and receipt.json only. Stored evidence tests are mandatory;
missing evidence fails rather than silently skipping. This qualifies Linuxarm64
native decoder execution for the tested inputs, not production payloads, other
ABI/CPU/libc combinations, hard RSS/CPU limits or archive/OCI semantics. The new
suite exercises composed ELF header observations on synthetic compressed vectors;
it does not establish executable runtime compatibility or installation safety.

Stored schema2 evidence was captured on
`bc614a3373d97f740b7f2a496157fb85f841967e`. Both Linuxarm64 release runs
passed all14 decoder tests and all10 decoded ELF tests. Both builder-produced
executable hashes match both runtime observations:

- `decode`: `88fe00df05956e8a3d49f58afc837cd76e708d6e53dadd47a881470cd32c1e6e`
- `decoded_elf`: `725c21f5d537893626793e3ce799004e2a1b4ebe9717d3d7b78b8c5b2dffc656`

The receipt binds the nonce-controlled build output and build log hash, and reports
no capture/cleanup errors or remaining owned containers/images. The owned names
and image tag were independently confirmed absent. All13 evidence regressions and
current-source verification pass normally and under optimization. Older receipts
without builder/log bindings are historical only. Production-payload and other
ABI/runtime qualification remains open.
