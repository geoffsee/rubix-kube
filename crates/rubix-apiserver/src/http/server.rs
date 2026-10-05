//! Accept loop for the API server HTTPS listener.

use std::convert::Infallible;
use std::io;
use std::net::SocketAddr;

use bytes::Bytes;
use http::{Method, Request, Response, StatusCode, header};
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio_rustls::TlsAcceptor;

use crate::client::KubernetesApiClient;
use crate::error::ApiserverError;
use crate::service::ApiserverService;

use super::dispatch::{self, Payload};
use super::route::{self, Route, wants_aggregated_discovery};
use super::tls::{self, PeerIdentity};

const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_CONNECTIONS: usize = 256;

pub(crate) async fn bind_listener(
    config: &crate::config::ApiserverConfig,
) -> Result<TcpListener, ApiserverError> {
    let preferred = SocketAddr::new(config.bind_address, config.secure_port);
    match TcpListener::bind(preferred).await {
        Ok(listener) => Ok(listener),
        Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
            let fallback = SocketAddr::new(config.bind_address, 0);
            TcpListener::bind(fallback)
                .await
                .map_err(ApiserverError::from)
        },
        Err(err) => Err(err.into()),
    }
}

pub(crate) async fn serve(
    listener: TcpListener,
    service: ApiserverService,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), ApiserverError> {
    let tls_config = tls::server_config(service.config())?;
    let acceptor = TlsAcceptor::from(tls_config);
    let mut connections = JoinSet::new();
    loop {
        if *shutdown.borrow() {
            break;
        }
        tokio::select! {
            biased;
            result = shutdown.changed() => {
                if result.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = connections.join_next(), if !connections.is_empty() => {}
            accepted = listener.accept(), if connections.len() < MAX_CONNECTIONS => {
                let (stream, _) = match accepted {
                    Ok(conn) => conn,
                    Err(err) => {
                        eprintln!("apiserver accept error: {err}");
                        continue;
                    }
                };
                let acceptor = acceptor.clone();
                let service = service.clone();
                connections.spawn(async move {
                    let Ok(tls) = acceptor.accept(stream).await else {
                        return;
                    };
                    let peer = tls
                        .get_ref()
                        .1
                        .peer_certificates()
                        .and_then(|chain| chain.first())
                        .and_then(|cert| tls::identity_from_der(cert.as_ref()));
                    let io = TokioIo::new(tls);
                    let hyper = hyper::server::conn::http1::Builder::new();
                    let connection = hyper.serve_connection(
                        io,
                        service_fn(move |request| {
                            let service = service.clone();
                            let peer = peer.clone();
                            async move {
                                Ok::<_, Infallible>(handle(request, &service, peer.as_ref()).await)
                            }
                        }),
                    );
                    let _ = connection.await;
                });
            }
        }
    }
    drop(listener);
    connections.abort_all();
    while connections.join_next().await.is_some() {}
    Ok(())
}

async fn handle(
    request: Request<Incoming>,
    service: &ApiserverService,
    peer: Option<&PeerIdentity>,
) -> Response<Full<Bytes>> {
    let path = request.uri().path().to_string();
    let route = route::parse(&path);
    if matches!(route, Route::Health) {
        return health(service, &path).await;
    }
    if matches!(route, Route::Version) {
        return json_response(
            StatusCode::OK,
            &json!({
                "major": "1",
                "minor": "35",
                "gitVersion": "v1.35.7",
                "platform": format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH)
            }),
        );
    }

    let query = request.uri().query().map(str::to_owned);
    let bearer = bearer_token(&request);
    let client = client_for(service, peer, bearer.as_deref());
    let accept = request
        .headers()
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let method = request.method().clone();
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    if !matches!(route, Route::Resource(_)) && method != Method::GET {
        return status_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "MethodNotAllowed",
            "method is not supported",
        );
    }
    let body = match Limited::new(request.into_body(), MAX_BODY_BYTES)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return status_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "RequestEntityTooLarge",
                "request body exceeds 1MiB",
            );
        },
    };

    match route {
        Route::Health | Route::Version => unreachable!("handled above"),
        Route::NotFound => status_response(
            StatusCode::NOT_FOUND,
            "NotFound",
            "the server could not find the requested resource",
        ),
        Route::ApiVersions => authenticated_document(&client, || api_versions(service)),
        Route::OpenApiV3Index => authenticated_document(&client, crate::openapi::v3_index),
        Route::OpenApiV3Core => authenticated_document(&client, crate::openapi::v3_core),
        Route::CoreDiscovery => aggregated_or(
            accept.as_deref(),
            || core_aggregated(&client),
            || client.discover_core(),
        ),
        Route::ApiGroupList => aggregated_or(
            accept.as_deref(),
            || groups_aggregated(&client),
            || client.discover_apis(),
        ),
        Route::GroupDiscovery { group, version } => {
            discovery_result(client.discover_group_resources(&group, &version))
        },
        Route::Resource(path) => {
            match dispatch::dispatch(
                service,
                &client,
                &method,
                &path,
                content_type.as_deref(),
                query.as_deref(),
                &body,
            )
            .await
            {
                Ok(outcome) => match outcome.body {
                    Payload::Json(body) => json_response(outcome.status, &body),
                    Payload::Text(body) => text_response(outcome.status, body),
                },
                Err(err) => error_response(&err),
            }
        },
    }
}

