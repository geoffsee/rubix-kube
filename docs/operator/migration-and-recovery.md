# Migration & Operator Recovery Runbook

This runbook provides actionable procedures for migrating legacy Go KubeSolo clusters to
Rubix (Rust distribution), recovering from interrupted upgrades or host crashes, and understanding
operational downtime bounds and platform constraints.

---

## 1. Supported Starting Versions & Compatibility Matrix

Rubix provides automated, fail-closed state migration for legacy clusters running supported versions:

| Starting Version | Configuration Source | On-Disk Layout | Migration Action Required |
| --- | --- | --- | --- |
| `v1.1.8` | systemd drop-in `flags.conf` | Legacy flags only | Converts flags to `/etc/kubesolo/config.yaml`; preserves datastore & PKI. |
| `v1.2.0` | systemd drop-in `flags.conf` | Legacy flags only | Converts flags to `/etc/kubesolo/config.yaml`; preserves datastore & PKI. |
| `v1.3.0` | `/etc/kubesolo/kubesolo.yaml` | Standalone YAML | Adopts YAML into canonical `config.yaml`; preserves datastore & PKI. |
| `v1.3.1` | `/etc/kubesolo/kubesolo.yaml` | Standalone YAML | Adopts YAML into canonical `config.yaml`; preserves datastore & PKI. |
| `v1.3.2` | `/etc/kubesolo/kubesolo.yaml` | Standalone YAML | Adopts YAML into canonical `config.yaml`; preserves datastore & PKI. |
| `v1.3.3` | `/etc/kubesolo/config.yaml` | Canonical layout | Direct binary update; verifies schema & preserves state. |

> [!WARNING]
> Versions prior to `v1.1.8` (e.g. `v1.0.0`, `v1.1.7`), unversioned development builds, and custom
> forks fail migration preflight fail-closed. These clusters must be upgraded to `v1.1.8`+ before migrating to Rubix.

---

## 2. Step-by-Step Go-to-Rust Migration Procedure

Follow these steps to transition a running Go KubeSolo node to Rubix:

### Phase 1: Pre-Migration Backup & Inspection
1. **Verify Cluster Health:**
   ```sh
   kubectl get nodes
   kubectl get pods -A
   ```
2. **Quiesce Writers (Recommended):**
   Temporarily pause or cordon workload deployments to minimize in-flight database writes.
3. **Stop Legacy Service:**
   ```sh
   sudo systemctl stop kubesolo
   ```
4. **Checkpoint Kine SQLite WAL:**
   Flush the Write-Ahead Log into the main SQLite database:
   ```sh
   sqlite3 /var/lib/kubesolo/kine/db/state.db "PRAGMA wal_checkpoint(TRUNCATE);"
   ```
5. **Create Immutable Backup Snapshot:**
   ```sh
   sudo mkdir -p /var/backups/kubesolo-pre-migration
   sudo cp -a /var/lib/kubesolo/pki /var/backups/kubesolo-pre-migration/
   sudo cp -a /var/lib/kubesolo/kine /var/backups/kubesolo-pre-migration/
   [ -f /etc/kubesolo/kubesolo.yaml ] && sudo cp /etc/kubesolo/kubesolo.yaml /var/backups/kubesolo-pre-migration/
   [ -f /etc/systemd/system/kubesolo.service.d/flags.conf ] && sudo cp -r /etc/systemd/system/kubesolo.service.d /var/backups/kubesolo-pre-migration/
   ```

### Phase 2: Configuration Conversion
Rubix automatically parses existing CLI flags and systemd drop-ins and renders canonical
`kubesolo.io/v1alpha1` YAML:
```sh
# Generate and validate new configuration
sudo rubixctl migrate --from-version v1.2.0 --dry-run
```
Inspect the generated `/etc/kubesolo/config.yaml`:
```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
path: /var/lib/kubesolo
network:
  nodeIP: "192.168.1.100"
  loadBalancer:
    enabled: true
storage:
  localPath:
    enabled: true
```

### Phase 3: Binary Replacement & Service Activation
1. Install Rubix node and management binaries:
   ```sh
   sudo cp rubix-kube /usr/local/bin/kubesolo
   sudo cp rubixctl /usr/local/bin/rubixctl
   sudo chmod 0755 /usr/local/bin/kubesolo /usr/local/bin/rubixctl
   ```
2. Reload systemd and start the Rubix service:
   ```sh
   sudo systemctl daemon-reload
   sudo systemctl start kubesolo
   ```
3. Verify node readiness:
   ```sh
   rubixctl check
   kubectl get nodes
   ```

---

## 3. Preserved State Invariants Across 5 Architectural Domains

The migration process strictly guarantees state preservation across:

