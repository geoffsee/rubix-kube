use std::sync::Arc;

use rubix_apiserver::client::KubernetesApiClient;
use serde_json::{Value, json};

use crate::error::ControllerError;

#[derive(Clone, Debug)]
pub struct JobReconciler {
    client: Arc<KubernetesApiClient>,
}

impl JobReconciler {
    #[must_use]
    pub fn new(client: Arc<KubernetesApiClient>) -> Self {
        Self { client }
    }

    /// Reconciles all Jobs in the specified namespace.
    pub async fn reconcile_all(&self, namespace: &str) -> Result<usize, ControllerError> {
        let list = self.client.list_jobs(namespace).await.map_err(|e| {
            ControllerError::ReconciliationFailed {
                resource: "jobs".to_string(),
                reason: e.to_string(),
            }
        })?;

        let items = list
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut count = 0;
        for job in items {
            self.reconcile_job(namespace, &job).await?;
            count += 1;
        }

        Ok(count)
    }

    async fn sync_job_pods(
        &self,
        namespace: &str,
        name: &str,
        uid: &str,
        parallelism: usize,
        template_labels: Value,
        template_spec: Value,
    ) -> Result<(), ControllerError> {
        let pod_list = self.client.list_pods(namespace).await.map_err(|e| {
            ControllerError::ReconciliationFailed {
                resource: "pods".to_string(),
                reason: e.to_string(),
            }
        })?;

        let all_pods = pod_list
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let owned_pods: Vec<Value> = all_pods
            .into_iter()
            .filter(|p| {
                p.get("metadata")
                    .and_then(|m| m.get("ownerReferences"))
                    .and_then(Value::as_array)
                    .is_some_and(|owners| {
                        owners.iter().any(|o| {
                            o.get("kind").and_then(Value::as_str) == Some("Job")
                                && (o.get("uid").and_then(Value::as_str) == Some(uid)
                                    || o.get("name").and_then(Value::as_str) == Some(name))
                        })
                    })
            })
            .collect();

        let current_count = owned_pods.len();

        if current_count < parallelism {
            for idx in current_count..parallelism {
                let pod_name = format!("{name}-{idx}");
                if self.client.get_pod(namespace, &pod_name).await.is_ok() {
                    continue;
                }

                let new_pod = json!({
                    "apiVersion": "v1",
                    "kind": "Pod",
                    "metadata": {
                        "name": pod_name,
                        "namespace": namespace,
                        "labels": template_labels,
                        "ownerReferences": [{
                            "apiVersion": "batch/v1",
                            "kind": "Job",
                            "name": name,
                            "uid": uid,
                            "controller": true,
                            "blockOwnerDeletion": true
                        }]
                    },
                    "spec": template_spec,
                    "status": {
                        "phase": "Pending"
                    }
                });

                self.client
                    .create_pod(namespace, new_pod)
                    .await
                    .map_err(|e| ControllerError::ReconciliationFailed {
                        resource: format!("pods/{pod_name}"),
                        reason: e.to_string(),
                    })?;
            }
        }
        Ok(())
    }

    /// Reconciles a single Job by creating or monitoring child Pods.
    pub async fn reconcile_job(
        &self,
        namespace: &str,
        job: &Value,
    ) -> Result<Value, ControllerError> {
        let name = job
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| ControllerError::InvalidConfiguration {
                field: "metadata.name".to_string(),
                reason: "Job missing metadata.name".to_string(),
            })?;

        let uid = job
            .get("metadata")
            .and_then(|m| m.get("uid"))
            .and_then(Value::as_str)
            .unwrap_or("");

        let parallelism = job
            .get("spec")
            .and_then(|s| s.get("parallelism"))
            .and_then(Value::as_i64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(1)
            .max(1);

        let template = job
            .get("spec")
            .and_then(|s| s.get("template"))
            .cloned()
            .unwrap_or_else(|| json!({ "spec": { "containers": [] } }));

        let mut template_labels = template
            .get("metadata")
            .and_then(|m| m.get("labels"))
            .cloned()
            .unwrap_or_else(|| json!({}));

        if let Some(labels_obj) = template_labels.as_object_mut() {
            labels_obj.insert("job-name".to_string(), json!(name));
            labels_obj.insert("batch.kubernetes.io/job-name".to_string(), json!(name));
            if !uid.is_empty() {
                labels_obj.insert("controller-uid".to_string(), json!(uid));
            }
        }

        let template_spec = template
            .get("spec")
            .cloned()
            .unwrap_or_else(|| json!({ "containers": [] }));

        self.sync_job_pods(
            namespace,
            name,
            uid,
            parallelism,
            template_labels,
            template_spec,
        )
        .await?;

        let mut updated_job = job.clone();
        if let Some(status) = updated_job.get_mut("status").and_then(Value::as_object_mut) {
            status.insert("active".to_string(), json!(parallelism));
            status.insert("ready".to_string(), json!(parallelism));
        } else if let Some(job_obj) = updated_job.as_object_mut() {
            job_obj.insert(
                "status".to_string(),
                json!({
                    "active": parallelism,
                    "ready": parallelism
                }),
            );
        }

        self.client
            .update_job(namespace, name, updated_job)
            .await
            .map_err(|e| ControllerError::ReconciliationFailed {
                resource: format!("jobs/{name}"),
                reason: e.to_string(),
            })
    }
}
