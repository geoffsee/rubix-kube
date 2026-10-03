# Operational Metrics & CPU Management Runbook

This runbook documents the operational HTTP metrics server, health check endpoints,
Prometheus/OpenMetrics integration, and Kubelet CPU manager policies in Rubix.

---

## 1. Operational Metrics HTTP Server

Rubix provides a lightweight, dedicated HTTP server exposing cluster health and performance
metrics for scraping by monitoring systems such as Prometheus, VictoriaMetrics, or Datadog.

### Enabling the Metrics Server
In `/etc/kubesolo/config.yaml`:
```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
metrics:
  enabled: true
  bindAddress: "127.0.0.1:9105"
```
Or via legacy environment variable / CLI flag:
```sh
export KUBESOLO_METRICS_SERVER="true"
export KUBESOLO_METRICS_BIND_ADDRESS="0.0.0.0:9105"
```

### Server Constraints & Resource Safeguards
- **Connection Backlog & Concurrency:** Capped at a maximum of `64` concurrent active connection tasks. Excess inbound connections wait in the kernel socket backlog.
- **Header Read Deadline:** 5-second timeout on initial request header receipt. Slowloris or trickling header connections are dropped.
- **Graceful Shutdown:** On cluster shutdown, active requests are drained within a bounded 5-second deadline before sockets are closed.
- **Degraded Fallback:** If the configured address/port cannot be bound (e.g. port conflict), the metrics adapter records an operational warning and allows the node to continue running.

---

## 2. HTTP Route Inventory & Content Negotiation

The metrics server exposes five standard routes:

| Route | HTTP Methods | Response Status | Purpose / Description |
| --- | --- | --- | --- |
| `/metrics` | `GET` | `200 OK` / `406 Not Acceptable` | Prometheus / OpenMetrics scrape endpoint. |
| `/healthz` | `GET` | `200 OK` | Liveness probe returning plaintext `ok\n`. |
| `/livez` | `GET` | `200 OK` | Alias for liveness check returning plaintext `ok\n`. |
| `/readyz` | `GET` | `200 OK` | Readiness check returning plaintext `ok\n`. |
| `/` | `GET` | `200 OK` | Simple HTML navigation index with links to `/metrics` and `/healthz`. |

Any non-`GET` request receives `405 Method Not Allowed`. Unknown routes receive `404 Not Found`.

### Prometheus & OpenMetrics Negotiation
The `/metrics` endpoint negotiates the exposition format based on the HTTP `Accept` header:
- **Default Format (No `Accept` header or `*/*`):** Prometheus text format (`text/plain; version=0.0.4; charset=utf-8`).
- **OpenMetrics Format:** Requested via `Accept: application/openmetrics-text; version=1.0.0`.
- **Quality Values & Exclusions:** `q=` values and parameter specificity are respected; explicit `q=0` excludes a format. If no supported format can satisfy the client's request, the server returns `406 Not Acceptable`.

Scraping example:
```sh
# Prometheus scrape
curl -s http://127.0.0.1:9105/metrics

# Explicit OpenMetrics scrape
curl -s -H "Accept: application/openmetrics-text; version=1.0.0" http://127.0.0.1:9105/metrics
```

---

## 3. Metric Series Reference

Rubix emits truthful, supervisor-connected operational gauges:

### A. Build & Process Uptime
- `kubesolo_build_info{version="...", commit="...", rust_version="...", arch="..."}`: Value `1.0`. Contains binary build metadata.
- `kubesolo_start_time_seconds`: Unix epoch timestamp at which the metrics server started.
- `kubesolo_uptime_seconds`: Elapsed time in seconds since node startup.

### B. Datastore Storage
- `kubesolo_kine_db_size_bytes`: Sum of the file size of the managed Kine SQLite database (`state.db`) and its active Write-Ahead Log (`state.db-wal`).

### C. Certificate Expiry & Validity
Monitors all internal control plane TLS certificates:
- `kubesolo_certificate_valid{name="<cert-name>"}`: `1.0` if readable and currently inside validity window; `0.0` if expired or unparseable.
- `kubesolo_certificate_expiry_timestamp_seconds{name="<cert-name>"}`: Unix timestamp when the certificate expires.

Tracked certificate label names:
`ca`, `apiserver`, `controller-manager`, `kubelet`, `admin`, `webhook`, `request-header-ca`, `request-header-client`, and optionally `d2k-server`, `d2k-client`.

### D. Component Health & Probes
Tracks supervisor lifecycle states for supervised components (`apiserver`, `controller`, `coredns`, `kine`, `kubelet`, `kubeproxy`, `runtime`, `webhook`):
- `kubesolo_component_up{component="<name>"}`: `1.0` if healthy and responding to probes; `0.0` if degraded, crashed, or stopped.
- `kubesolo_component_ready_timestamp_seconds{component="<name>"}`: Timestamp of the most recent transition to ready.
- `kubesolo_component_last_probe_timestamp_seconds{component="<name>"}`: Timestamp of the most recent probe execution.

---

## 4. Kubelet CPU Management Policies

For latency-critical, telecom, or high-throughput workloads, Kubernetes supports pinning container
processes to exclusive physical CPU cores via the Kubelet CPU Manager.

### Supported Policies

| Policy | Behavior | Best Suited For |
| --- | --- | --- |
| **`none`** (Default) | Standard Linux CFS scheduler. Workloads share CPU time slices across all cores without CPU pinning. | General microservices, web servers, dev clusters. |
| **`static`** | Allocates exclusive CPU cores to pods in the **Guaranteed QoS** class (i.e. CPU limits equal CPU requests, with integer values). | Database engines, DSP, trading engines, real-time networking. |

### Configuring the Static CPU Policy
In `/etc/kubesolo/config.yaml`:
```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
kubernetes:
  kubelet:
    cpuManager:
      policy: "static"
      # Reserve system CPU cores so host daemons don't interrupt pinned workloads
      reservedCPUs: "0-1"
      policyOptions:
        full-pcpus-only: "true"
        distribute-cpus-across-numa: "true"
```

### Policy Options Reference
- **`full-pcpus-only="true"`:** Allocates full physical CPU cores rather than individual SMT hyperthreads, preventing noisy neighbor contention on shared L1/L2 caches.
- **`distribute-cpus-across-numa="true"`:** Spreads allocated CPU cores evenly across available NUMA nodes.
- **`align-by-socket="true"`:** Aligns core allocation to physical CPU socket boundaries.

### Checkpoint Invalidation (`cpu_manager_state`)
Kubelet persists active core assignments to a checkpoint file:
```
/var/lib/kubelet/cpu_manager_state
```
If an operator alters `policy`, `policyOptions`, or `reservedCPUs` in `/etc/kubesolo/config.yaml`:
1. Rubix detects the change in CPU manager configuration.
2. Rubix automatically invalidates/removes the stale `cpu_manager_state` file prior to launching Kubelet.
3. Kubelet boots cleanly without throwing `SMTCpusInvalid` or `CPUAllocationConflict` errors.

> [!WARNING]
> **Container Mode Limitation:**
> The `static` CPU manager policy is **strictly unsupported** when running Rubix in Docker container
> mode (`--container-mode`). Docker container virtualization does not grant exclusive cpuset cgroup
> isolation to nested processes. Setting `policy: "static"` in container mode will fail preflight.
