//! Integration tests for operational metrics HTTP serving, scrape parsing, and health checks.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rubix_config::{
    EnvironmentMode, ExplicitFlags, HostContext, ValidatedConfig, decode, resolve_layers,
};
use rubix_kube::lifecycle_sink::FlushPolicy;
use rubix_kube::metrics::{
    BuildInfoCollector, MetricsRegistry, MetricsServer, UptimeCollector, parse_scrape,
};
use rubix_kube::runtime::NodeRuntime;
use rubix_supervisor::{FailureKind, stop_channel};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

#[derive(Clone, Default)]
struct LogProbe(Arc<Mutex<Vec<u8>>>);

impl LogProbe {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().expect("lock").clone()).expect("utf8")
    }
}

impl Write for LogProbe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn test_config_metrics(
    dir: &Path,
    metrics_enabled: bool,
    metrics_bind_address: &str,
) -> ValidatedConfig {
    let yaml = format!(
        r#"
path: "{}"
network:
  node_ip: "127.0.0.1"
kubernetes:
  node_name: "test-node"
  apiServer:
    startupTimeoutSeconds: 5
logging:
  debug: false
metrics:
  enabled: {}
  bindAddress: "{}"
storage:
  local_path:
    enabled: false
"#,
        dir.display(),
        metrics_enabled,
        metrics_bind_address
    );
    resolve_layers(
        Some(decode(&yaml).expect("decode")),
        &BTreeMap::new(),
        &ExplicitFlags::default(),
        EnvironmentMode::Include,
        &HostContext {
            cpu_count: 4,
            architecture: "arm64".into(),
            detected_container_mode: false,
        },
    )
    .expect("resolve")
    .validated
}

async fn http_get(addr: SocketAddr, path: &str, headers: &[(&str, &str)]) -> (u16, String, String) {
    let mut stream = TcpStream::connect(addr).await.expect("connect to server");
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(k);
        req.push_str(": ");
        req.push_str(v);
        req.push_str("\r\n");
    }
    req.push_str("\r\n");
    stream
        .write_all(req.as_bytes())
        .await
        .expect("write request");

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read response");
    let resp = String::from_utf8_lossy(&buf).to_string();
    let (head, body) = resp.split_once("\r\n\r\n").unwrap_or(("", &resp));
    let status_line = head.lines().next().unwrap_or("");
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);
    (status_code, head.to_string(), body.to_string())
}

#[tokio::test]
async fn test_standalone_metrics_server_endpoints_and_shutdown() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind free port");
    let addr = listener.local_addr().expect("local addr");

    let registry = Arc::new(MetricsRegistry::new());
    registry.register(BuildInfoCollector::default());
    registry.register(UptimeCollector::with_start_time(1_700_000_000));

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let reg_clone = registry.clone();
    let server_task = tokio::spawn(async move {
        MetricsServer::run_with_listener(listener, reg_clone, shutdown_rx).await
    });

    // 1. Plain HTTP /healthz, /livez, /readyz checks
    let (status, _headers, body) = http_get(addr, "/healthz", &[]).await;
    assert_eq!(status, 200);
    assert_eq!(body, "ok\n");

    let (status, _headers, body) = http_get(addr, "/livez", &[]).await;
    assert_eq!(status, 200);
    assert_eq!(body, "ok\n");

    let (status, _headers, body) = http_get(addr, "/readyz", &[]).await;
    assert_eq!(status, 200);
    assert_eq!(body, "ok\n");

    // 2. Root HTML landing page
    let (status, headers, body) = http_get(addr, "/", &[]).await;
    assert_eq!(status, 200);
    assert!(headers.contains("text/html"));
    assert!(body.contains("kubesolo metrics"));
    assert!(body.contains("/metrics"));

    // 3. Prometheus plain exposition scrape without TLS
    let (status, headers, body) = http_get(addr, "/metrics", &[]).await;
    assert_eq!(status, 200);
    assert!(headers.contains("text/plain"));
    assert!(!body.contains("# EOF"));
    let scrape = parse_scrape(&body).expect("parse Prometheus scrape");
    assert_eq!(
        scrape.get_first_value("kubesolo_start_time_seconds"),
        Some(1_700_000_000.0)
    );
    let build_sample = scrape
        .get_sample("kubesolo_build_info", "rust_version", "1.97.1")
        .expect("build info present");
    assert!((build_sample.value - 1.0).abs() < f64::EPSILON);

    // 4. OpenMetrics exposition scrape with Accept header
    let (status, headers, body) = http_get(
        addr,
        "/metrics",
        &[("Accept", "application/openmetrics-text; version=1.0.0")],
    )
    .await;
    assert_eq!(status, 200);
    assert!(headers.contains("application/openmetrics-text"));
    assert!(body.ends_with("# EOF\n"));
    let om_scrape = parse_scrape(&body).expect("parse OpenMetrics scrape");
    assert_eq!(
        om_scrape.get_first_value("kubesolo_start_time_seconds"),
        Some(1_700_000_000.0)
    );

    // 5. 404 for unknown routes
    let (status, _headers, _body) = http_get(addr, "/nonexistent", &[]).await;
    assert_eq!(status, 404);

    // 6. Graceful shutdown
    let _ = shutdown_tx.send(true);
    let server_res = tokio::time::timeout(Duration::from_secs(3), server_task)
        .await
        .expect("shutdown timeout");
    assert!(server_res.is_ok());

    // Connecting after shutdown fails
    let conn_after = TcpStream::connect(addr).await;
    assert!(
        conn_after.is_err(),
        "Server should no longer accept connections"
    );
}

