# Explicit identity ELF qualification

`qualify.py` downloads only the four exact API/Kine URLs and SHA-256 values already
pinned in `experiments/component-boundary/inputs.json`, into an explicit temporary
cache. Each file is capped at256MiB, each network read has a30second timeout, and
elapsed download time is checked against300seconds between reads. A final blocked
read can extend that bound by its timeout. Existing cache content must already
match; symlink entries are rejected. Temporary download files are removed on all
normal exceptions and publication never overwrites an existing cache entry.

The trusted Rust test is run twice in release mode (900second/8MiB output bounds
per run, using the source-bound existing defaults capture helper). Artifacts are
read only; no upstream executable is launched. The helper owns spawned trusted
Cargo process cleanup; this is not an arbitrary executable sandbox. SHA-256 is
checked before and after inspection. Python `struct` parses independent ELF64
header, interpreter and dynamic dependencies; all four resulting records must
match Rust. The fixture checks arm64 and amd64 only, not all deployment targets.

```sh
python3 tools/assets-elf/qualify.py --cache /tmp/rubix-elf-cache --output /tmp/rubix-elf-capture
python3 tools/assets-elf/verify.py /tmp/rubix-elf-capture
python3 -m unittest discover -s tools/assets-elf -p 'test_*.py'
```

Final capture must follow source freeze/review. Publish receipt and two bounded raw
logs only; binaries remain in the explicit temporary cache. The receipt binds exact
current compiled sources/manifests/lock, harness, pins, independent observations,
raw logs and honest revision/working-source snapshot semantics. It proves the
implemented observations of those locked bytes, not execution, ABI closure,
cryptographic publisher authenticity or install safety. Full E06.01 remains open.
