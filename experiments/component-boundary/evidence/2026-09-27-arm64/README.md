# Component-boundary evidence, 2026-09-27

Command: `python3 experiments/component-boundary/run.py --output <new-directory>`.
Host: Darwin arm64; disposable Linux arm64 Docker Desktop container, kernel
`7.0.12-linuxkit`, 1 GiB limit, unprivileged UID, dropped capabilities, no network,
mounts or published ports. Exact image and container identities are recorded per run.
Release artifact hashes are in each result. Logs contain test diagnostics, no private keys.
`sha256.json` covers the collected JSON and compressed logs.

| Run | Result | Interpretation |
| --- | --- | --- |
| r1 | Failed API readiness | Test CA lacked strict key-usage extensions; loopback advertise address also produced invalid Service endpoints. Fixed fixture credentials and used a documentation-only advertise address. TLS verification remained enabled. |
| r2 | Failed graceful shutdown after injected datastore death | Authenticated CRUD, clean restart and SQLite integrity passed. API process required SIGKILL after Kine became unavailable; no owned processes remained. This is a retained lifecycle limitation, not a passing crash-recovery result. |
| r3 | Passed all 20 assertions and runner cleanup | Restored Kine under the same API PID, recovered the acknowledged update, then verified deletion across restart and normal API-before-Kine shutdown. Exact source hashes match the committed fixture. |

R3 API readiness durations were 2.233, 1.728 and 2.234 seconds; datastore recovery
readiness took 8.078 seconds. Sampled summed component RSS was 301,116–307,732 KiB.
There were two retained component processes plus the Python driver. These are isolated
point samples, not steady-state whole-distribution measurements or E29 budget evidence.
The driver, other Kubernetes components, runtime, shims and workloads are excluded from
that component RSS number. There is no claim of Kubernetes conformance or Rust lifecycle
implementation. Only Linux arm64 was exercised.

R1/R2 precede the source-hash reporting addition; image identities and failed results
are preserved, but their exact uncommitted source snapshots were not retained. R3's
`runner-result.json` records every fixture source hash and `result.json` independently
records the executed `spike.py` hash. Subsequent documentation/evidence additions do not
change Docker inputs. Production supervision must test bounded escalation when the
API cannot exit gracefully with storage unavailable; E04/E08/E28 own those regressions.
