# Migration and operator recovery

Production Go-to-Rust cutover is **NOT QUALIFIED** under C13/C14/E30. Historical
version catalogs, file comparisons, recovery receipts and synthetic rehearsals do
not establish a supported live transition, preserved running workloads or measured
downtime. Read [state-transition limits](../architecture/state-transitions.md),
[the compatibility contract](../architecture/compatibility-contract.md) and the
[retained-executable ADR](../../experiments/component-boundary/ADR.md).

## Historical starting-point catalog

| Fixture version | Configuration behavior |
| --- | --- |
| v1.1.8 | Legacy flags in the service ExecStart; no canonical config file. |
| v1.2.0 | Legacy flags in the service ExecStart; no canonical config file. |
| v1.3.0 | First config-file version: `/etc/kubesolo/config.yaml`. |
| v1.3.1 | Preserve existing `/etc/kubesolo/config.yaml`. |
| v1.3.2 | Preserve existing `/etc/kubesolo/config.yaml`. |
| v1.3.3 | Preserve existing `/etc/kubesolo/config.yaml`. |

These are fixture cases, not six qualified production migration paths. Older,
empty or malformed catalog versions are refused. Upgrading an older cluster to
a listed version does not itself qualify a Rust cutover. There is no `rubixctl
migrate --from-version ... --dry-run` interface. Upgrade's legacy service/config
conversion must be inspected against the actual service and target; unsupported
or ambiguous inputs require operator investigation.

## Before a production cutover

Keep the existing deployment until a disposable exact-revision rehearsal succeeds.
Record source/target distribution versions, candidate hashes, binary and service
paths, resolved configuration, ownership and a tested repair plan. Protect
credentials and retain the original executable/service definition.

Stop every datastore writer or use an independently verified consistent backup
mechanism. Cordon or pause workload scheduling alone does not quiesce Kubernetes
controllers, Kine or volume writers. Preserve Kine's `kine/db/state.db` and its
WAL/SHM when applicable; a `wal_checkpoint` request can be busy or incomplete and
must not be assumed successful. Do not copy only state.db while writes continue.
Filesystem copying and sealed hash evidence preserve captured bytes but do not
prove SQLite consistency, schema compatibility or original semantic health.

Preserve cluster CA/signing keys, service-account identity, dedicated datastore
CA/client/server mTLS identities, configuration/backups, static manifests and
complete PV trees with ownership/modes/link targets. Never replace Kine SQLite
with the experimental `RUBXSNP1` snapshot format. Native datastore conversion
fixtures do not read or adopt a real Kine SQLite database.

Rehearse restored API access and wrong/no-client rejection, workloads and UID/
resourceVersion/spec reconciliation, real PV read/write access, network behavior,
backup restoration and downtime before cutover. File equality and YAML/JSON
kubeconfig parsing are not authenticated API or application readiness evidence.
A direct binary swap followed by a successful service-start request does not meet
these gates. This reference supplies no approved production cutover procedure.

## Implemented upgrade and explicit recovery interface

Choose a reviewed target **distribution** version and existing installation path:

```sh
rubixctl upgrade --version v1.3.3 --offline-install /media/candidate.tar.gz --path /var/lib/kubesolo
rubixctl upgrade --recover --path /var/lib/kubesolo
```

The artifact is an operator-selected verified candidate, not evidence that a Rust
release v1.3.3 exists. Recovery is explicitly requested; the supervisor does not
automatically recover interrupted upgrades. It dispatches receipt recovery without
downloading/staging a new target. Do not combine recovery with a new version or
artifact selection. Retain receipts and backups when a command fails; do not use
`uninstall --purge` to clear the error while recovery is still needed.

### Eleven stages and receipts

| Stage | Durable receipt | Behavior |
| --- | --- | --- |
| Validation | none | Reject invalid version without service changes. |
| Preparation | none | Reject unavailable artifact without service changes. |
| Quiesce | none | Restart old deployment on failure. |
| Snapshot | none | Remove partial snapshot and restart old deployment. |
| ReceiptPending | none if persistence fails | Restart old deployment; retain complete orphaned backup. |
| ArtifactReplacement | `.upgrade-pending` | Pending recovery rolls back captured state. |
| ConfigMigration | `.upgrade-pending` | Pending recovery rolls back captured state. |
| ServiceStart | `.upgrade-pending` | Pending recovery rolls back captured state after confirmed quiescence. |
| ReceiptCommitting | `.upgrade-committing` | Request target start, check container running state, finalize target. |
| Commit | `.upgrade-completed` | Retain terminal recovery evidence if cleanup fails. |
| PostCommitCleanup | `.upgrade-completed` | Retry idempotent receipt cleanup. |

Before effects, recovery requires the owned private 0700 direct child of the
installation's `backups` directory, complete PKI and `kine/db` plus backend
material, and exact full-tree type/mode/length/SHA-256 evidence sealed from the
quiesced snapshot before replacement. Missing/extra/changed/link entries are
refused, including unsafe intermediate directories. Backups predating integrity
evidence cannot be repaired by generating hashes after suspected corruption.

Pending rollback requires successful stop or structured proof of inactivity before
restoration. Container recovery reconstructs persisted replacement state and checks
exact active/rollback identities; it does not rely on lost in-memory flags.
Committing recovery finalizes the target; completed receipts need cleanup only.
Failed validation, quiescence, commit or recovery retains evidence for inspection.
The integrity record detects changes to originally captured bytes; it is neither
an authenticated publisher signature nor a PKI/SQLite health certificate.

Host service-start success is only a request. Container running-state checks do
not prove API readiness. Post-commit cleanup/output failure is a warning about
retained evidence, not authority to roll back an already committed target whose
old artifact may no longer exist.

## Declared targets, no observed SLA

| Criterion | Declared target | Current observed production result |
| --- | --- | --- |
| Clean component restart | 10 seconds | Not measured/qualified. |
| Datastore crash restart | 10 seconds | Not measured/qualified. |
| Outage shutdown escalation | 5 seconds | Not measured/qualified. |
| Startup cancellation re-entry | 5 seconds | Not measured/qualified. |
| Scoped reset cleanup | 30 seconds | Not measured/qualified. |
| Real 24-hour settled memory | <=1.10x initial; zero failures | Not measured/qualified. |

Canonical synthetic timing/RSS inputs are not execution receipts or an SLA. C14
remains pending actual retained-node workloads and independently observed recovery.

## Platform limits

Native macOS nodes and native Windows binaries are unsupported; supported management
clients require a Linux Engine (including Docker Desktop/WSL2). Static CPU policy
is rejected in container mode. Portainer riscv64 and D2K ARMv7/riscv64 are excluded
by upstream availability. Supported matrix entries are obligations to qualify,
not proof that these migration procedures have been rehearsed on every platform.
