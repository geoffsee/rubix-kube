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

Historical schema1 evidence in this branch was captured on
`ec6f19e6c32ef5cdb5276d6b5794ccfd9bdfd12f`, with14 decoder cases per run.
It intentionally cannot satisfy the expanded schema2 verifier and remains
historical; the current receipts below replace it without relabeling that capture.

Fresh schema2 receipts must also bind the build log and both builder-produced
binary digests. Older receipts without those fields require recapture and cannot
be relabelled as qualification of this combined harness.

Historical schema2 qualification was captured twice on decoder error-budget source
`7be6768673263c26cf6fc3407b6650b239e41cf8`. Both Linux arm64 release runs passed
all 14 decoder and 10 compressed ELF integration tests. Builder and both runtime
observations agree on decoder SHA-256
`0a27bdd86dd617e5a971482a073020f78951f4ab00ded4dab212e28321b6d2a8`
and compressed ELF test SHA-256
`d34ebfb20488e19aced666b2eab1147957aeaf5ea56e4a6cd879ebed5c53c2ac`.
Capture and cleanup error arrays are empty; independent Docker inventory checks
confirmed both owned containers and the image tag absent. All 13 evidence tests
and current-source verification pass normally and under optimization. This fixture
runs the two integration executables; the private failed-read error-accounting
regression passed separately as a library test, not in these Docker runs.

Current nested-layer integration qualification was captured twice at source
`bfb31a1914e0d8a365e5b32ab320d4131f5d8d12`. Each Linux arm64 run passed all
14 decoder and 10 compressed-ELF integration tests. Fresh builder records and both
runtime observations match decoder SHA-256
`71643138b63beb75903665b62c2aba5b9631555bea1ce71a3d0e2d4ece4a0799`
and compressed-ELF SHA-256
`e0baaddca78061be29eb058ed7c1561a4dece60ff5d5deb82f3bb7ce479024eb`.
All 13 evidence tests and current-source verification pass normally and with `-O`.
Capture and cleanup error arrays are empty; owned containers and image tag were
independently confirmed absent. This fixture still exercises only its two named
integration executables; nested-layer compatibility has separate evidence.
