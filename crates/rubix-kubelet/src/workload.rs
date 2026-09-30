use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rubix_apiserver::KubernetesApiClient;

use crate::error::KubeletError;

/// Abstract interface for container runtime providers executing workloads.
#[async_trait]
pub trait RuntimeProvider: std::fmt::Debug + Send + Sync {
    /// Identifier for the runtime provider (e.g. "containerd", "cri-o").
    fn provider_name(&self) -> &str;

    /// Whether this provider requires a live unix domain socket path to exist on the host filesystem.
    fn requires_socket(&self) -> bool {
        false
    }

    /// Runs a pod sandbox and starts its containers.
    async fn run_pod(&self, pod: &Value) -> Result<String, KubeletError>;

    /// Stops a running pod sandbox and its containers.
    async fn stop_pod(&self, pod_id: &str) -> Result<(), KubeletError>;

    /// Queries the status of an active pod sandbox.
    async fn get_pod_status(&self, pod_id: &str) -> Result<String, KubeletError>;
}

/// Simulated in-memory runtime provider for testing managed and external runtime engines.
#[derive(Debug)]
pub struct MockRuntimeProvider {
    name: String,
    pod_counter: AtomicUsize,
}

impl MockRuntimeProvider {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pod_counter: AtomicUsize::new(1),
        }
    }
}

#[async_trait]
impl RuntimeProvider for MockRuntimeProvider {
    fn provider_name(&self) -> &str {
        &self.name
    }

    async fn run_pod(&self, pod: &Value) -> Result<String, KubeletError> {
        let name = pod
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("unnamed");

        let id = self.pod_counter.fetch_add(1, Ordering::SeqCst);
        let pod_id = format!("{}-{}-{}", self.name, name, id);
        Ok(pod_id)
    }

    async fn stop_pod(&self, _pod_id: &str) -> Result<(), KubeletError> {
        Ok(())
    }

    async fn get_pod_status(&self, _pod_id: &str) -> Result<String, KubeletError> {
        Ok("Running".to_string())
    }
}

/// Pod reconciler driving pod lifecycle on the registered node.
#[derive(Clone, Debug)]
pub struct PodReconciler {
    client: Arc<KubernetesApiClient>,
    runtime: Arc<dyn RuntimeProvider>,
    node_name: String,
    node_ip: String,
}

impl PodReconciler {
    #[must_use]
    pub fn new(
        client: Arc<KubernetesApiClient>,
        runtime: Arc<dyn RuntimeProvider>,
        node_name: impl Into<String>,
        node_ip: impl Into<String>,
    ) -> Self {
        let node_ip_str = node_ip.into();
        let effective_ip = if node_ip_str.is_empty() {
            "127.0.0.1".to_string()
        } else {
            node_ip_str
        };

        Self {
            client,
            runtime,
            node_name: node_name.into(),
            node_ip: effective_ip,
        }
    }

    #[must_use]
    pub fn runtime_provider_name(&self) -> &str {
        self.runtime.provider_name()
    }

    /// Reconciles all pods in the specified namespace assigned to this node.
    pub async fn reconcile_namespace(&self, namespace: &str) -> Result<usize, KubeletError> {
        let pod_list = self
            .client
            .list_pods(namespace)
            .await
            .map_err(KubeletError::from)?;

        let Some(items) = pod_list.get("items").and_then(Value::as_array) else {
            return Ok(0);
        };

        let mut reconciled_count = 0;
        for pod in items {
            let Some(assigned_node) = pod
                .get("spec")
                .and_then(|s| s.get("nodeName"))
                .and_then(Value::as_str)
            else {
                continue;
            };

            if assigned_node != self.node_name {
                continue;
            }

            if let Err(e) = self.sync_pod(namespace, pod).await {
                let pod_name = pod
                    .get("metadata")
                    .and_then(|m| m.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                eprintln!("Failed to sync pod {namespace}/{pod_name}: {e}");
            } else {
                reconciled_count += 1;
            }
        }

        Ok(reconciled_count)
    }

    /// Synchronizes an individual pod's runtime state and updates its API status.
    pub async fn sync_pod(&self, namespace: &str, pod: &Value) -> Result<(), KubeletError> {
        let name = pod
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| KubeletError::PodReconciliationFailed {
                pod: "unknown".to_string(),
                reason: "pod missing metadata.name".to_string(),
            })?;

        // If the pod is already Running, skip re-execution
        if let Some(phase) = pod
            .get("status")
            .and_then(|s| s.get("phase"))
            .and_then(Value::as_str)
            && phase == "Running"
        {
            return Ok(());
        }

        // Execute pod on runtime provider
        let sandbox_id = self.runtime.run_pod(pod).await?;

        // Construct containerStatuses from spec.containers
        let mut container_statuses = Vec::new();
        if let Some(containers) = pod
            .get("spec")
            .and_then(|s| s.get("containers"))
            .and_then(Value::as_array)
        {
            for (idx, c) in containers.iter().enumerate() {
                let c_name = c.get("name").and_then(Value::as_str).unwrap_or("main");
                let c_image = c.get("image").and_then(Value::as_str).unwrap_or("unknown");

                container_statuses.push(json!({
                    "name": c_name,
                    "ready": true,
                    "restartCount": 0,
                    "image": c_image,
                    "imageID": format!("{}-image-{}", self.runtime.provider_name(), c_image),
                    "containerID": format!("{}://{}-c-{}", self.runtime.provider_name(), sandbox_id, idx),
                    "state": {
                        "running": {
                            "startedAt": "2026-09-30T12:00:00Z"
                        }
                    }
                }));
            }
        }

        let status = json!({
            "phase": "Running",
            "hostIP": self.node_ip,
            "podIP": self.node_ip,
            "startTime": "2026-09-30T12:00:00Z",
            "conditions": [
                {
                    "type": "PodScheduled",
                    "status": "True",
                    "reason": "PodScheduled",
                    "message": "pod assigned to node"
                },
                {
                    "type": "Initialized",
                    "status": "True",
                    "reason": "PodInitialized",
                    "message": "all init containers completed"
                },
                {
                    "type": "ContainersReady",
                    "status": "True",
                    "reason": "ContainersReady",
                    "message": "all containers ready"
                },
                {
                    "type": "Ready",
                    "status": "True",
                    "reason": "PodReady",
                    "message": "pod is ready"
                }
            ],
            "containerStatuses": container_statuses
        });

        self.client
            .patch_pod_status(namespace, name, status)
            .await
            .map_err(|e| KubeletError::PodReconciliationFailed {
                pod: format!("{namespace}/{name}"),
                reason: format!("failed to patch pod status: {e}"),
            })?;

        Ok(())
    }
}
