# Go-to-Rust State Preservation Checks

Production migration remains **unqualified** under C13/E30. This change supplies
isolated state-preservation checks and a native datastore format experiment; it
does not establish a supported cluster cutover or a measured downtime window.

## Selected production boundary

The [component-boundary ADR](../../experiments/component-boundary/ADR.md) retains
Kine v0.16.3 and the upstream Kubernetes executables. Kine owns its SQLite state
at the selected instance's `kine/db/state.db`. Preserve that database, its WAL,
configuration, and identities through a production transition. The API-server
uses Kine's etcd-compatible protocol over loopback mTLS with a dedicated datastore
CA and separate API-server client identity. Preserve this trust boundary, including
its wrong-client and no-client certificate rejection behavior.

Replacing SQLite with `RUBXSNP1` snapshots is **not** a production migration path:
the selected Kine executable cannot consume that native format. Any change of
datastore boundary requires an accepted ADR change and independent live evidence.

## Fixture scope and state invariants

The version catalog classifies historical versions `v1.1.8` through `v1.3.3`,
rejecting older, empty and malformed versions. These are fixture starting points,
not a claim that all production upgrades have been rehearsed.

- Configuration fixtures exercise legacy systemd flag conversion and preservation
  of existing `kubesolo.io/v1alpha1` YAML. Keep `/etc/kubesolo` defaults and backups.
- PKI checks require parseable CA and service-account private keys on both sides,
  compare their bytes, and verify the client certificate signature and validity
  under the preserved CA. Preserve cluster and dedicated datastore identities;
  never replace a missing signing key with an empty successful value.
- Workload checks reject duplicates and reconcile identities in both directions,
  including API version, kind, namespace, name, UID, resource version and spec hash.
  Static manifest bytes must also be preserved.
- Volume checks include the root, directories, regular files and symlinks without
  following symlink targets. Compare file hashes, entry types, link targets,
  uid/gid and complete Unix permission bits. Missing roots and unsupported entry
  types are errors. These checks do not provide a concurrency-safe live snapshot;
  freeze writers or use a consistent filesystem capture before comparing.
- Runtime sockets, PIDs and process state are ephemeral and require owned-resource
  handling; classification does not authorize deleting unrelated host state.

## Native snapshot experiment

`validate_datastore_transition` accepts in-memory `KineRecord` fixtures. It writes
a fake SQLite header to exercise native format rejection, converts fixture records
to a native snapshot, and restores into the experimental `DatastoreEngine`.
The format is `RUBXSNP1`, a big-endian u64 revision, and a JSON key/value map with
`backup-meta.json`. This does not open SQLite, read an actual Kine database,
checkpoint a WAL, or establish Kubernetes protocol/lease/watch compatibility.
It always reports `wal_checkpointed: false` and
`production_transition_qualified: false`.

## Required production evidence

Before closing E30, capture a disposable Linux rehearsal at the exact source and
candidate revisions against the retained executables. Establish a consistent
Kine SQLite backup/checkpoint with all writers stopped or a verified consistent
backup mechanism; preserve and hash the database, WAL where applicable, PKI,
configuration, static manifests and volume state. Restore and prove API access,
identities, values, revisions, workload storage access and dedicated datastore
mTLS rejection behavior. Keep rollback artifacts and rehearse recovery before
any cutover. Measure downtime and workload availability; neither is inferred from
these fixture checks. Coordinate with external runtime ownership and supervisor
contracts rather than assuming running workloads survive a binary swap.

## Running fixture checks

```sh
cargo run --locked -p rubix-dev --bin rubix-state-transition
cargo test --locked -p rubix-dev --test suite state_transitions::
```

The command checks static catalogs and synthetic kubeconfig samples and explicitly
prints that production migration is unqualified. It consumes no production node
or database capture. Passing tests on macOS do not qualify Linux migration.

## Operator Recovery & Migration Failure Rehearsal (Gate C14 / E30.02)

**Issue:** #125 (`[E30.02] Rehearse migration failure and operator recovery`)
**Gate:** C14 remains unqualified. Disposable filesystem fixtures exercise lifecycle policy;
they do not demonstrate a live Kine migration, application readiness, or process-crash recovery.

### Transition Stages & Lifecycle Boundaries

Upgrade and migration workflows transition through 11 discrete stages, classified by disk mutation and rollback requirements:

| Stage | Classification | Active Receipt | Recovery Action |
| --- | --- | --- | --- |
| `Validation` | Pre-mutation | None | Reject invalid version without service changes |
| `Preparation` | Pre-mutation | None | Reject unavailable artifact without service changes |
| `Quiesce` | Pre-mutation | None | Restart old service |
| `Snapshot` | Pre-mutation | None | Remove partial snapshot, restart old service |
| `ReceiptPending` | Pre-mutation | None if persistence fails | Restart old service; retain complete orphaned backup for inspection |
| `ArtifactReplacement` | Mutating | `.upgrade-pending` | Full rollback from backup |
| `ConfigMigration` | Mutating | `.upgrade-pending` | Full rollback from backup |
| `ServiceStart` | Mutating | `.upgrade-pending` | Full rollback from backup (reverses dirty datastore/pki writes) |
| `ReceiptCommitting` | Committing | `.upgrade-committing` | Request target start, verify container running state, finalize commit |
| `Commit` | Post-commit | `.upgrade-completed` | Idempotent receipt removal |
| `PostCommitCleanup` | Post-commit | `.upgrade-completed` | Idempotent receipt removal |

