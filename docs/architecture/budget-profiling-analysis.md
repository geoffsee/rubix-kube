# Performance budget profiling and scope decisions

Issue #122 / E29.02, Gate C14 remains **unqualified**. The profiler compares
supplied reference and candidate report fields; committed reports are synthetic
fixtures. No live hardware capture importer, causal optimization receipt, real
archive parity check, or current-source cluster qualification is implemented.

The authoritative limits remain in the [compatibility contract](compatibility-contract.md)
and [acceptance matrix](acceptance-matrix.md). Fixture comparisons exercise
latency, median idle memory, distribution, density, growth and shutdown gates;
a fixture PASS describes arithmetic only. Declared hardware metadata does not
establish that a run occurred there.

## Aggregation bases

The amd64 fixture's retained-process PSS entries sum to **484 MiB**; its separate
system-pod entry is **32 MiB**. Adding those entries yields **516 MiB**. The
**450 MiB** E01 input is a separate median of summed PSS samples, not a matched
breakdown of those role entries. The generated process percentages use the
retained-role sum and exclude system pods. These fixture values must not be
presented as shares of a matched whole-node capture.

The nine listed binary entries sum to **492 MiB**, while the fixture's independent
extracted-executable total is **245 MiB**. The profiler preserves that historical
fixture total for its contract comparison, but computes binary shares against
the **492 MiB listed sum**. They are inconsistent accounting inputs, not verified
artifact measurements. Comparing the listed sum to the fixture reference total
of 268 MiB gives **1.836x**, exceeding the 1.10x limit; the 245/268 fixture aggregate
comparison gives 0.914x. Neither establishes real distribution compliance.

## Comparison scope and required evidence

Four comparisons summarize node-daemon PSS, node executable size, compressed
archive bytes and boot-to-API p95 from the supplied fields. Their differences
are not implemented optimization claims. The reports cannot establish zstd level,
compression causality, authenticated startup sequencing, dedicated-CA mTLS,
configuration parity, identical upstream bytes, image completeness or real archive
decompression. Verified parity lists therefore remain empty. Independent retained
protocol, lifecycle and artifact receipts are required before claiming any of these.

The five scope decisions preserve the requirements to reject unmeasured sub-200MB
and under-60-second claims, retain upstream defaults under D09/KS-68, account for
images separately, and leave secondary-target gaps explicit. Input observations
are examples, not evidence that a compliant node can or cannot meet a marketing
claim. Cold-start comparisons use only the analyzed architecture and actual
reference input, without inventing another architecture or a reference number.

## Verification

```sh
cargo run --locked -p rubix-dev --bin rubix-perf -- profile \
  --reference tools/perf/fixtures/amd64-reference-go.json \
  --candidate tools/perf/fixtures/amd64-candidate-rust.json
cargo test --locked -p rubix-dev --test suite budget_profiling::
```

The profile command prints an unqualified input comparison and exits nonzero.
Unit tests check report arithmetic and failure behavior; they do not qualify
Linux runtime performance or platform support.
