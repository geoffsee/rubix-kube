//! HTTPS gateway: client certificates, discovery, and namespace CRUD.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use rustls::RootCertStore;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use rubix_apiserver::{ApiserverAdapter, ApiserverConfig, ApiserverService, KubernetesStorage};
use rubix_datastore::{DatastoreConfig, DatastoreEngine};
use rubix_pki::cluster::{ClusterPki, ClusterPkiConfig};

fn setup(dir: &TempDir) -> ApiserverConfig {
    let node_ip: IpAddr = "127.0.0.1".parse().unwrap();
    let pki_dir = dir.path().join("pki");
    std::fs::create_dir_all(&pki_dir).unwrap();
    ClusterPki::new(ClusterPkiConfig::new(
        pki_dir.clone(),
        "test-node".to_string(),
        node_ip,
    ))
    .reconcile()
    .expect("pki reconcile");
    ApiserverConfig::default_for_pki(&pki_dir, node_ip)
}

fn storage(dir: &TempDir) -> KubernetesStorage {
    let (engine, _) = DatastoreEngine::open(DatastoreConfig::new(dir.path().join("datastore")))
        .expect("datastore");
    KubernetesStorage::new(engine.client(), "/registry")
}

fn load_certs(path: &std::path::Path) -> Vec<CertificateDer<'static>> {
    let bytes = std::fs::read(path).unwrap();
    CertificateDer::pem_slice_iter(&bytes)
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn load_key(path: &std::path::Path) -> PrivateKeyDer<'static> {
    let bytes = std::fs::read(path).unwrap();
    PrivateKeyDer::from_pem_slice(&bytes).unwrap()
}

async fn request(
    addr: std::net::SocketAddr,
    pki: &std::path::Path,
    admin: bool,
    method: &str,
    path: &str,
    body: &str,
) -> (u16, String) {
    let mut roots = RootCertStore::empty();
    for cert in load_certs(&pki.join("ca.crt")) {
        roots.add(cert).unwrap();
    }
    let builder = rustls::ClientConfig::builder().with_root_certificates(roots);
    let config = if admin {
        builder
            .with_client_auth_cert(
                load_certs(&pki.join("admin.crt")),
                load_key(&pki.join("admin.key")),
            )
            .unwrap()
    } else {
        builder.with_no_client_auth()
    };
    let connector = TlsConnector::from(Arc::new(config));
    let tcp = TcpStream::connect(addr).await.unwrap();
    let server_name = ServerName::try_from("127.0.0.1").unwrap();
    let mut tls = connector.connect(server_name, tcp).await.unwrap();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    tls.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    tls.read_to_end(&mut response).await.unwrap();
    let text = String::from_utf8(response).unwrap();
    let status = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    (status, text)
}

#[tokio::test]
async fn admin_certificate_serves_namespace_crud_and_rejects_anonymous() {
    let dir = TempDir::new().unwrap();
    let config = setup(&dir);
    let service = ApiserverService::new(config, storage(&dir));
    let registration = ApiserverAdapter::registration(
        "apiserver",
        service.clone(),
        Vec::new(),
        Duration::from_secs(10),
    );
    let supervisor = rubix_supervisor::Supervisor::new(vec![registration]).unwrap();
    let (stop_handle, stop_receiver) = rubix_supervisor::stop_channel();
    let supervisor = tokio::spawn(async move { supervisor.run(stop_receiver).await });

    let addr = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(addr) = service.bound_addr() {
                return addr;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("listener bound");

    let pki = dir.path().join("pki");

    let (status, _) = request(addr, &pki, false, "GET", "/readyz", "").await;
    assert_eq!(status, 200, "readyz is unauthenticated");

    let (status, body) = request(addr, &pki, false, "GET", "/api/v1/namespaces", "").await;
    assert_eq!(status, 401, "{body}");

    let (status, body) = request(addr, &pki, true, "GET", "/version", "").await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("v1.35.7"), "{body}");

    let (status, body) = request(
        addr,
        &pki,
        true,
        "POST",
        "/api/v1/namespaces",
        r#"{"apiVersion":"v1","kind":"Namespace","metadata":{"name":"demo"}}"#,
    )
    .await;
    assert_eq!(status, 201, "{body}");
    assert!(body.contains("\"demo\""), "{body}");

    let (status, body) = request(addr, &pki, true, "GET", "/api/v1/namespaces/demo", "").await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("Active"), "{body}");

    let (status, body) = request(addr, &pki, true, "DELETE", "/api/v1/namespaces/demo", "").await;
    assert_eq!(status, 200, "{body}");

    let (status, _) = request(addr, &pki, true, "GET", "/api/v1/namespaces/demo", "").await;
    assert_eq!(status, 404);

    stop_handle.stop();
    let report = supervisor.await.unwrap();
    assert!(matches!(
        report.cause,
        rubix_supervisor::StopCause::Requested
    ));
    assert!(!service.is_running());
}

