use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rubix_apiserver::{ApiserverService, KubernetesApiClient};

use crate::config::KubeletConfigOptions;
use crate::error::KubeletError;
use crate::health::KubeletHealthReport;
use crate::registration::NodeRegistration;
use crate::workload::{PodReconciler, RuntimeProvider};

/// Kubelet service orchestrating node lifecycle, registration, and workload execution.
#[derive(Clone, Debug)]
pub struct KubeletService {
    options: KubeletConfigOptions,
    apiserver: Arc<ApiserverService>,
    runtime: Arc<dyn RuntimeProvider>,
    running: Arc<AtomicBool>,
    registration: NodeRegistration,
    reconciler: PodReconciler,
}

impl KubeletService {
    pub fn new(
        options: KubeletConfigOptions,
        apiserver: Arc<ApiserverService>,
        runtime: Arc<dyn RuntimeProvider>,
    ) -> Self {
        let client = Arc::new(apiserver.node_client(&options.node_name));
        let registration = NodeRegistration::new(
            client.clone(),
            options.clone(),
            format!("{}://v1.35.7", runtime.provider_name()),
        );
        let reconciler = PodReconciler::new(
            client,
            runtime.clone(),
            &options.node_name,
            &options.node_ip,
        );

        Self {
            options,
            apiserver,
            runtime,
            running: Arc::new(AtomicBool::new(false)),
            registration,
            reconciler,
        }
    }

