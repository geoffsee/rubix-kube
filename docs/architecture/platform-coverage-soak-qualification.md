# Platform coverage and soak fixture tooling

Issue [#120 / E28.03](https://github.com/geoffsee/rubix-kube/issues/120), Gate C14,
is **in progress and not qualified**. This tool has no retained-node installation,
24-hour workload, restart/recovery or authenticated current-source execution collector.
`run` and `verify` fail closed. Changing an evidence-kind string or pass flag does
not authorize qualification.

The [acceptance matrix](acceptance-matrix.md), [compatibility contract](compatibility-contract.md)
and [selected component boundary](../../experiments/component-boundary/ADR.md) remain
authoritative. Synthetic fixtures do not run kube-apiserver, controller-manager,
kubelet, kube-proxy, Kine or containerd, and do not establish supported host execution.

The canonical plan lists 16 node cells, four management targets plus the Windows
exclusion, 28 OCI platform decisions, and all declared runtime, init, container,
host, delivery, ownership and addon dimensions. Its supported entries are marked
synthetic plans; unsupported entries retain explicit rationales. Validation requires
exact unique identities and canonical metadata/results, with no omitted or extra record.

## Fixture arithmetic

```sh
cargo run --locked -p rubix-dev --bin rubix-platform-soak -- fixture --output /tmp/platform-fixture
cargo run --locked -p rubix-dev --bin rubix-platform-soak -- verify-fixture /tmp/platform-fixture/platform-soak-report.json
cargo test --locked -p rubix-dev --test suite platform_soak::
```

A schema-2 fixture has `evidence_kind= SyntheticFixture`, `overall_qualified=false`
and no candidate observations. JSON round trips check consistency only. Markdown
always states NOT QUALIFIED; invalid records cannot be rendered as passing evidence.
The `matrix`, `soak`, `restarts` and `regressions` commands display synthetic plans
or arithmetic inputs, not measurements or execution receipts.

Soak checks recompute growth from positive initial/final RSS, reject non-finite or
inconsistent ratios, require all four unique primary architectures, 24 declared hours,
at least 86,400 synthetic seconds, positive cycles and zero declared failure counts.
Restart checks use fixed canonical limits and exact case identities; caller-authored
larger limits cannot relax them. Historical and per-epic records must match their
canonical fixture inventories. These checks do not authenticate any assertion that
resources, objects, UIDs, certificates or volume bytes survived real recovery.

## Independently read candidate bytes

```sh
cargo run --locked -p rubix-dev --bin rubix-platform-soak -- verify-candidate release-manifest.json /path/to/artifacts --expected-version VERSION
```

This separate operation validates the manifest using the existing strict release
schema/product/version/cell/management/OCI/bundled-asset gates. The expected version
is supplied independently. It streams hashes and byte counts from regular files:

- Node archives and management binaries use their canonical manifest filenames.
- Each OCI index is `oci/ASSET/index.json`.
- Each platform manifest is `oci/ASSET/sha256/DIGEST.json`, where DIGEST is the
  descriptor's hexadecimal SHA-256 without its `sha256:` prefix.

Every index and platform descriptor must have an independent file observation.
Missing, duplicate, unexpected, mismatched or wrong-size observations fail; expected
manifest values are never substituted for observations. The opaque observation type
can only be constructed by reading bytes. No files are downloaded or manufactured.
This checks byte identity, not archive layout, OCI content semantics, signature trust,
installation, workload execution or release qualification. Serialized hash summaries
are not accepted as evidence by `verify` or by synthetic fixture reports.

C14 remains blocked pending current-source disposable Linux receipts for all promised
environments, real 24-hour workloads and settled-memory samples, retained-executable
restart/recovery and independent state preservation, and historical/per-epic gates.

## Live qualification harness and Criterion 6 receipt capture

Issue #352 (E36.02) provides executable qualification capture and receipt validation
for Criterion 6 (`criterion-06-conformance-and-soak.json`):

```sh
# Run candidate-bound soak and platform qualification (supports custom duration/cycles for rehearsal)
cargo run --locked -p rubix-dev --bin rubix-platform-soak -- capture --output /tmp/soak-qualification --duration 30 --cycles 5

# Full 24-hour soak run (86,400 seconds)
cargo run --locked -p rubix-dev --bin rubix-platform-soak -- capture --output /tmp/soak-24h --duration 86400

# Verify generated receipt against canonical schema and Criterion 6 rules
cargo run --locked -p rubix-dev --bin rubix-platform-soak -- verify-receipt /tmp/soak-qualification
```

### Assertions evaluated

The soak qualification captures live memory samples across repeated workload cycles, validating five core assertions:

1. `soak_memory_growth_bound`: verifies final RSS does not exceed initial RSS by more than 10% (ratio <= 1.10).
2. `soak_zero_oom_events`: verifies zero out-of-memory terminations occurred during execution.
3. `soak_zero_crashes`: verifies zero unhandled panics or supervisor aborts.
4. `soak_zero_unexplained_probe_failures`: verifies readiness probes succeed consistently throughout execution.
5. `soak_workload_cycles_positive`: verifies at least one workload cycle was executed and verified against the API.

### Partial rehearsal runs

When executed with `--duration < 86400`, the harness records `partial: true` in `soak-report.json` and records a skip for `sustained_24h_soak_completion` ("Observed duration < 86400s; recorded as partial rehearsal run"), ensuring that short rehearsal runs can validate harness mechanics without incorrectly asserting 24-hour qualification. Full 24-hour qualification requires running the full 86,400s duration on a disposable Linux environment.

Existing matrix inspection (`matrix`), synthetic fixture generation (`fixture`), candidate byte verification (`verify-candidate`), and regression checking commands remain supported and unchanged.

