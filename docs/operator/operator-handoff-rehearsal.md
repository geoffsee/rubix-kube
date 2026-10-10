# Operator handoff rehearsal runbook (Criterion 11)

This runbook defines the operational protocol for qualifying Roadmap #263 **Criterion 11:
Operator Documentation & Release Qualification** ("fresh operator rehearsal, exact artifacts
and trusted publication").

Read the [acceptance matrix](../architecture/acceptance-matrix.md),
[compatibility contract](../architecture/compatibility-contract.md),
[component boundary ADR](../../experiments/component-boundary/ADR.md),
[upstream inventory](../architecture/upstream-inputs.md),
[fresh installation runbook](fresh-installs.md), and
[migration and recovery runbook](migration-and-recovery.md) before executing qualification.

---

## 1. Operational scope and integrity policy

Passing unit tests, static code analysis, or documentation alone **do not** qualify Criterion 11.
Criterion 11 requires a comprehensive, candidate-bound rehearsal executed across all 5 operational
phases, capturing exact logs and evidence, and emitting a cryptographic candidate receipt.

### Core integrity rules
1. **Zero committed receipts on default checkout**: Qualification receipts are candidate-bound
   artifacts. No receipt is committed to `docs/release/receipts/criterion-11-operator-handoff.json`.
   On a clean repository checkout, Criterion 11 evaluates to `Pending` (`satisfied: false`).
2. **Zero skips permitted**: Unlike criteria where explicit hardware gaps may be justified in
   the `skips` ledger, Criterion 11 operator handoff qualification strictly forbids skips
   (`receipt.skips.is_empty()` must be true). Any documented skip causes immediate fail-closed rejection.
3. **All 18 command executions must exit 0**: Every step across all 5 phases must execute to completion
   with exit code 0.
4. **All 5 assertions must pass**: The 5 required assertions must be present and verified with
   `passed: true` and cannot contain `"non-qualifying"` markers.
5. **Fail-closed verification**: The verification gate verifies candidate source revision binding,
   SHA-256 payload integrity, and command log integrity.

---

## 2. Rehearsal phases and command execution inventory

The operator handoff rehearsal exercises 5 operational phases comprising 18 distinct commands and log files:

### Phase 1: Fresh installation and client access handoff
- `01_preflight.log`: Host preflight inspection via `rubixctl check` verifying prerequisites, cgroups, and storage mounts.
- `02_pki_bootstrap.log`: PKI reconciliation and certificate authority bootstrapping via `ClusterPki::reconcile`.
- `03_kubeconfig_yaml.log`: Dual-format client credential handoff: validation and parsing of YAML-formatted kubeconfig.
- `04_kubeconfig_json.log`: Dual-format client credential handoff: validation and parsing of JSON-formatted kubeconfig.

### Phase 2: Workload placement and storage admission
- `05_mutate_workload.log`: Admission control webhook execution (`NodeSetterHandler::mutate_pod`) ensuring correct node affinity and tolerations.
- `06_reconcile_storage.log`: LocalPath storage manifest rendering (`LocalPathManifests::render`) for host storage provisioners.
- `07_verify_io.log`: Directory traversal prevention and path containment verification (`safe_resolve_volume_path`).

### Phase 3: Metrics and observability scraping
- `08_metrics_probe.log`: Health probe endpoint verification (`/healthz`, `/livez`, `/readyz`).
- `09_metrics_prometheus.log`: Standard Prometheus text format scrape (`text/plain; version=0.0.4`).
- `10_metrics_openmetrics.log`: OpenMetrics text format scrape (`application/openmetrics-text; version=1.0.0`).
- `11_metrics_negotiation.log`: HTTP `Accept` header content negotiation with weighted q-values.

### Phase 4: State migration compatibility and datastore integrity
- `12_state_transition_matrix.log`: In-process migration rehearsal across all supported starting versions (`v1.1.8` through `v1.3.3`).
- `13_sqlite_rejection.log`: Strict enforcement of Option B boundary per ADR: raw SQLite datastore adoption rejection.
- `14_wal_integrity.log`: Write-ahead log (WAL) integrity, monotonic revision indexing, and MVCC datastore verification.

### Phase 5: Recovery rehearsal and cleanup boundary isolation
- `15_validate_data_path.log`: Data path validation (`validate_data_path`) ensuring safe host paths.
- `16_reset_dry_run.log`: Host cleanup dry-run execution (`plan_cleanup`) previewing disposable resources.
- `17_reset_execute.log`: Host cleanup live execution (`run_host_cleanup`) removing disposable runtime state while retaining persistent data.
- `18_verify_isolation.log`: Foreign file boundary check verifying that host-owned and neighboring files outside Rubix ownership are untouched.

---

## 3. Execution CLI

The operator rehearsal harness is provided by `rubix-operator-rehearsal`.

### Rehearsal capture
To run all 5 rehearsal phases, generate logs, and produce candidate-bound receipts:
```sh
cargo run --locked -p rubix-dev --bin rubix-operator-rehearsal -- capture --output target/operator-handoff-rehearsal
```

Output artifacts generated in the target directory:
- `criterion-11-operator-handoff.json`: Candidate-bound qualification receipt.
- `operator-handoff-report.json`: Detailed JSON execution report.
- `operator-handoff-report.md`: Human-readable markdown summary report.
- `01_preflight.log` through `18_verify_isolation.log`: Execution logs for all 18 commands.

### Receipt verification
To verify a candidate receipt against current candidate inventory and fail-closed rules:
```sh
cargo run --locked -p rubix-dev --bin rubix-operator-rehearsal -- verify-receipt target/operator-handoff-rehearsal/criterion-11-operator-handoff.json
```

---

## 4. Integration in CI / Disposable nodes

In CI (`.github/workflows/integration.yml`), the rehearsal runs on a disposable Linux runner:
1. Rehearsal capture runs against the built candidate binaries.
2. The receipt is verified immediately using `rubix-operator-rehearsal verify-receipt`.
3. The entire rehearsal directory (`${{ runner.temp }}/operator-handoff-rehearsal/`) is archived as a workflow artifact.
4. Receipts are NOT checked into Git, preserving the clean-checkout pending status for release gates.
