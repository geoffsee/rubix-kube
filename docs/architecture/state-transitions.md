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
cargo test --locked -p rubix-dev --test state_transitions
```

The command checks static catalogs and synthetic kubeconfig samples and explicitly
prints that production migration is unqualified. It consumes no production node
or database capture. Passing tests on macOS do not qualify Linux migration.