/// Serves a static document to any client that may read core discovery.
fn authenticated_document(
    client: &KubernetesApiClient,
    document: impl FnOnce() -> Value,
) -> Response<Full<Bytes>> {
    match client.discover_core() {
        Ok(_) => json_response(StatusCode::OK, &document()),
        Err(err) => error_response(&err),
    }
}

fn aggregated_or(
    accept: Option<&str>,
    aggregated: impl FnOnce() -> Result<Value, ApiserverError>,
    legacy: impl FnOnce() -> Result<Value, ApiserverError>,
) -> Response<Full<Bytes>> {
    if wants_aggregated_discovery(accept) {
        discovery_result(aggregated())
    } else {
        discovery_result(legacy())
    }
}

fn discovery_result(result: Result<Value, ApiserverError>) -> Response<Full<Bytes>> {
    match result {
        Ok(body) => json_response(StatusCode::OK, &body),
        Err(err) => error_response(&err),
    }
}

fn api_versions(service: &ApiserverService) -> Value {
    let server = service.bound_addr().map_or_else(
        || {
            format!(
                "{}:{}",
                service.config().bind_address,
                service.config().secure_port
            )
        },
        |addr| addr.to_string(),
    );
    json!({
        "kind": "APIVersions",
        "versions": ["v1"],
        "serverAddressByClientCIDRs": [{
            "clientCIDR": "0.0.0.0/0",
            "serverAddress": server
        }]
    })
}

fn core_aggregated(client: &KubernetesApiClient) -> Result<Value, ApiserverError> {
    let legacy = client.discover_core()?;
    Ok(discovery_list("", "v1", &legacy))
}

fn groups_aggregated(client: &KubernetesApiClient) -> Result<Value, ApiserverError> {
    let groups = client.discover_apis()?;
    let mut items = Vec::new();
    let Some(entries) = groups.get("groups").and_then(Value::as_array) else {
        return Ok(json!({
            "kind": "APIGroupDiscoveryList",
            "apiVersion": "apidiscovery.k8s.io/v2",
            "metadata": {},
            "items": items
        }));
    };
    for group in entries {
        let Some(name) = group.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(version) = group
            .pointer("/preferredVersion/version")
            .and_then(Value::as_str)
        else {
            continue;
        };
        let resources = client.discover_group_resources(name, version)?;
        items.push(group_item(name, version, &resources));
    }
    Ok(json!({
        "kind": "APIGroupDiscoveryList",
        "apiVersion": "apidiscovery.k8s.io/v2",
        "metadata": {},
        "items": items
    }))
}