### Recovery Procedure & Fail-Closed Integrity Validation

When an upgrade fails or is interrupted mid-flight, the operator explicitly requests
recovery via `rubixctl upgrade --recover --path /var/lib/kubesolo`. There is no automatic
supervisor recovery. Backups created before checksummed integrity evidence was implemented
are refused; keep them for manual inspection rather than generating evidence after corruption.

1. **Receipt Inspection:** Under the installation lock, read the active receipt to discover `from`, `target`, and the owned `backup` path. Completed receipts only require cleanup.
2. **Fail-Closed Backup Integrity Validation:** Before stopping or starting services:
   - Verify backup directory exists and is a directory (not an unsafe symlink).
   - Require a private `0700` direct child of the installation's backup directory, complete `pki` and `kine/db` directories, and backend-specific binary/service or container-spec material.
   - Compare every entry's type, permissions, length and SHA-256 against evidence captured once from the quiesced snapshot before replacement. Reject changed, missing, extra or linked entries, including intermediate directories.
   - These checks preserve the original bytes; they do not prove the original database or identities were healthy. The integrity record is not an authenticated signature or SQLite integrity check.
   - If backup is missing, corrupt, or empty, recovery **aborts immediately without mutation** (`BackupIntegrityError`), retaining active receipts and error diagnostics for operator triage.
3. **Quiesce and State Restoration (Rollback for Pending Upgrades):** Require successful stop (or a structured observation proving absence/inactivity) before restoring state. For containers, verify exact active/rollback names and image references and reconstruct crash-lost replacement state first.
   - Restore binary executable and host service units captured in the original snapshot; an initially absent standard unit is permitted, but deletion from a sealed snapshot is refused.
   - Restore Kine SQLite database (`state.db`) and WAL files, reversing partial migrations or corrupt writes.
   - Restore PKI private keys (`ca.key`, `service-account.key`) and certificates (`ca.crt`).
   - Restore configuration according to version-specific limitations.
   - Clean up `.upgrade-pending` upon successful restoration.
4. **State Finalization (Commit for Committing Upgrades):**
   - Request target start. Container recovery checks running state; host service-start success does not establish application readiness.
   - Retain `.upgrade-committing` and diagnostics if backend finalization fails; retry uses observed engine state rather than lost in-memory flags.
   - Clean receipts only after successful commit. If start fails, rollback requires the old artifact to remain available; otherwise retain evidence for manual repair.
   - Before rollback changes artifacts or state, durably change `.upgrade-committing` to `.upgrade-pending` so interrupted or failed restoration retries rollback rather than target finalization.

### Fixture Checks Across 5 Core Domains

Filesystem fixtures check restored or retained state across these domains:
1. **Configuration:** Restores exact pre-upgrade configuration files and permissions (`0600`).
2. **PKI & Identities:** Restores exact captured keys/certificates. The fixture separately verifies a synthetic client certificate signature; recovery does not establish original PKI health.
3. **Datastore:** Restores exact captured database/WAL bytes. The rehearsal uses synthetic byte files, not a live Kine SQLite database, and establishes no schema compatibility or integrity-check result.
4. **Workloads:** Retains static manifests and pod definitions bit-for-bit.
5. **Storage:** Preserves persistent volume directory trees, regular files, permissions, and symlink integrity without corruption.

### Dual-Format Client Access Accommodation

Kubeconfig access is validated across both historical and modern formats:
- **YAML Format:** Standard Kubernetes kubeconfig with client certificate/key authentication.
- **JSON Format:** Strict JSON representation parsed and validated against cryptographic CA roots.
Both formats check synthetic credentials after fixture recovery; no live API access is exercised.

### Version-Specific Known Limitations

| Starting Version | Configuration Behavior | Rollback & Recovery Specifics |
| --- | --- | --- |
| `v1.1.8` | Legacy flags in the systemd unit `ExecStart` line; no `config.yaml` | Rollback deletes migration-created `/etc/kubesolo/config.yaml` and restores systemd unit flags. |
| `v1.2.0` | Legacy flags in the systemd unit `ExecStart` line; no `config.yaml` | Rollback deletes migration-created `/etc/kubesolo/config.yaml` and restores systemd unit flags. |
| `v1.3.0` | First version introducing standalone `/etc/kubesolo/config.yaml` | Rollback restores original YAML configuration file with strict `0600` permissions. |
| `v1.3.1` | YAML configuration file with updated component defaults | Rollback restores original YAML configuration, preserving custom configuration fields. |
| `v1.3.2` | YAML configuration file with updated component defaults | Rollback restores original YAML configuration, preserving custom configuration fields. |
| `v1.3.3` | YAML configuration file with updated component defaults | Rollback restores original YAML configuration, preserving custom configuration fields. |

### Running Failure Rehearsal & Verification

```sh
# Execute automated failure rehearsal matrix across all versions and stages
cargo run --locked -p rubix-dev --bin rubix-recovery-rehearsal

# Run synthetic filesystem and mocked-service regression tests
cargo test --locked -p rubix-dev --test suite recovery_rehearsal::
cargo test --locked -p rubixctl --test suite upgrade_review::
```
