use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http::{Request, Response};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::watch;

use super::error::WebhookError;
use super::handler::NodeSetterHandler;

#[derive(Debug)]
pub struct WebhookServer {
    addr: SocketAddr,
    handler: Arc<NodeSetterHandler>,
    shutdown_tx: watch::Sender<bool>,
}

impl WebhookServer {
    #[must_use]
    pub fn new(addr: SocketAddr, handler: Arc<NodeSetterHandler>) -> (Self, watch::Receiver<bool>) {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        (
            Self {
                addr,
                handler,
                shutdown_tx,
            },
            shutdown_rx,
        )
    }

    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    #[must_use]
    pub fn handler(&self) -> &Arc<NodeSetterHandler> {
        &self.handler
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    pub async fn run(
        addr: SocketAddr,
        handler: Arc<NodeSetterHandler>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Result<(), WebhookError> {
        let listener = TcpListener::bind(addr).await?;
        Self::run_with_listener(listener, handler, shutdown_rx).await
    }

    pub async fn run_with_listener(
        listener: TcpListener,
        handler: Arc<NodeSetterHandler>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<(), WebhookError> {
        loop {
            tokio::select! {
                res = listener.accept() => {
                    let (stream, _) = match res {
                        Ok(conn) => conn,
                        Err(e) => {
                            eprintln!("webhook accept error: {e}");
                            continue;
                        }
                    };

                    let io = TokioIo::new(stream);
                    let h = handler.clone();

                    tokio::spawn(async move {
                        let service = service_fn(move |req: Request<Incoming>| {
                            let h = h.clone();
                            async move {
                                let method = req.method().as_str().to_string();
                                let path = req.uri().path().to_string();

                                let body_bytes = match req.into_body().collect().await {
                                    Ok(collected) => collected.to_bytes().to_vec(),
                                    Err(_) => Vec::new(),
                                };

                                let http_resp = h.handle_http_request(&method, &path, &body_bytes).await;

                                let resp = Response::builder()
                                    .status(http_resp.status)
                                    .header("Content-Type", http_resp.content_type)
                                    .body(Full::new(Bytes::from(http_resp.body)))
                                    .unwrap_or_default();

                                Ok::<_, Infallible>(resp)
                            }
                        });

                        let _ = hyper::server::conn::http1::Builder::new()
                            .serve_connection(io, service)
                            .await;
                    });
                }
                _ = shutdown_rx.changed() => {
                    break;
                }
            }
        }

        Ok(())
    }
}