#[tokio::test]
async fn test_node_runtime_disabled_mode_opens_no_listener() {
    let temp = TempDir::new().unwrap();
    // Default metrics.enabled is false, default bind address is 127.0.0.1:9105
    let config = test_config_metrics(temp.path(), false, "127.0.0.1:9105");
    assert!(!config.config().metrics.enabled);
    assert_eq!(config.config().metrics.bind_address, "127.0.0.1:9105");

    let runtime = NodeRuntime::from_config(config).expect("assemble runtime");
    assert!(!runtime.is_metrics_enabled());
    assert_eq!(runtime.metrics_bind_address(), "127.0.0.1:9105");

    let client = runtime.client().expect("client present").clone();

    // Disabled mode registers no metrics listener
    let (stop_handle, stop_receiver) = stop_channel();
    let probe = LogProbe::default();
    let run_handle =
        tokio::spawn(runtime.run_with_sink(stop_receiver, probe, FlushPolicy::EachFrame));

    // Wait until apiserver is ready and serving
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if client.list_namespaces().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("apiserver becomes reachable");

    // Verify 127.0.0.1:9105 has NO listener open
    let conn = TcpStream::connect("127.0.0.1:9105").await;
    assert!(
        conn.is_err(),
        "Disabled metrics mode must open no listener on 127.0.0.1:9105"
    );

    stop_handle.stop();
    let (report, _, _) = run_handle.await.expect("runtime run");
    assert!(report.failures.is_empty());
}

