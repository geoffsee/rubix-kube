# Operational metrics and CPU management

The metrics adapter and Kubelet configuration renderer are implemented boundaries,
not proof of live retained-node workload or CPU isolation qualification. Read the
[compatibility contract](../architecture/compatibility-contract.md) and
[acceptance matrix](../architecture/acceptance-matrix.md).

## Metrics listener and routes

Metrics are disabled by default. Enable explicitly in configuration:

```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
metrics:
  enabled: true
  bindAddress: "127.0.0.1:9105"
```

The node inputs `KUBESOLO_METRICS_SERVER` and `KUBESOLO_METRICS_BIND_ADDRESS`
(or corresponding metrics-server/metrics-bind-address flags) preserve ordinary
configuration precedence. Binding 0.0.0.0 exposes the endpoint beyond loopback;
choose deliberate network access controls rather than assuming authentication.

The listener bounds 64 connection tasks, imposes a five-second header deadline,
and drains/cancels owned connections within a five-second shutdown budget.
A listener bind failure records a degraded warning; other node operations may continue.

| Route | GET result | Meaning |
| --- | --- | --- |
| `/metrics` | 200 or 406 Not Acceptable | Prometheus/OpenMetrics exposition. |
| `/healthz`, `/livez`, `/readyz` | 200 with `ok` | Metrics HTTP handler response; not Kubernetes node or workload readiness. |
| `/` | 200 | Metrics navigation page. |

Unknown routes return 404; non-GET methods return 405. Prometheus text is the
default (`text/plain; version=0.0.4`); supported OpenMetrics accepts
`application/openmetrics-text; version=1.0.0`. Quality/version parameters and q=0
exclusions apply. Unsupported acceptable formats yield 406. Inspect endpoint and
component diagnostics independently; a successful `/readyz` scrape is not a
production-readiness or C14 qualification check.

## Metric series

- `kubesolo_build_info`: version, commit, rust_version and arch labels; unavailable
  build metadata can explicitly be unknown.
- `kubesolo_start_time_seconds`, `kubesolo_uptime_seconds`: metrics endpoint start
  and elapsed time, not a measurement of cluster/workload availability.
- `kubesolo_kine_db_size_bytes`: observed database/WAL sizes; not a SQLite integrity
  or WAL-checkpoint assertion.
- `kubesolo_certificate_valid` and `kubesolo_certificate_expiry_timestamp_seconds`:
  configured certificate observations. File/validity observations are not proof
  of live client authentication or all application-specific PKI semantics.
- `kubesolo_component_up`, `kubesolo_component_ready_timestamp_seconds` and
  `kubesolo_component_last_probe_timestamp_seconds`: supervisor-connected component
  observations. Interpret degraded/stopped state and actual probe scope, not a
  fabricated all-components-ready total.

Listener tests and scrape samples do not qualify actual Linux retained components.
Protect logs/configuration and avoid publishing private certificate/key material.

## Kubelet CPU manager

The default policy is `none`. `static` settings render upstream Kubelet CPU-manager
configuration and require valid host CPU reservations and non-container mode.
A partial configuration example (reserve CPU 0 only on a host with additional
available CPUs) is:

```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
kubernetes:
  kubelet:
    cpuManager:
      policy: static
      reservedCPUs: "0"
```

`policyOptions` values are strings. The experimental Rust CPU manager allocates
the first available logical CPUs; it does not enforce `full-pcpus-only`, NUMA
distribution or socket alignment. `align-by-socket` is rejected by the current
configuration validator. Rendering accepted option strings for the retained
upstream Kubelet does not qualify those policies: verify support in its pinned
version, effective topology, reservations and pod QoS/resources independently.
A YAML example is not proof of real exclusive-core assignment or latency results.
Static policy is rejected in container mode by configuration validation.

The Kubelet configuration adapter compares effective CPU-manager settings when
writing rendered configuration and can invalidate `cpu_manager_state` under its
configured Kubelet root. A previous-configuration read failure or stale-checkpoint
removal failure aborts configuration replacement and Rust service startup before
in-memory allocations are cleared. A missing previous configuration or checkpoint
is permitted. This is not permission to delete a live checkpoint to
force an update. Stop/quiesce the owning Kubelet and rehearse policy changes while
preserving workload and CPU ownership. Do not confuse the experimental Rust CPU
manager with replacement of the selected upstream Kubelet executable.