1. **Configuration:** Permissions are locked to `0600`. File contents are preserved with atomic backups.
2. **PKI & Identities:** The root CA (`ca.crt`, `ca.key`), service-account private key (`service-account.key`), and dedicated datastore loopback mTLS certificates are preserved byte-for-byte. Client certificates remain valid under the original CA root.
3. **Datastore (Kine SQLite):** Kine's SQLite database (`state.db`), WAL, and SHM files are retained. Native in-memory datastore experiments (`RUBXSNP1`) are **never** used in production; Kine continues managing SQLite directly.
4. **Workloads:** Existing pods, namespaces, UIDs, and resourceVersions survive the transition without recreation.
5. **Persistent Volumes:** LocalPath storage directories under `/var/lib/kubesolo/local-path-storage/` (or configured `sharedPath`) retain file ownership, permissions, and symlink structures.

---

## 4. Disaster Recovery & Interrupted Upgrade Playbook

During upgrades and migrations, Rubix progresses through 10 discrete stages protected by durable
receipt markers:

| Stage | Classification | Active Receipt | Automatic Recovery Action |
| --- | --- | --- | --- |
| `Validation` | Pre-mutation | None | Restart previous service |
| `Preparation` | Pre-mutation | None | Clean temporary staging paths |
| `Quiesce` | Pre-mutation | None | Restart previous service |
| `Snapshot` | Pre-mutation | None | Remove partial snapshot, restart previous service |
| `ReceiptPending` | Pre-mutation | `.upgrade-pending` | Clean receipt, restart previous service |
| `ArtifactReplacement` | Mutating | `.upgrade-pending` | Full rollback from backup directory |
| `ConfigMigration` | Mutating | `.upgrade-pending` | Full rollback from backup directory |
| `ServiceStart` | Mutating | `.upgrade-pending` | Full rollback from backup (reverses dirty writes) |
| `ReceiptCommitting` | Committing | `.upgrade-committing` | Verify health and finalize commit |
| `Commit` | Post-commit | `.upgrade-completed` | Clean up commit receipts |

### Triggering Automated Recovery
If an upgrade is aborted or fails during execution:
```sh
sudo rubixctl upgrade --recover
```

### Fail-Closed Backup Integrity Validation
Before modifying any host files during recovery, Rubix executes strict fail-closed checks:
- Verifies the backup directory exists and is a genuine directory (not a symlink).
- Verifies that valid, non-empty `pki` and `kine/db` directories exist within the backup.
- If the backup is missing or corrupt, recovery aborts immediately without modifying the host, retaining error diagnostics for operator review.

### Dual-Format Client Access Validation
Following recovery, Rubix validates that both YAML and JSON formatted kubeconfig structures
can successfully authenticate against the API server.

---

## 5. Contractual Downtime Bounds & SLA

Under Gate C14/C16 qualification, Rubix establishes the following measured timing bounds:

| Scenario / Operation | Metric | Declared Contractual Bound | Observed Execution Time |
| --- | --- | --- | --- |
| **Clean Component Restart** | Node readiness recovery | **<= 10 seconds** | ~3,250 ms |
| **Datastore Crash Restart** | SQLite recovery & API access | **<= 10 seconds** | ~2,800 ms |
| **Outage Shutdown Escalation** | Process escalation & kill | **<= 5 seconds** | ~1,200 ms |
| **Startup Cancellation Re-entry** | Lock release & clean re-entry | **<= 5 seconds** | ~850 ms |
| **Scoped Reset Cleanup** | State cleanup completion | **<= 30 seconds** | ~4,100 ms |
| **24-Hour Sustained Soak** | Settled memory growth ratio | **<= 1.10x initial** | 1.050x (PASS) |

---

## 6. Known Platform Limitations & Technical Rationale

Operators should be aware of the following technical limitations:

1. **macOS Bare-Metal Node Daemon:**
   - *Status:* Explicitly unsupported.
   - *Rationale:* macOS does not possess native Linux cgroups or namespaces. Rubix runs on macOS exclusively via Docker container mode (`rubixctl container create`).
2. **Native Windows Binaries:**
   - *Status:* Excluded.
   - *Rationale:* Windows node workloads must run inside WSL2 using the standard Linux container engine workflow.
3. **Static CPU Manager in Container Mode:**
   - *Status:* Unsupported.
   - *Rationale:* Requires exclusive cpuset cgroups on the physical host, which nested container engines cannot provide.
4. **Portainer Agent on RISC-V (`riscv64`):**
   - *Status:* Unsupported.
   - *Rationale:* Upstream Portainer does not compile or publish `linux/riscv64` agent binaries.
5. **D2K Docker API Bridge on ARMv7 and RISC-V:**
   - *Status:* Disabled.
   - *Rationale:* Upstream D2K Docker bridge binaries are published only for 64-bit architectures (`amd64`, `arm64`).
