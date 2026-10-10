# Recovery and Lifecycle Fixture Coverage

**Issue:** #119 (E28.02)

**Gate:** C13 remains unqualified
**Environment:** disposable temporary directories and owned Rust subprocesses;
Linux/macOS unit-test environments, with process escalation requiring Linux. No live Kubernetes, host reboot, Docker, or
retained upstream executable qualification is established by these tests.

## Implemented checks

`crates/rubix-kube/tests/recovery_lifecycle.rs` exercises the experimental native
`DatastoreEngine`, in-process API adapter, PKI code and supervisor. The selected
production boundary continues to retain upstream kube-apiserver and Kine, with
loopback mTLS and a dedicated datastore CA, as defined by the
[component ADR](../../experiments/component-boundary/ADR.md) and
[compatibility contract](compatibility-contract.md). Native snapshot/WAL evidence
does not establish Kine SQLite recovery or replace that boundary.

- An owned Rust runtime subprocess acknowledges an API write. Its parent sends
  an abrupt kill without calling the stop channel, reaps it, reopens the native
  datastore, and compares the exact object value, UID and resourceVersion. CA
  certificate/key bytes remain unchanged. This is process-crash fixture coverage,
  not a physical reboot, power-loss or real Kine test.
- An owned Rust subprocess installs a TERM handler and deliberately keeps running.
  The real `OwnedProcessAdapter` sends TERM, escalates to KILL after the supervisor's
  configured 30-second grace period, and reports the observed signal, reaped leader
  and completed owner thread. Observation is bounded to 41 seconds. This checks a
  single owned fixture leader; it does not prove arbitrary descendants cannot escape.
  This test is Linux-only: an exploratory macOS run observed KILL/reaping but
  an unsuccessful process-group cleanup receipt. That platform behavior is not
  reclassified as successful cleanup or production qualification.
- A separate completed mock adapter checks fatal failure policy and diagnostic
  component identity. It provides no process-kill or escalation evidence.
- Native/in-process fixtures check node-IP leaf rotation, generated YAML/JSON
  kubeconfig decoding, optional mock-component degradation, cooperative startup
  cancellation/re-entry, and torn native WAL rejection versus explicit repair.
  They do not exercise actual SIGINT/SIGTERM delivery to a production node, real
  optional services, secret-bearing diagnostic output or measured decoder memory.

## Historical regression inventory

The inventory test checks that ten entries contain descriptions, source references
and historical target bounds. It does not mark those entries verified or derive a
release result from constants. These targets still require current-source evidence:

| Entries | Remaining evidence |
| --- | --- |
| REG-01, REG-05, REG-06 | Real CoreDNS readiness, LoadBalancer identity and Linux network-host behavior |
| REG-02, REG-09 | Selected executable TLS/SAN/client access and bounded credential decoding |
| REG-03, REG-04 | Retained API-server/Kine outage, TERM/KILL, acknowledged-update recovery and absence evidence |
| REG-07 | Real Kine SQLite/WAL corruption and supported repair boundaries |
| REG-08 | Disposable Linux init/Engine cleanup, owned paths, mounts and PV/config retention |
| REG-10 | Actual node signals during startup, owned process cleanup and lock re-entry |

The retained failed r2 outage-shutdown capture is not closed by a passing mock or
Rust fixture. C13 and dependent release qualification remain blocked until the
[acceptance matrix](acceptance-matrix.md) evidence requirements are satisfied.

## Production ownership versus fixture paths

Production cleanup selects `kine/db`, kubelet, network and managed containerd
root/state for reset. PKI, `local-path-storage`, configuration, registry inputs and
image archives are retained. Ordinary uninstall retains data; explicit purge
discards owned state and recovery receipts. `--keep-config` controls the selected
configuration/backup separately. Foreign files and external runtimes remain outside
ownership. The native fixture's `datastore/` directory is not the production Kine
path. Consult [rubixctl](../../crates/rubixctl/README.md) for implemented cleanup and
shared-lock/receipt behavior; these runtime tests do not qualify host deletion.

## Reproduction and limits

```sh
cargo test --locked -p rubix-kube --test suite recovery_lifecycle::
cargo clippy --locked -p rubix-kube --all-targets --all-features -- -D warnings
```

The two ignored child entry points are invoked only by parent tests with private
fixture directories. Parent-owned crash children are killed/reaped on error paths.
Passing tests provide fixture behavior at their tested revision. Qualification
still needs exact candidate/artifact hashes, current Linux environment/variant,
durable commands and raw logs, independent before/after cluster identities and PV
assertions, failures/skips, and the selected executable recovery evidence above.

## In-process recovery rehearsal harness and Criterion 5 candidate receipt capture

Issue #352 (E36.02) provides an in-process recovery rehearsal harness and candidate-bound receipt
generation for Criterion 5 (`criterion-05-lifecycle-and-storage.json`).
Live recovery qualification on disposable Linux infrastructure with retained upstream executables
and external processes remains pending.

```sh
# Run in-process candidate-bound lifecycle and recovery rehearsal
cargo run --locked -p rubix-dev --bin rubix-recovery-rehearsal -- capture --output /tmp/recovery-rehearsal

# Verify candidate-bound receipt against canonical schema and Criterion 5 rules
# (Fails closed on in-process rehearsal receipts; live_node execution on Linux required for qualification)
cargo run --locked -p rubix-dev --bin rubix-recovery-rehearsal -- verify /tmp/recovery-rehearsal
```

### Assertions evaluated

The rehearsal executes seven required assertions against in-process runtime instances:

1. `crash_restart_state_retention`: verifies state, namespace records, `resourceVersion`, and PKI preservation across runtime restart. (Abrupt SIGKILL qualification on live subprocesses remains pending.)
2. `bounded_escalation_and_cleanup`: verifies simulated daemon failure in `MockAdapter` triggers `StopCause::Fatal`, completes escalation within the shutdown budget, and permits subsequent clean re-entry. (Real process-group signal escalation remains pending.)
3. `datastore_outage_blocking_r2`: proves simulated datastore outage triggers immediate non-graceful `StopCause::Fatal` on the supervisor, preventing degraded execution and recovering cleanly upon restart.
4. `reboot_state_retention`: validates state retention across simulated cold re-initialization of runtime from disk. (Physical bare-metal host reboot skipped; live qualification pending.)
5. `wal_torn_write_fails_closed`: proves torn native WAL frames fail closed with diagnostic code `datastore_failure`, and recover cleanly when `dbWalRepair: true`.
6. `startup_interruption_safe_reentry`: verifies cooperative early stop during node startup cleanly releases file locks and allows immediate restart.
7. `ownership_cleanup_isolation`: verifies `rubixctl` cleanup plan logic removes owned runtime state (`kine/db`) while leaving foreign host files inside and outside the data directory untouched.

### Skips and platform bounds

- `physical_host_reboot`: Skipped in disposable test runners lacking bare-metal reboot capabilities; tested via simulated reboot and in-process restart.
- `linux_process_group_escalation`: Skipped in in-process rehearsal on all platforms because in-process execution with `MockAdapter` does not exercise Linux cgroup/process-group signaling; live qualification on Linux required.

Existing migration failure rehearsal continues to run when `rubix-recovery-rehearsal` is invoked with no arguments or with `migration` or `rehearse` subcommands.