#[derive(Debug)]
struct StaticLogReader;

#[async_trait::async_trait]
impl rubix_apiserver::PodLogReader for StaticLogReader {
    async fn read_pod_log(
        &self,
        pod: &serde_json::Value,
        options: &rubix_apiserver::PodLogOptions,
    ) -> Result<String, rubix_apiserver::ApiserverError> {
        Ok(format!(
            "{}/{} tail={:?}\n",
            pod["metadata"]["name"].as_str().unwrap_or(""),
            options.container.as_deref().unwrap_or("-"),
            options.tail_lines
        ))
    }
}

#[tokio::test]
async fn pod_log_subresource_and_openapi_serve_kubectl() {
    let dir = TempDir::new().unwrap();
    let config = setup(&dir);
    let service = ApiserverService::new(config, storage(&dir));
    service.set_pod_log_reader(Arc::new(StaticLogReader));
    let registration = ApiserverAdapter::registration(
        "apiserver",
        service.clone(),
        Vec::new(),
        Duration::from_secs(10),
    );
    let supervisor = rubix_supervisor::Supervisor::new(vec![registration]).unwrap();
    let (stop_handle, stop_receiver) = rubix_supervisor::stop_channel();
    let supervisor = tokio::spawn(async move { supervisor.run(stop_receiver).await });
    let addr = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(addr) = service.bound_addr() {
                return addr;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("listener bound");
    let pki = dir.path().join("pki");

    // kubectl validation handshake: discovery of the core OpenAPI document.
    let (status, body) = request(addr, &pki, true, "GET", "/openapi/v3", "").await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("/openapi/v3/api/v1?hash="), "{body}");
    let (status, body) = request(addr, &pki, true, "GET", "/openapi/v3/api/v1", "").await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("fieldValidation"), "{body}");
    let (status, _) = request(addr, &pki, false, "GET", "/openapi/v3", "").await;
    assert_eq!(status, 401, "openapi requires an authenticated client");

    // Pod log subresource is read from the registered kubelet, as text.
    let (status, body) = request(
        addr,
        &pki,
        true,
        "POST",
        "/api/v1/namespaces/default/pods",
        r#"{"apiVersion":"v1","kind":"Pod","metadata":{"name":"hello"},"spec":{"containers":[{"name":"hello","image":"localhost/rubix-hello:latest"}]}}"#,
    )
    .await;
    assert_eq!(status, 201, "{body}");
    let (status, body) = request(
        addr,
        &pki,
        true,
        "GET",
        "/api/v1/namespaces/default/pods/hello/log?container=hello&tailLines=5",
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("content-type: text/plain"), "{body}");
    assert!(body.ends_with("hello/hello tail=Some(5)\n"), "{body}");
    let (status, body) = request(
        addr,
        &pki,
        true,
        "GET",
        "/api/v1/namespaces/default/pods/missing/log",
        "",
    )
    .await;
    assert_eq!(status, 404, "{body}");
    let (status, body) = request(
        addr,
        &pki,
        true,
        "GET",
        "/api/v1/namespaces/default/pods/hello/status",
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"kind\":\"Pod\""), "{body}");
    let (status, _) = request(addr, &pki, true, "GET", "/api/v1/pods", "").await;
    assert_eq!(status, 200);

    stop_handle.stop();
    supervisor.await.unwrap();
}

