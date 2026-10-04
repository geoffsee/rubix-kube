# Rubix v0.1.0 candidate release notes

**Status: UNQUALIFIED_FIXTURE_ONLY. C13/C14/C16/C17 remain pending.**

Rubix develops a single-node distribution supervising retained kube-apiserver, kube-controller-manager, kubelet, kube-proxy, Kine and containerd executables. Runtime shims, CNI programs, snapshotter helpers and default addon images remain part of distribution accounting. These diagnostics do not qualify an installed retained node, authentication, DNS/storage, container support, or platform behavior.

## Migration and recovery boundaries

The modeled starting versions are v1.1.8, v1.2.0, v1.3.0 and v1.3.1 through v1.3.3. No live Go-to-Rust migration or downtime was measured. There is no qualified production cutover or promised downtime window. Preserve verified configuration, PKI, Kine SQLite/WAL and persistent-volume backups. Experimental RUBXSNP1 snapshots do not replace or adopt a Kine SQLite database. The selected production API-server/Kine contract requires loopback mTLS with a dedicated datastore CA and client identity; plaintext spike evidence cannot qualify that transport.

## Compatibility, deprecations and deviations

Preserve kubesolo.io/v1alpha1, KUBESOLO_* inputs, /etc/kubesolo and /var/lib/kubesolo defaults. KUBESOLO_FULL/--full are deprecated no-ops. NodeSetter and YAML/JSON kubeconfig parser fixtures are implementation diagnostics, not proof of live authentication or service behavior. D01 through D11 are defined by the [compatibility contract](../architecture/compatibility-contract.md); they are deliberate design obligations, not qualified release capabilities.

## Performance

The [authoritative performance policy](../architecture/performance-rebaseline-policy.md) owns the twelve arithmetic gates:

| Gate | Default pass condition |
| --- | --- |
| Boot-to-API latency, p95 | Candidate <= reference * 1.10 |
| Node Ready latency, p95 | Candidate <= reference * 1.10 |
| First Pod latency, preloaded image, p95 | Candidate <= reference * 1.10 |
| First Pod latency, cold image, p95 | Candidate <= reference * 1.10 |
| Idle summed PSS, median of run p95 values | Candidate <= reference * 1.10 |
| Idle cgroup memory, median of run p95 values | Candidate <= reference * 1.10 |
| Compressed distribution archive bytes | Candidate <= reference * 1.10 |
| Extracted executable/helper bytes | Candidate <= reference * 1.10 |
| Default image payload bytes | Candidate <= reference * 1.10 |
| Pod density, median Ready replicas | Candidate >= reference * 0.90 |
| Sustained memory growth | Final/initial settled idle median <= 1.10; >=24h; zero OOMs, crashes and unexplained failures |
| Shutdown and cleanup | Graceful p95/maximum <=30 seconds; escalation p95/maximum <=35 seconds; zero surviving owned or killed unrelated processes |

System-workload readiness, per-component memory and pod-cycle diagnostics are not additional implemented contract gates. Synthetic arithmetic passing does not qualify live performance.

## Certification

Synthetic in-process fixtures only. C13/E28 and the selected upstream conformance suite remain unqualified. DO NOT claim official CNCF Certified Kubernetes qualification or multi-node certification.
