# Recovery, Lifecycle, and Ownership Qualification Report

**Gate:** C13 (Final candidate child outputs)  
**Issue:** #119 ([E28.02] Qualify restart, ownership and failure recovery)  
**Parent Epic:** E28 (Qualify Kubernetes compatibility and failure recovery, #28)  
**Date:** 2026-10-03  
**Status:** Qualified  

---

## 1. Scope and Objective

This report qualifies restart behavior, state ownership boundaries, and fault recovery for Rubix Kube in accordance with Epic E28 child deliverable **E28.02** and Gate **C13**.

The qualification exercises:
1. **Crash / Reboot Recovery**: Abrupt node and supervisor crashes followed by restart, verifying complete preservation of committed datastore state, PKI trust roots, node identity, and strict before/after directory ownership.
2. **Ungraceful Daemon Kills**: Unexpected exit / kill of core daemons (such as datastore or API server) triggering reverse-topological dependency shutdown, bounded escalation, diagnostic retention, and successful post-kill restart.
3. **Node-IP Changes**: Reconfiguration of node IP address across restarts, verifying automatic PKI leaf SAN rotation (while preserving the CA), server URL updates in kubeconfigs, and tolerance for both YAML block and JSON kubeconfig formats.
4. **Optional Service Failure Isolation**: Failures in optional components (e.g. Portainer Edge Agent, CoreDNS, Local Path Provisioner, Metrics, Config API) evaluated under `FailurePolicy::Degrade`, verifying that the supervisor continues running and the core Kubernetes API remains fully operational.
5. **Lifecycle Interruption**: Cancellation signals received during startup or lifecycle transitions, verifying clean shutdown without leaked locks or orphan processes, and ensuring subsequent restarts remain idempotent.
6. **Required Failure Gates**: Fail-closed rejection of corrupted datastore state without explicit opt-in repair (`dbWalRepair: true`), blocking release and preventing false readiness declarations.
7. **Historical Regression Mapping**: Mapping ten historical upstream and architectural regressions to passing verification runs and declared recovery bounds.

---

## 2. Declared Recovery Bounds and Escalation Limits

Rubix enforces explicit, declared bounds across component startup, shutdown escalation, and datastore recovery:

| Boundary Dimension | Declared Bound | Enforcement Mechanism | Failure Action |
| --- | --- | --- | --- |
| **Component Startup Deadline** | `600s` (default, configurable via `kubernetes.apiServer.startupTimeoutSeconds`) | `rubix_supervisor::Coordinator` deadline timer per component | Abort startup; emit `startup_timeout` diagnostic |
| **Shutdown Escalation** | `5s` per phase (Graceful -> Force) | `rubix_supervisor::process::OwnedChild` wait timer | Escalate `SIGTERM` to `SIGKILL`; assert process group absence |
| **Datastore Recovery Readiness** | `< 10s` | `rubix_datastore::engine::DatastoreEngine::open` WAL replay & VACUUM | Fail closed if WAL corrupt and `dbWalRepair: false` |
| **Kubeconfig Decoding Memory** | `<= 8 MiB` | `rubix_config::decode::DecodeLimits` bounded reader | Reject oversized documents with `InputExceedsLimit` |
| **Cancellation Grace Period** | `<= 5s` | `rubix_supervisor::StopReceiver` reaction loop | Transition components to `StopPhase::Graceful` then halt |
| **Init Backend Shutdown Wait** | `<= 30s` (s6 / systemd service stop) | `rubixctl::cleanup::CleanupHost` stop timeout | Abort cleanup if service stop fails |

---

## 3. Historical Regression Mapping

The following matrix maps historical regressions from upstream KubeSolo, architectural experiments, and component crates to verified passing runs, declared recovery bounds, and before/after ownership assertions:

| ID | Origin / Upstream Reference | Description & Historical Failure Symptom | Declared Recovery Bound | Verification Test & Evidence |
| --- | --- | --- | --- | --- |
| **REG-01** | Upstream #98 (KS-16) | CoreDNS falsely reported ready when replicas or readyReplicas were 0. | Readiness probe timeout `<= 10s` | `crates/rubix-dns/tests/dns_resolution_restarts.rs` & `recovery_lifecycle.rs` |
| **REG-02** | Component Boundary r1 | API server advertise address and SAN mismatch prevented loopback/client TLS handshake. | SAN reconciliation instantaneous during PKI reconcile | `test_node_ip_change_rotates_leaf_sans_and_updates_kubeconfig_both_formats` |
| **REG-03** | Component Boundary r2 | Datastore outage during shutdown caused API server graceful exit hang. | Forced escalation timeout `<= 5s` | `test_ungraceful_daemon_kill_escalation_and_diagnostic_retention` |
| **REG-04** | Component Boundary r3 | Recovery after datastore crash and acknowledged update retention. | Datastore recovery readiness `<= 10s` | `test_crash_reboot_preserves_datastore_pki_and_ownership` |
| **REG-05** | Upstream #178 (KS-75) | LoadBalancer external IP lost or unpopulated across service update / restart. | Admission update instantaneous | `crates/rubix-apiserver/src/admission.rs` & `recovery_lifecycle.rs` |
| **REG-06** | Upstream #190 (3fd84ca) | Host network failure on nftables-only or read-only `/proc/sys` hosts. | Preflight probe `<= 5s` | `crates/rubix-kube/tests/host_network.rs` & `recovery_lifecycle.rs` |
| **REG-07** | Rubix Datastore WAL | Torn write or corrupted WAL segment silently accepted or panicking. | Immediate fail-closed rejection | `test_required_datastore_corruption_fails_closed_and_blocks_release` |
| **REG-08** | Rubixctl #112 / #113 | Unscoped cleanup deleting PKI or persistent storage data on node reset. | State cleanup `<= 30s` | `crates/rubixctl/src/cleanup.rs` & `recovery_lifecycle.rs` |
| **REG-09** | Rubix PKI / Rubixctl #107 | Kubeconfig parsers rejecting valid JSON representation or formatting. | Decode memory `<= 8 MiB` | `assert_kubeconfig_format_and_server` (YAML & JSON dual parse) |
| **REG-10** | Rubix Supervisor #44 | Cancellation during startup leaving locked files or dangling threads. | Cancellation grace period `<= 5s` | `test_lifecycle_interruption_during_startup_leaves_clean_ownership` |

---

## 4. State Ownership Invariants Across Lifecycle Operations

Rubix strictly separates disposable runtime state from persistent cluster assets. Relative to the configured state root (`path`, default `/var/lib/kubesolo`):

| Relative Path | Component Owner | Crash / Reboot | Node Reset | Uninstall | Uninstall `--purge` |
| --- | --- | --- | --- | --- | --- |
| `pki/` | `rubix-pki` | **Preserved** | **Preserved** | **Preserved** | Removed |
| `datastore/kine.db` | `rubix-datastore` | **Preserved** | Removed | Removed | Removed |
| `datastore/kine.wal` | `rubix-datastore` | **Replayed & Preserved** | Removed | Removed | Removed |
| `datastore/.lock` | `rubix-datastore` | **Re-acquired** | Removed | Removed | Removed |
| `storage/` | `rubix-storage` | **Preserved** | **Preserved** | **Preserved** | Removed |
| `backups/` | `rubix-datastore` | **Preserved** | **Preserved** | **Preserved** | Removed |
| `kubelet/` | `rubix-kubelet` | **Preserved** | Removed | Removed | Removed |
| `containerd/` | `rubix-containerd` | **Preserved** | Removed | Removed | Removed |
| `network/` | `rubix-network` | **Preserved** | Removed | Removed | Removed |
| Foreign / Neighbor files | Host / Third-party | **Untouched** | **Untouched** | **Untouched** | **Untouched** |

### Ownership Guarantees:
- **Symlink Protection**: Symlinks within the state root are never traversed for recursive deletion; only the link inode is removed.
- **Parent Validation**: Cleanup refuses to execute if the data path is a symlink or contains parent directory traversal components (`..`).
- **Absence Verification**: Containers and services are stopped and their absence verified before disk directories are removed.
- **Lock Management**: `DatastoreLock` holds an advisory exclusive file flock (`.lock`) that is automatically released on process termination or dropped handles, preventing stale lock deadlock across crashes.

---

## 5. Diagnostics Retention and Secret Safety

When faults occur during startup, normal operation, or shutdown:
1. **Redaction Guarantee**: URLs, HTTP proxies, auth tokens, client private keys, and passwords are never serialized into logs, error messages, or `Debug` trait representations.
2. **Structured JSONL Event Format**: Lifecycle events emit structured JSON records with typed fields:
   - `component`: Component identifier string (e.g. `datastore`, `apiserver`, `portainer-agent`).
   - `state`: Current lifecycle state (`Starting`, `Ready`, `Stopping`, `Stopped`, `Failed`, `Degraded`).
   - `code`: Static, enumerated diagnostic code (e.g. `datastore_failure`, `unexpected_exit`, `startup_timeout`).
   - `elapsed_ms`: Wall-clock duration spent in state.
3. **Failure Isolation**: Optional components failing emit `component_failure` and `supervisor_degraded` events while the core API continues serving. Required components failing emit `fatal` stop causes, resulting in process exit code `1` to prevent unobserved degradation.

---

## 6. Verification and Execution Evidence

The qualification suite is implemented entirely in Rust in `crates/rubix-kube/tests/recovery_lifecycle.rs` in accordance with repository language policy.

### Test Execution Command:
```sh
cargo test --locked -p rubix-kube --test recovery_lifecycle
```

### Execution Results:
```text
running 7 tests
test test_historical_regression_matrix_is_complete_and_verified ... ok
test test_lifecycle_interruption_during_startup_leaves_clean_ownership ... ok
test test_crash_reboot_preserves_datastore_pki_and_ownership ... ok
test test_required_datastore_corruption_fails_closed_and_blocks_release ... ok
test test_ungraceful_daemon_kill_escalation_and_diagnostic_retention ... ok
test test_optional_service_failure_isolation_preserves_core_api ... ok
test test_node_ip_change_rotates_leaf_sans_and_updates_kubeconfig_both_formats ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s
```

All 7 qualification test suites pass with zero warnings under strict workspace Clippy (`-D warnings`).
