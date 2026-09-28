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

Committed qualification passed twice on `f2080912285b5857ed59ba3b9f8f57c9b0e26d4d`. All seven evidence regressions and strict verification pass normally and under optimization. The four pinned files have the expected arm64/amd64 machine IDs and ET_EXEC headers; no interpreter or DT_NEEDED entries were observed. Loader/ABI compatibility remains unresolved.

Dependency-name limit correction was qualified against the four pinned artifacts twice on source `ebedb171a6d538d1eca2a7172116875e9199ef5e`. The observations remain read-only and do not establish runtime ABI compatibility.

The corrected oracle was also qualified against all four pins twice on integrated
decoder source `436a871e77da04ea6ebeeff8dfb64a94c3c66d1b`. Current-source
verification and all seven evidence regressions pass normally and under optimization.
The raw upstream executables were read only and never executed.

Corrected oracle field/limit checks were also qualified against all four pins twice
on decoded ELF composition source `a45e635357eafb086fde62aaa0ed5d43ff217f59`.
Current-source verification and all seven evidence regressions pass normally and
under optimization. Upstream artifacts were read only and never executed.

Current archive-integration qualification read all four pinned artifacts twice on
source `bc614a3373d97f740b7f2a496157fb85f841967e`. The source-bound current
verifier and seven regression tests pass normally and under optimization.
The existing explicit cache was reused after hash checks; no artifact was executed.
