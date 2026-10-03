//! HTTP server exposing metrics and health check routes.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http::{Method, Request, Response, StatusCode, header};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::watch;

use super::registry::MetricsRegistry;

/// Operational metrics and health HTTP server.
#[derive(Debug)]
pub struct MetricsServer {
    addr: SocketAddr,
    registry: Arc<MetricsRegistry>,
}

impl MetricsServer {
    #[must_use]
    pub fn new(addr: SocketAddr, registry: Arc<MetricsRegistry>) -> Self {
        Self { addr, registry }
    }

    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    #[must_use]
    pub fn registry(&self) -> &Arc<MetricsRegistry> {
        &self.registry
    }

    /// Runs the HTTP server on the provided listener until shutdown is signaled.
    pub async fn run_with_listener(
        listener: TcpListener,
        registry: Arc<MetricsRegistry>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> std::io::Result<()> {
        loop {
            tokio::select! {
                res = listener.accept() => {
                    let (stream, _) = match res {
                        Ok(conn) => conn,
                        Err(e) => {
                            // Transient accept errors do not stop the server
                            eprintln!("metrics server accept error: {e}");
                            continue;
                        }
                    };

                    let io = TokioIo::new(stream);
                    let reg = registry.clone();

                    tokio::spawn(async move {
                        let service = service_fn(move |req: Request<Incoming>| {
                            let reg = reg.clone();
                            async move {
                                Ok::<Response<Full<Bytes>>, Infallible>(handle_request(&req, &reg))
                            }
                        });

                        let _ = hyper::server::conn::http1::Builder::new()
                            .serve_connection(io, service)
                            .await;
                    });
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Handles incoming HTTP requests for metrics and health routes.
fn handle_request(req: &Request<Incoming>, registry: &MetricsRegistry) -> Response<Full<Bytes>> {
    let method = req.method();
    let path = req.uri().path();

    if method != Method::GET {
        return Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Full::new(Bytes::from("method not allowed\n")))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())));
    }

    match path {
        "/metrics" => {
            let accept = req
                .headers()
                .get(header::ACCEPT)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            if accept.contains("application/openmetrics-text") {
                let body = registry.render_openmetrics();
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        header::CONTENT_TYPE,
                        "application/openmetrics-text; version=1.0.0; charset=utf-8",
                    )
                    .body(Full::new(Bytes::from(body)))
                    .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
            } else {
                let body = registry.render_prometheus();
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        header::CONTENT_TYPE,
                        "text/plain; version=0.0.4; charset=utf-8",
                    )
                    .body(Full::new(Bytes::from(body)))
                    .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
            }
        },
        "/healthz" | "/livez" | "/readyz" => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Full::new(Bytes::from("ok\n")))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))),
        "/" => {
            let html = concat!(
                "<html><body><h1>kubesolo metrics</h1>",
                "<ul>",
                "<li><a href=\"/metrics\">/metrics</a></li>",
                "<li><a href=\"/healthz\">/healthz</a></li>",
                "</ul></body></html>\n"
            );
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Full::new(Bytes::from(html)))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
        },
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Full::new(Bytes::from("404 not found\n")))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))),
    }
}