/// Boots a supervised gateway on an ephemeral port and returns how to reach and stop it.
async fn spawn_gateway(
    dir: &TempDir,
) -> (
    std::net::SocketAddr,
    rubix_supervisor::StopHandle,
    tokio::task::JoinHandle<rubix_supervisor::SupervisorReport>,
) {
    let config = setup(dir);
    let service = ApiserverService::new(config, storage(dir));
    let registration = ApiserverAdapter::registration(
        "apiserver",
        service.clone(),
        Vec::new(),
        Duration::from_secs(10),
    );
    let supervisor = rubix_supervisor::Supervisor::new(vec![registration]).unwrap();
    let (stop_handle, stop_receiver) = rubix_supervisor::stop_channel();
    let supervisor = tokio::spawn(async move { supervisor.run(stop_receiver).await });
    let addr = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(addr) = service.bound_addr() {
                return addr;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("listener bound");
    (addr, stop_handle, supervisor)
}

#[tokio::test]
async fn watch_streams_events_and_unacknowledged_pods_delete_at_once() {
    let dir = TempDir::new().unwrap();
    let (addr, stop_handle, supervisor) = spawn_gateway(&dir).await;
    let pki = dir.path().join("pki");
    let (status, body) = request(
        addr,
        &pki,
        true,
        "POST",
        "/api/v1/namespaces/default/pods",
        r#"{"apiVersion":"v1","kind":"Pod","metadata":{"name":"hello"},"spec":{"containers":[{"name":"hello","image":"localhost/rubix-hello:latest"}]}}"#,
    )
    .await;
    assert_eq!(status, 201, "{body}");

    // Watch streams the current collection as ADDED events, then live changes,
    // until timeoutSeconds elapses. kubectl waits for deletions this way.
    let (status, body) = request(
        addr,
        &pki,
        true,
        "GET",
        "/api/v1/namespaces/default/pods?watch=true&timeoutSeconds=1",
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("transfer-encoding: chunked"), "{body}");
    assert!(body.contains("\"type\":\"ADDED\""), "{body}");
    assert!(body.contains("\"name\":\"hello\""), "{body}");
    let (status, body) = request(
        addr,
        &pki,
        true,
        "GET",
        "/api/v1/namespaces/default/pods?watch=true&timeoutSeconds=1&fieldSelector=metadata.name%3Dgone&resourceVersion=1",
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"type\":\"DELETED\""), "{body}");
    assert!(!body.contains("\"name\":\"hello\""), "{body}");

    // kubectl's informers ask for the whole collection and a bookmark that
    // marks the end of the initial events.
    let (status, body) = request(
        addr,
        &pki,
        true,
        "GET",
        "/api/v1/namespaces/default/pods?watch=true&timeoutSeconds=1&sendInitialEvents=true&resourceVersionMatch=NotOlderThan&allowWatchBookmarks=true&fieldSelector=metadata.name%3Dhello",
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let added = body.find("\"type\":\"ADDED\"").expect("ADDED event");
    let bookmark = body.find("\"type\":\"BOOKMARK\"").expect("BOOKMARK event");
    assert!(added < bookmark, "{body}");
    assert!(
        body.contains("\"k8s.io/initial-events-end\":\"true\""),
        "{body}"
    );
    assert!(
        body.contains("\"kind\":\"Pod\",\"apiVersion\":\"v1\"")
            || body.contains("\"apiVersion\":\"v1\",\"kind\":\"Pod\""),
        "{body}"
    );

    // A pod no kubelet has acknowledged is deleted at once.
    let (status, body) = request(
        addr,
        &pki,
        true,
        "DELETE",
        "/api/v1/namespaces/default/pods/hello",
        r#"{"kind":"DeleteOptions","apiVersion":"v1","propagationPolicy":"Background"}"#,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"status\":\"Success\""), "{body}");
    let (status, _) = request(
        addr,
        &pki,
        true,
        "GET",
        "/api/v1/namespaces/default/pods/hello",
        "",
    )
    .await;
    assert_eq!(status, 404);

    stop_handle.stop();
    supervisor.await.unwrap();
}
