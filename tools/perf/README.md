# Synthetic performance fixtures and arithmetic tooling

C13 / E29.01 remain **not qualified**. The JSON files in `fixtures/` are generated
from constants. No Go or Rust node, dedicated Linux hardware, workload, fresh
boot, idle sampling interval, shutdown experiment or 24-hour soak produced these
values. Machine names, timestamps, PIDs, sizes and timings are fictional examples.
The five idle values illustrate per-run p95 aggregates; the files do not contain
five actual 900-sample captures. `inputs.json` is likewise a synthetic example,
including its digests, not a verified source/workload manifest. `provenance.json`
checks fixture integrity only and does not authenticate measured performance.

The Rust tooling in `tools/dev/src/perf/` validates schema, finite/nonnegative
raw samples, sample counts and cached statistics. Pair comparison rejects
mismatched architectures, hardware, workloads, memory limits, accounting modes
and retained component versions. Canonical process roles distinguish containerd
from its shim. Growth arithmetic derives final/initial memory, requires a
24-hour declared interval and checks observed failures. Markdown reports print
the supplied failure counts and clearly mark every current report unqualified.

This slice has **no verified live-capture importer**. Qualification therefore
fails closed for both `synthetic-fixture` and legacy/unverified inputs, even if
all demonstrated arithmetic budgets are met. Changing an evidence label cannot
qualify a report. Before enabling live acceptance, implement capture ingestion
that retains and verifies raw samples, source/workload hashes, commands, failures,
logs, process receipts, Linux libc/filesystem/component variants and matched
concrete implementation revisions. Follow the
[compatibility contract](../../docs/architecture/compatibility-contract.md) and
[acceptance matrix](../../docs/architecture/acceptance-matrix.md), including paired
amd64/arm64 measurements and the independent pinned Go reference. Successful
unit tests or macOS execution cannot replace that work.

Generate synthetic examples (success means fixture generation only):

```sh
cargo run --locked -p rubix-dev --bin rubix-perf -- generate-fixtures tools/perf
cargo test --locked -p rubix-dev --test perf_harness
```

These qualification commands intentionally exit nonzero for the committed
fixtures; `report` emits an explicitly unqualified comparison before failing:

```sh
cargo run --locked -p rubix-dev --bin rubix-perf -- check-baselines tools/perf/fixtures
cargo run --locked -p rubix-dev --bin rubix-perf -- evaluate-gates \
  --reference tools/perf/fixtures/amd64-reference-go.json \
  --candidate tools/perf/fixtures/amd64-candidate-rust.json
cargo run --locked -p rubix-dev --bin rubix-perf -- report \
  --reference tools/perf/fixtures/amd64-reference-go.json \
  --candidate tools/perf/fixtures/amd64-candidate-rust.json
```

`verify-secondary` checks gap-record structure only. The secondary examples record
missing qualification and the platform's disabled D2K / unavailable Portainer
cases. They authorize no pod-density ceilings or relaxed latency multipliers.
Existing contracts and the authoritative upstream-input inventory own platform
support and payload availability.