#[tokio::test]
async fn test_node_runtime_metrics_serving_scrape_and_certificates_without_tls() {
    let temp = TempDir::new().unwrap();

    // Find an unused local port
    let dummy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = dummy.local_addr().unwrap();
    drop(dummy);

    let config = test_config_metrics(temp.path(), true, &addr.to_string());
    assert!(config.config().metrics.enabled);

    let runtime = NodeRuntime::from_config(config).expect("assemble runtime");
    assert!(runtime.is_metrics_enabled());
    assert!(runtime.metrics_registry().is_some());

    let client = runtime.client().expect("client present").clone();
    let (stop_handle, stop_receiver) = stop_channel();
    let probe = LogProbe::default();
    let run_handle =
        tokio::spawn(runtime.run_with_sink(stop_receiver, probe, FlushPolicy::EachFrame));

    // Wait until apiserver is ready and serving
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if client.list_namespaces().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("apiserver becomes reachable");

    // Wait for the metrics endpoint to become ready
    let mut ready = false;
    for _ in 0..50 {
        if let Ok(mut stream) = TcpStream::connect(addr).await {
            let req = format!("GET /healthz HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
            let _ = stream.write_all(req.as_bytes()).await;
            let mut buf = Vec::new();
            let _ = stream.read_to_end(&mut buf).await;
            if String::from_utf8_lossy(&buf).contains("ok") {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(ready, "Metrics endpoint did not become ready in time");

    // 1. Scrape metrics over plain HTTP (demonstrating that certificate metrics do not imply TLS)
    let (status, _headers, body) = http_get(addr, "/metrics", &[]).await;
    assert_eq!(status, 200);

    // 2. Parse scrape response
    let scrape = parse_scrape(&body).expect("parse Prometheus scrape");

    // Assert build info is present
    let build_sample = scrape
        .get_sample("kubesolo_build_info", "rust_version", "1.97.1")
        .expect("kubesolo_build_info");
    assert!((build_sample.value - 1.0).abs() < f64::EPSILON);

    // Assert certificate metrics are reported (e.g. apiserver, ca, kubelet)
    let apiserver_cert_valid = scrape
        .get_sample("kubesolo_certificate_valid", "name", "apiserver")
        .expect("apiserver cert valid metric");
    assert!((apiserver_cert_valid.value - 1.0).abs() < f64::EPSILON);

    let ca_cert_valid = scrape
        .get_sample("kubesolo_certificate_valid", "name", "ca")
        .expect("ca cert valid metric");
    assert!((ca_cert_valid.value - 1.0).abs() < f64::EPSILON);

    let apiserver_expiry = scrape
        .get_sample(
            "kubesolo_certificate_expiry_timestamp_seconds",
            "name",
            "apiserver",
        )
        .expect("apiserver cert expiry metric");
    assert!(apiserver_expiry.value > 1_700_000_000.0);

    // 3. Graceful shutdown
    stop_handle.stop();
    let (report, _, _) = run_handle.await.expect("runtime join");
    assert!(report.failures.is_empty());

    // Verify endpoint is closed
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(TcpStream::connect(addr).await.is_err());
}

#[tokio::test]
async fn test_node_runtime_metrics_bind_failure_is_observable_and_nonfatal() {
    let temp = TempDir::new().unwrap();

    // Intentionally occupy a port before starting runtime
    let blocker = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let blocked_addr = blocker.local_addr().unwrap();

    let config = test_config_metrics(temp.path(), true, &blocked_addr.to_string());

    // NodeRuntime assembly succeeds (bind is deferred to adapter run)
    let runtime =
        NodeRuntime::from_config(config).expect("assemble runtime succeeds despite conflict");

    let client = runtime.client().expect("client present").clone();
    let probe = LogProbe::default();
    let (stop_handle, stop_receiver) = stop_channel();

    let run_handle =
        tokio::spawn(runtime.run_with_sink(stop_receiver, probe.clone(), FlushPolicy::EachFrame));

    // Wait until degradation is recorded in the structured logs
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let text = probe.text();
            if text.contains("\"event\":\"component_failure\"")
                && text.contains("\"component\":\"metrics\"")
                && text.contains("\"detail_code\":\"metrics_bind_failed\"")
                && text.contains("\"event\":\"supervisor_degraded\"")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("metrics bind failure is logged as component failure and supervisor degraded");

    // Nonfatal: Assert KubernetesApiClient remains fully operational
    client
        .create_namespace("nonfatal-metrics-ns")
        .await
        .expect("core Kubernetes API server remains operational despite metrics bind failure");

    // Clean shutdown
    stop_handle.stop();
    let (report, _, _) = run_handle.await.expect("runtime join");

    // Observable in supervisor report
    assert!(
        report
            .failures
            .iter()
            .any(|f| f.component == "metrics"
                && f.kind == FailureKind::Adapter("metrics_bind_failed")),
        "Metrics component must be listed in failures with metrics_bind_failed"
    );

    // Free blocker
    drop(blocker);
}