    #[must_use]
    pub fn options(&self) -> &KubeletConfigOptions {
        &self.options
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn client(&self) -> KubernetesApiClient {
        self.apiserver.node_client(&self.options.node_name)
    }

    #[must_use]
    pub fn reconciler(&self) -> &PodReconciler {
        &self.reconciler
    }

    #[must_use]
    pub fn registration(&self) -> &NodeRegistration {
        &self.registration
    }

    /// Validates all credentials, CRI runtime endpoints, and API server reachability.
    ///
    /// Identifies the responsible component specifically:
    /// - `KubeletError::MissingCredential` / `AuthenticationFailed` for PKI/auth failures
    /// - `KubeletError::RuntimeUnavailable` for CRI endpoint connectivity failures
    /// - `KubeletError::ApiserverUnavailable` for API server outages
    pub async fn check_prerequisites(&self) -> Result<(), KubeletError> {
        // 1. Verify credential file paths
        if !self.options.kubeconfig.exists() {
            return Err(KubeletError::MissingCredential {
                path: self.options.kubeconfig.clone(),
                component: "kubeconfig",
            });
        }

        if !self.options.ca_file.exists() {
            return Err(KubeletError::MissingCredential {
                path: self.options.ca_file.clone(),
                component: "root-ca",
            });
        }

        if !self.options.cert_file.exists() {
            return Err(KubeletError::MissingCredential {
                path: self.options.cert_file.clone(),
                component: "client-cert",
            });
        }

        if !self.options.key_file.exists() {
            return Err(KubeletError::MissingCredential {
                path: self.options.key_file.clone(),
                component: "client-key",
            });
        }

        // Validate kubeconfig content and user identity
        let kubeconfig_content =
            std::fs::read_to_string(&self.options.kubeconfig).map_err(|e| {
                KubeletError::InvalidConfiguration {
                    field: "kubeconfig".to_string(),
                    reason: format!("failed to read kubeconfig: {e}"),
                }
            })?;

        self.validate_kubeconfig_client_cert(&kubeconfig_content)?;

        // 2. Validate CRI runtime endpoint
        let endpoint_str = &self.options.runtime_endpoint;
        let socket_path = endpoint_str.trim_start_matches("unix://");
        let path = Path::new(socket_path);
        if self.runtime.requires_socket() && !path.exists() {
            return Err(KubeletError::RuntimeUnavailable {
                endpoint: endpoint_str.clone(),
                reason: format!("CRI runtime socket does not exist at {}", path.display()),
            });
        }

        // 3. Validate API server communication and node authorization
        let client = self.client();
        client
            .list_nodes()
            .await
            .map_err(|e| KubeletError::ApiserverUnavailable {
                reason: format!("failed to list nodes with node identity: {e}"),
            })?;

        // 4. Validate upstream resource defaults preservation
        self.options.validate_upstream_resource_defaults()?;

        Ok(())
    }

    /// Starts the Kubelet service, writes its config, and registers the node.
    pub async fn start(&self) -> Result<(), KubeletError> {
        self.check_prerequisites().await?;
        self.options.write_kubelet_config_file()?;
        self.registration.register_or_update().await?;
        self.registration.update_lease().await?;
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Checks the health and readiness of the Kubelet service.
    pub async fn check_readiness(&self) -> Result<KubeletHealthReport, KubeletError> {
        if !self.is_running() {
            return Ok(KubeletHealthReport::new_unhealthy(
                &self.options.node_name,
                "kubelet service is not running",
            ));
        }

        // Verify API server connectivity
        let client = self.client();
        let node_doc = match client.get_node(&self.options.node_name).await {
            Ok(doc) => doc,
            Err(e) => {
                return Ok(KubeletHealthReport::new_unhealthy(
                    &self.options.node_name,
                    format!("API server communication failed: {e}"),
                ));
            },
        };

        // Verify node Ready condition
        let mut is_ready = false;
        if let Some(conditions) = node_doc
            .get("status")
            .and_then(|s| s.get("conditions"))
            .and_then(serde_json::Value::as_array)
        {
            for cond in conditions {
                if cond.get("type").and_then(serde_json::Value::as_str) == Some("Ready")
                    && cond.get("status").and_then(serde_json::Value::as_str) == Some("True")
                {
                    is_ready = true;
                    break;
                }
            }
        }

        Ok(KubeletHealthReport::new_healthy(
            &self.options.node_name,
            is_ready,
            &self.options.runtime_endpoint,
            self.runtime.provider_name(),
            &self.options.cgroup_driver,
            0,
        ))
    }

    fn validate_kubeconfig_client_cert(
        &self,
        kubeconfig_content: &str,
    ) -> Result<(), KubeletError> {
        let expected_node_user = format!("system:node:{}", self.options.node_name);
        let cert_pem = if let Some(idx) = kubeconfig_content.find("client-certificate-data:") {
            let after = &kubeconfig_content[idx + "client-certificate-data:".len()..];
            let b64_token = after.split_whitespace().next().unwrap_or("");
            let decoded = rubix_pki::base64_decode(b64_token).map_err(|_| {
                KubeletError::AuthenticationFailed {
                    reason: "failed to decode client-certificate-data in kubeconfig".to_string(),
                }
            })?;
            String::from_utf8(decoded).map_err(|_| KubeletError::AuthenticationFailed {
                reason: "client-certificate-data is not valid UTF-8 PEM".to_string(),
            })?
        } else if let Some(idx) = kubeconfig_content.find("client-certificate:") {
            let after = &kubeconfig_content[idx + "client-certificate:".len()..];
            let path_str = after.lines().next().unwrap_or("").trim();
            let cert_path = Path::new(path_str);
            let resolved_path = if cert_path.is_absolute() {
                cert_path.to_path_buf()
            } else {
                self.options
                    .kubeconfig
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(cert_path)
            };
            std::fs::read_to_string(&resolved_path).map_err(|e| {
                KubeletError::AuthenticationFailed {
                    reason: format!(
                        "failed to read client-certificate file {}: {e}",
                        resolved_path.display()
                    ),
                }
            })?
        } else {
            return Err(KubeletError::AuthenticationFailed {
                reason: "kubeconfig missing client certificate credentials".to_string(),
            });
        };

        let ca_pem = std::fs::read_to_string(&self.options.ca_file).map_err(|e| {
            KubeletError::InvalidConfiguration {
                field: "ca_file".to_string(),
                reason: format!("failed to read ca_file: {e}"),
            }
        })?;

        let identity = rubix_pki::verify_certificate_chain(&cert_pem, &ca_pem).map_err(|e| {
            KubeletError::AuthenticationFailed {
                reason: format!("kubeconfig client certificate verification failed: {e}"),
            }
        })?;

        if identity.common_name != expected_node_user {
            return Err(KubeletError::AuthenticationFailed {
                reason: format!(
                    "kubeconfig client certificate CN '{}' does not match expected '{}'",
                    identity.common_name, expected_node_user
                ),
            });
        }

        Ok(())
    }

    /// Stops the Kubelet service.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}
