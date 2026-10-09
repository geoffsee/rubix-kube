# State Migration & Recovery Rehearsal Runbook

This runbook guides operators through running the live Go-to-Rust Kine SQLite migration
and interrupted recovery rehearsals under Epic E36 / Issue #354.

## Overview & Architecture Boundary

Per the [component-boundary ADR](../../experiments/component-boundary/ADR.md) (amended 2026-10-07),
Rubix selects **Option B (in-process Rust control plane)** as the production architecture.
Under Option B:
- `rubix-apiserver` binds directly in-process to `rubix-datastore` via `KubernetesStorage`.
- `rubix-datastore` uses an in-memory MVCC index, WAL, and native snapshot format (`RUBXSNP1`).
- **Raw SQLite Non-Interchangeability**: `rubix-datastore` explicitly rejects raw SQLite files
  by design (`invalid snapshot magic header`). Migration from legacy Go/Kine installations requires
  an explicit export/import step rather than raw database adoption.

The mandatory scope marker emitted during execution is:
```text
E36.04:scope: Option B in-process control plane selected (ADR amended 2026-10-07); raw SQLite non-interchangeable; explicit export/import required
```

See [State Transitions Architecture](../architecture/state-transitions.md) for background and contracts.

## Rehearsal Matrix

The rehearsal exercises all 6 supported starting versions across both kubeconfig formats (12 matrix runs):

| Starting Version | Layout Description | YAML Kubeconfig | JSON Kubeconfig |
| --- | --- | --- | --- |
| `v1.1.8` | Legacy flags in systemd unit | Supported | Supported |
| `v1.2.0` | Legacy flags in systemd unit | Supported | Supported |
| `v1.3.0` | First version with `/etc/kubesolo/config.yaml` | Supported | Supported |
| `v1.3.1` | Standalone YAML config | Supported | Supported |
| `v1.3.2` | Standalone YAML config | Supported | Supported |
| `v1.3.3` | Standalone YAML config | Supported | Supported |

## Verified Invariants

For every matrix combination, the rehearsal verifies:
1. **Option B Raw SQLite Rejection**: Verifies `assert_raw_sqlite_rejected` fails when presented with a raw SQLite database header.
2. **Native Snapshot Conversion**: Converts Kine records into `RUBXSNP1` format and restores into `DatastoreEngine`.
3. **Revision Monotonicity & Key Integrity**: Verifies `restored_rev >= source_max_rev` and all active keys match expected payloads.
4. **PKI Trust Roots**: Verifies cluster CA SHA-256 fingerprint preservation and cryptographic x509 chain verification for client credentials.
5. **Static Manifests**: Bit-for-bit retention of static pod manifests.
6. **Persistent Volume Storage**: Preserves PV file trees, byte checksums, and Unix permissions.
7. **Downtime Measurement**: Measures duration in milliseconds (`Instant::now()`) around the quiesce-to-start window.

In addition, interrupted recovery is rehearsed across all 11 lifecycle stages, and fail-closed refusal is verified for missing, corrupted, and symlinked backups.

## Running the Rehearsal

### 1. Verification Run

To run the complete failure rehearsal and live migration matrix:

```sh
cargo run --locked -p rubix-dev --bin rubix-recovery-rehearsal
```

### 2. Candidate Qualification Receipt Generation

On disposable Linux test infrastructure, operators can generate a signed Criterion 8 candidate qualification receipt:

```sh
cargo run --locked -p rubix-dev --bin rubix-recovery-rehearsal -- --generate-receipt /tmp/criterion-08-receipt.json
```

> [!IMPORTANT]
> In accordance with repository release qualification policy, candidate qualification receipts must NOT be committed to `docs/release/receipts/` in the repository checkout. Criterion 8 must remain `satisfied: false` until formal release qualification on qualified infrastructure.

## Operator Recovery Procedure

If an upgrade is interrupted:
1. Check for active receipt (`.upgrade-pending` or `.upgrade-committing`) in `/var/lib/kubesolo`.
2. Inspect the backup directory referenced by the receipt.
3. If rollback is needed, run:
   ```sh
   rubixctl upgrade --recover --path /var/lib/kubesolo
   ```
4. Verify service status and client connectivity.
