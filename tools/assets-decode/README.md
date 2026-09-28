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
run commands, raw logs and binary hashes. JSON duplicate/nonintegral values and
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
`b431b351a7ca841b0e60153a4d574d7aba2f18c0`. Both Linuxarm64 release runs
passed all14 decoder tests and all10 decoded ELF tests. Executable SHA-256 values:

- `decode`: `cadd95f023f0313f32076c167324cfdb4590f94ad29c2862412c2d618309f581`
- `decoded_elf`: `5feed5268d8fbabeff7bd64a8de41c5ff4c44d9d3d3a07f0ce0ed503cb34641f`

The receipt reports no capture/cleanup errors or remaining owned containers/images;
those owned names and image tag were independently checked absent after capture.
All10 evidence regressions and current-source verification pass normally and
under optimization. Other target ABI/runtime and production-artifact gates remain open.