fn discovery_list(group: &str, version: &str, resources: &Value) -> Value {
    json!({
        "kind": "APIGroupDiscoveryList",
        "apiVersion": "apidiscovery.k8s.io/v2",
        "metadata": {},
        "items": [group_item(group, version, resources)]
    })
}

fn group_item(group: &str, version: &str, resources: &Value) -> Value {
    let entries = resources
        .get("resources")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|resource| {
                    json!({
                        "resource": resource["name"],
                        "singularResource": resource["singularName"],
                        "shortNames": resource.get("shortNames").cloned().unwrap_or(json!([])),
                        "responseKind": {
                            "group": group,
                            "version": version,
                            "kind": resource["kind"]
                        },
                        "scope": if resource["namespaced"].as_bool() == Some(true) {
                            "Namespaced"
                        } else {
                            "Cluster"
                        },
                        "verbs": resource["verbs"]
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "metadata": { "name": group },
        "versions": [{
            "version": version,
            "resources": entries
        }]
    })
}

async fn health(service: &ApiserverService, path: &str) -> Response<Full<Bytes>> {
    if path == "/readyz" {
        match service.check_readiness().await {
            Ok(report) if report.is_healthy => text(StatusCode::OK, "ok"),
            Ok(_) | Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "not ready"),
        }
    } else if service.is_running() {
        text(StatusCode::OK, "ok")
    } else {
        text(StatusCode::INTERNAL_SERVER_ERROR, "not ready")
    }
}

fn client_for(
    service: &ApiserverService,
    peer: Option<&PeerIdentity>,
    bearer: Option<&str>,
) -> KubernetesApiClient {
    if let Some(peer) = peer {
        service.user_client(peer.username.clone(), peer.groups.clone())
    } else if let Some(token) = bearer {
        service.token_client(token)
    } else {
        service.anonymous_client()
    }
}

fn bearer_token(request: &Request<Incoming>) -> Option<String> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value.strip_prefix("Bearer ").map(str::to_owned)
}

fn error_response(err: &ApiserverError) -> Response<Full<Bytes>> {
    let (status, reason) = match &err {
        ApiserverError::Unauthenticated { .. } => (StatusCode::UNAUTHORIZED, "Unauthorized"),
        ApiserverError::Unauthorized { .. } | ApiserverError::AdmissionDenied { .. } => {
            (StatusCode::FORBIDDEN, "Forbidden")
        },
        ApiserverError::NotFound { .. } => (StatusCode::NOT_FOUND, "NotFound"),
        ApiserverError::Conflict { .. } => (StatusCode::CONFLICT, "AlreadyExists"),
        ApiserverError::BadRequest { .. } | ApiserverError::InvalidInput { .. } => {
            (StatusCode::BAD_REQUEST, "BadRequest")
        },
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "InternalError"),
    };
    status_response(status, reason, &err.to_string())
}

fn status_response(status: StatusCode, reason: &str, message: &str) -> Response<Full<Bytes>> {
    json_response(
        status,
        &json!({
            "kind": "Status",
            "apiVersion": "v1",
            "metadata": {},
            "status": if status.is_success() { "Success" } else { "Failure" },
            "message": message,
            "reason": reason,
            "code": status.as_u16()
        }),
    )
}

fn json_response(status: StatusCode, body: &Value) -> Response<Full<Bytes>> {
    let bytes = serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec());
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(bytes)))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::from_static(b"{\"kind\":\"Status\"}"))))
}

fn text_response(status: StatusCode, body: String) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from(body)))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::from_static(b"log unavailable"))))
}

fn text(status: StatusCode, body: &'static str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from_static(body.as_bytes())))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::from_static(body.as_bytes()))))
}
