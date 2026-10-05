//! OCI container-engine adapter.
//!
//! [`OciRuntimeAdapter`] implements [`RuntimeProvider`] for any engine that can
//! run, list, remove, log and exec labelled containers. The adapter owns the
//! Kubernetes semantics: one container per Pod container, correlation back to
//! the Pod through labels keyed on `metadata.uid`, `command`/`args` mapped onto
//! entrypoint and arguments, and stopping containers whose Pod is gone.
//! Engines such as [`crate::podman::PodmanEngine`] own only the command line.
//!
//! The adapter is not a CRI. It covers single-node, non-networked pods:
//! `command`, `args`, `env`, `workingDir` and `imagePullPolicy` are honoured;
//! volumes, probes, ports, init containers and restarts are not.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::KubeletError;
use crate::registration::rfc3339_seconds;
use crate::workload::{
    ContainerRuntimeState, ContainerRuntimeStatus, ExecResult, ManagedPodRef, PodRuntimeStatus,
    RuntimeProvider,
};

/// Marks every container this kubelet created.
pub const LABEL_MANAGED: &str = "io.rubix.managed";
/// Pod `metadata.uid`; the sandbox identity handed to [`RuntimeProvider::stop_pod`].
pub const LABEL_POD_UID: &str = "io.rubix.pod-uid";
pub const LABEL_POD_NAMESPACE: &str = "io.rubix.pod-namespace";
pub const LABEL_POD_NAME: &str = "io.rubix.pod-name";
pub const LABEL_CONTAINER_NAME: &str = "io.rubix.container-name";

/// Image pull behaviour, from `imagePullPolicy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PullPolicy {
    /// Pull only when the image is absent (`IfNotPresent`).
    #[default]
    Missing,
    Always,
    Never,
}

/// Engine-neutral description of one container to start.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContainerSpec {
    pub name: String,
    pub image: String,
    pub pull: PullPolicy,
    /// Replaces the image entrypoint (Kubernetes `command`).
    pub entrypoint: Option<Vec<String>>,
    /// Replaces the image command (Kubernetes `args`).
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub working_dir: Option<String>,
    pub labels: BTreeMap<String, String>,
}

/// Normalised container state as reported by an engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContainerState {
    /// Created, paused or otherwise not executing; carries the engine's own word.
    Idle(String),
    Running {
        started_at: Option<u64>,
    },
    Exited {
        exit_code: i32,
        started_at: Option<u64>,
        finished_at: Option<u64>,
    },
}

/// One container known to the engine. Timestamps are Unix seconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerSummary {
    pub id: String,
    pub image: String,
    pub image_id: String,
    pub labels: BTreeMap<String, String>,
    pub state: ContainerState,
}

impl ContainerSummary {
    fn label(&self, key: &str) -> String {
        self.labels.get(key).cloned().unwrap_or_default()
    }
}

/// Container-level operations of an OCI engine (podman, docker, nerdctl, ...).
#[async_trait]
pub trait OciEngine: std::fmt::Debug + Send + Sync {
    /// Short engine name used as the `containerID` scheme, e.g. `podman`.
    fn name(&self) -> &str;

    /// Engine version if known, reported in the node's `containerRuntimeVersion`.
    fn version(&self) -> Option<String>;

    /// Verifies the engine answers.
    async fn ping(&self) -> Result<(), KubeletError>;

    /// Starts a detached container and returns its engine id.
    async fn run_detached(&self, spec: &ContainerSpec) -> Result<String, KubeletError>;

    /// Lists containers, including stopped ones, carrying every given label.
    async fn list(
        &self,
        label_filters: &[(&str, &str)],
    ) -> Result<Vec<ContainerSummary>, KubeletError>;

    /// Stops (if needed) and removes containers.
    async fn remove(&self, ids: &[String]) -> Result<(), KubeletError>;

    /// Captured stdout and stderr of a container.
    async fn logs(&self, id: &str, tail_lines: Option<usize>) -> Result<String, KubeletError>;

    /// Runs a command inside a running container.
    async fn exec(&self, id: &str, command: &[String]) -> Result<ExecResult, KubeletError>;
}

/// [`RuntimeProvider`] over any [`OciEngine`].
#[derive(Clone, Debug)]
pub struct OciRuntimeAdapter {
    engine: Arc<dyn OciEngine>,
}

impl OciRuntimeAdapter {
    #[must_use]
    pub fn new(engine: Arc<dyn OciEngine>) -> Self {
        Self { engine }
    }

    #[must_use]
    pub fn engine(&self) -> &Arc<dyn OciEngine> {
        &self.engine
    }

    fn qualified_id(&self, id: &str) -> String {
        format!("{}://{id}", self.engine.name())
    }

    async fn container_of(
        &self,
        pod_uid: &str,
        container_name: &str,
    ) -> Result<String, KubeletError> {
        let containers = self
            .engine
            .list(&[
                (LABEL_POD_UID, pod_uid),
                (LABEL_CONTAINER_NAME, container_name),
            ])
            .await?;
        containers
            .into_iter()
            .next()
            .map(|container| container.id)
            .ok_or_else(|| KubeletError::ContainerOperationFailed {
                container: container_name.to_string(),
                reason: format!("no {} container for pod {pod_uid}", self.engine.name()),
            })
    }

    fn runtime_status(&self, summary: &ContainerSummary) -> ContainerRuntimeStatus {
        let state = match &summary.state {
            ContainerState::Idle(word) => ContainerRuntimeState::Waiting {
                reason: format!("ContainerCreating ({word})"),
            },
            ContainerState::Running { started_at } => ContainerRuntimeState::Running {
                started_at: started_at.map(rfc3339_seconds).unwrap_or_default(),
            },
            ContainerState::Exited {
                exit_code,
                started_at,
                finished_at,
            } => ContainerRuntimeState::Terminated {
                exit_code: *exit_code,
                started_at: started_at.map(rfc3339_seconds),
                finished_at: finished_at.map(rfc3339_seconds).unwrap_or_default(),
            },
        };
        ContainerRuntimeStatus {
            name: summary.label(LABEL_CONTAINER_NAME),
            container_id: self.qualified_id(&summary.id),
            image: summary.image.clone(),
            image_id: summary.image_id.clone(),
            state,
        }
    }
}

#[async_trait]
impl RuntimeProvider for OciRuntimeAdapter {
    fn provider_name(&self) -> &str {
        self.engine.name()
    }

    fn is_external(&self) -> bool {
        true
    }

    fn runtime_version(&self) -> String {
        self.engine
            .version()
            .unwrap_or_else(|| "unknown".to_string())
    }

    async fn check_available(&self) -> Result<(), KubeletError> {
        self.engine.ping().await
    }

    async fn run_pod(&self, pod: &Value) -> Result<String, KubeletError> {
        let identity = PodIdentity::from_pod(pod)?;
        let empty = Vec::new();
        let containers = pod
            .pointer("/spec/containers")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for container in containers {
            let spec = container_spec(&identity, container)?;
            if let Err(e) = self.engine.run_detached(&spec).await {
                let _ = self.stop_pod(&identity.uid).await;
                return Err(e);
            }
        }
        Ok(identity.uid)
    }

    async fn stop_pod(&self, pod_id: &str) -> Result<(), KubeletError> {
        let ids: Vec<String> = self
            .engine
            .list(&[(LABEL_POD_UID, pod_id)])
            .await?
            .into_iter()
            .map(|container| container.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        self.engine.remove(&ids).await
    }

    async fn get_pod_status(&self, pod_id: &str) -> Result<String, KubeletError> {
        let running = self
            .engine
            .list(&[(LABEL_POD_UID, pod_id)])
            .await?
            .iter()
            .any(|c| matches!(c.state, ContainerState::Running { .. }));
        Ok(if running { "Running" } else { "Stopped" }.to_string())
    }

    async fn inspect_pod(&self, pod: &Value) -> Result<Option<PodRuntimeStatus>, KubeletError> {
        let identity = PodIdentity::from_pod(pod)?;
        let containers = self
            .engine
            .list(&[(LABEL_POD_UID, &identity.uid)])
            .await?
            .iter()
            .map(|summary| self.runtime_status(summary))
            .collect();
        Ok(Some(PodRuntimeStatus { containers }))
    }

    async fn list_managed_pods(&self) -> Result<Vec<ManagedPodRef>, KubeletError> {
        let mut pods: BTreeMap<String, ManagedPodRef> = BTreeMap::new();
        for container in self.engine.list(&[(LABEL_MANAGED, "true")]).await? {
            let Some(uid) = container.labels.get(LABEL_POD_UID) else {
                continue;
            };
            pods.entry(uid.clone()).or_insert_with(|| ManagedPodRef {
                pod_id: uid.clone(),
                namespace: container.label(LABEL_POD_NAMESPACE),
                name: container.label(LABEL_POD_NAME),
                uid: uid.clone(),
            });
        }
        Ok(pods.into_values().collect())
    }

    async fn get_container_logs(
        &self,
        pod_id: &str,
        container_name: &str,
        tail_lines: Option<usize>,
    ) -> Result<String, KubeletError> {
        let id = self.container_of(pod_id, container_name).await?;
        self.engine.logs(&id, tail_lines).await
    }

    async fn exec_in_container(
        &self,
        pod_id: &str,
        container_name: &str,
        cmd: &[String],
    ) -> Result<ExecResult, KubeletError> {
        let id = self.container_of(pod_id, container_name).await?;
        self.engine.exec(&id, cmd).await
    }
}

/// The Pod fields containers are labelled with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodIdentity {
    pub namespace: String,
    pub name: String,
    pub uid: String,
}

impl PodIdentity {
    /// Reads identity from a stored Pod object; the uid must already be assigned.
    pub fn from_pod(pod: &Value) -> Result<Self, KubeletError> {
        let name = pod
            .pointer("/metadata/name")
            .and_then(Value::as_str)
            .ok_or_else(|| KubeletError::PodReconciliationFailed {
                pod: "unknown".to_string(),
                reason: "pod missing metadata.name".to_string(),
            })?;
        let uid = pod
            .pointer("/metadata/uid")
            .and_then(Value::as_str)
            .ok_or_else(|| KubeletError::PodReconciliationFailed {
                pod: name.to_string(),
                reason: "pod missing metadata.uid".to_string(),
            })?;
        Ok(Self {
            namespace: pod
                .pointer("/metadata/namespace")
                .and_then(Value::as_str)
                .unwrap_or("default")
                .to_string(),
            name: name.to_string(),
            uid: uid.to_string(),
        })
    }

    /// Labels that tie a container back to this pod.
    #[must_use]
    pub fn labels(&self, container_name: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            (LABEL_MANAGED.to_string(), "true".to_string()),
            (LABEL_POD_UID.to_string(), self.uid.clone()),
            (LABEL_POD_NAMESPACE.to_string(), self.namespace.clone()),
            (LABEL_POD_NAME.to_string(), self.name.clone()),
            (LABEL_CONTAINER_NAME.to_string(), container_name.to_string()),
        ])
    }
}

/// Translates one Pod container into an engine-neutral [`ContainerSpec`].
pub fn container_spec(pod: &PodIdentity, container: &Value) -> Result<ContainerSpec, KubeletError> {
    let name = container
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("main");
    let image = container
        .get("image")
        .and_then(Value::as_str)
        .filter(|image| !image.is_empty())
        .ok_or_else(|| KubeletError::ContainerOperationFailed {
            container: name.to_string(),
            reason: "container has no image".to_string(),
        })?;
    let pull = match container
        .get("imagePullPolicy")
        .and_then(Value::as_str)
        .unwrap_or("IfNotPresent")
    {
        "Never" => PullPolicy::Never,
        "Always" => PullPolicy::Always,
        _ => PullPolicy::Missing,
    };
    let strings = |key: &str| -> Option<Vec<String>> {
        container.get(key).and_then(Value::as_array).map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
    };
    let env = container
        .get("env")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let key = entry.get("name").and_then(Value::as_str)?;
                    let value = entry.get("value").and_then(Value::as_str).unwrap_or("");
                    Some((key.to_string(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(ContainerSpec {
        name: container_name(pod, name),
        image: image.to_string(),
        pull,
        entrypoint: strings("command"),
        args: strings("args").unwrap_or_default(),
        env,
        working_dir: container
            .get("workingDir")
            .and_then(Value::as_str)
            .map(str::to_owned),
        labels: pod.labels(name),
    })
}

fn container_name(pod: &PodIdentity, container: &str) -> String {
    let safe = |value: &str| -> String {
        value
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect()
    };
    let uid = safe(&pod.uid);
    let uid_tail = &uid[uid.len().saturating_sub(12)..];
    format!(
        "rubix_{}_{}_{}_{uid_tail}",
        safe(&pod.namespace),
        safe(&pod.name),
        safe(container)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    fn identity() -> PodIdentity {
        PodIdentity {
            namespace: "default".to_string(),
            name: "hello".to_string(),
            uid: "uid-pods-hello-12".to_string(),
        }
    }

    fn pod() -> Value {
        json!({
            "metadata": { "name": "hello", "namespace": "default", "uid": "uid-pods-hello-12" },
            "spec": { "containers": [
                { "name": "hello", "image": "localhost/rubix-hello:latest", "command": ["/rubix-hello", "600"] },
                { "name": "side", "image": "img2" }
            ] }
        })
    }

    #[test]
    fn container_spec_maps_command_args_env_and_pull_policy() {
        let container = json!({
            "name": "hello",
            "image": "localhost/rubix-hello:latest",
            "imagePullPolicy": "Never",
            "command": ["/rubix-hello"],
            "args": ["600"],
            "env": [{"name": "GREETING", "value": "hi"}, {"name": "EMPTY"}],
            "workingDir": "/work"
        });
        let spec = container_spec(&identity(), &container).unwrap();
        assert_eq!(spec.name, "rubix_default_hello_hello_ods-hello-12");
        assert_eq!(spec.pull, PullPolicy::Never);
        assert_eq!(
            spec.entrypoint.as_deref(),
            Some(&["/rubix-hello".to_string()][..])
        );
        assert_eq!(spec.args, ["600"]);
        assert_eq!(
            spec.env,
            [
                ("GREETING".to_string(), "hi".to_string()),
                ("EMPTY".to_string(), String::new())
            ]
        );
        assert_eq!(spec.working_dir.as_deref(), Some("/work"));
        assert_eq!(spec.labels[LABEL_POD_UID], "uid-pods-hello-12");
        assert_eq!(spec.labels[LABEL_CONTAINER_NAME], "hello");
        assert_eq!(spec.labels[LABEL_MANAGED], "true");
    }

    #[test]
    fn container_spec_requires_an_image_and_defaults_the_rest() {
        let err = container_spec(&identity(), &json!({"name": "x"})).unwrap_err();
        assert!(matches!(err, KubeletError::ContainerOperationFailed { .. }));
        let spec = container_spec(&identity(), &json!({"name": "x", "image": "img"})).unwrap();
        assert_eq!(spec.pull, PullPolicy::Missing);
        assert!(spec.entrypoint.is_none());
        assert!(spec.args.is_empty() && spec.env.is_empty() && spec.working_dir.is_none());
    }

    #[test]
    fn pod_identity_requires_uid() {
        let err = PodIdentity::from_pod(&json!({"metadata": {"name": "p"}})).unwrap_err();
        assert!(matches!(err, KubeletError::PodReconciliationFailed { .. }));
        let id = PodIdentity::from_pod(&json!({"metadata": {"name": "p", "uid": "u"}})).unwrap();
        assert_eq!(id.namespace, "default");
    }

    /// In-memory engine: records every spec it was asked to run.
    #[derive(Debug, Default)]
    struct FakeEngine {
        containers: Mutex<Vec<ContainerSummary>>,
        fail_on: Option<String>,
        removed: Mutex<Vec<Vec<String>>>,
    }

    #[async_trait]
    impl OciEngine for FakeEngine {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn version(&self) -> Option<String> {
            Some("9.9".to_string())
        }
        async fn ping(&self) -> Result<(), KubeletError> {
            Ok(())
        }
        async fn run_detached(&self, spec: &ContainerSpec) -> Result<String, KubeletError> {
            if self.fail_on.as_deref() == Some(spec.labels[LABEL_CONTAINER_NAME].as_str()) {
                return Err(KubeletError::ContainerOperationFailed {
                    container: spec.name.clone(),
                    reason: "boom".to_string(),
                });
            }
            let mut containers = self.containers.lock().unwrap();
            let id = format!("id{}", containers.len() + 1);
            containers.push(ContainerSummary {
                id: id.clone(),
                image: spec.image.clone(),
                image_id: "sha256:img".to_string(),
                labels: spec.labels.clone(),
                state: ContainerState::Running {
                    started_at: Some(1_791_146_364),
                },
            });
            Ok(id)
        }
        async fn list(
            &self,
            filters: &[(&str, &str)],
        ) -> Result<Vec<ContainerSummary>, KubeletError> {
            Ok(self
                .containers
                .lock()
                .unwrap()
                .iter()
                .filter(|c| {
                    filters
                        .iter()
                        .all(|(k, v)| c.labels.get(*k).map(String::as_str) == Some(*v))
                })
                .cloned()
                .collect())
        }
        async fn remove(&self, ids: &[String]) -> Result<(), KubeletError> {
            self.containers
                .lock()
                .unwrap()
                .retain(|c| !ids.contains(&c.id));
            self.removed.lock().unwrap().push(ids.to_vec());
            Ok(())
        }
        async fn logs(&self, id: &str, tail: Option<usize>) -> Result<String, KubeletError> {
            Ok(format!("logs {id} {tail:?}"))
        }
        async fn exec(&self, id: &str, command: &[String]) -> Result<ExecResult, KubeletError> {
            Ok(ExecResult {
                exit_code: 0,
                stdout: format!("{id} {}", command.join(" ")),
                stderr: String::new(),
            })
        }
    }

    #[tokio::test]
    async fn adapter_runs_labels_inspects_and_removes_pod_containers() {
        let engine = Arc::new(FakeEngine::default());
        let adapter = OciRuntimeAdapter::new(engine.clone());
        assert_eq!(adapter.provider_name(), "fake");
        assert_eq!(adapter.runtime_version(), "9.9");

        let sandbox = adapter.run_pod(&pod()).await.unwrap();
        assert_eq!(sandbox, "uid-pods-hello-12");
        let observed = adapter.inspect_pod(&pod()).await.unwrap().unwrap();
        assert_eq!(observed.containers.len(), 2);
        assert_eq!(observed.containers[0].name, "hello");
        assert_eq!(observed.containers[0].container_id, "fake://id1");
        assert_eq!(
            observed.containers[0].state,
            ContainerRuntimeState::Running {
                started_at: "2026-10-04T20:39:24Z".to_string()
            }
        );
        assert_eq!(adapter.get_pod_status(&sandbox).await.unwrap(), "Running");

        let managed = adapter.list_managed_pods().await.unwrap();
        assert_eq!(managed.len(), 1);
        assert_eq!(
            (managed[0].namespace.as_str(), managed[0].name.as_str()),
            ("default", "hello")
        );

        assert_eq!(
            adapter
                .get_container_logs(&sandbox, "side", Some(3))
                .await
                .unwrap(),
            "logs id2 Some(3)"
        );
        let exec = adapter
            .exec_in_container(&sandbox, "hello", &["true".to_string()])
            .await
            .unwrap();
        assert_eq!(exec.stdout, "id1 true");
        let err = adapter
            .get_container_logs(&sandbox, "missing", None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no fake container"), "{err}");

        adapter.stop_pod(&sandbox).await.unwrap();
        assert_eq!(
            *engine.removed.lock().unwrap(),
            vec![vec!["id1".to_string(), "id2".to_string()]]
        );
        assert!(
            adapter
                .inspect_pod(&pod())
                .await
                .unwrap()
                .unwrap()
                .containers
                .is_empty()
        );
        assert_eq!(adapter.get_pod_status(&sandbox).await.unwrap(), "Stopped");
        // Stopping an unknown pod is a no-op, not an error.
        adapter.stop_pod("nope").await.unwrap();
        assert_eq!(engine.removed.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn adapter_rolls_back_started_containers_when_a_later_one_fails() {
        let engine = Arc::new(FakeEngine {
            fail_on: Some("side".to_string()),
            ..FakeEngine::default()
        });
        let adapter = OciRuntimeAdapter::new(engine.clone());
        let err = adapter.run_pod(&pod()).await.unwrap_err();
        assert!(err.to_string().contains("boom"), "{err}");
        assert!(engine.containers.lock().unwrap().is_empty());
        assert_eq!(
            *engine.removed.lock().unwrap(),
            vec![vec!["id1".to_string()]]
        );
    }

    #[test]
    fn exited_and_idle_states_map_to_runtime_states() {
        let adapter = OciRuntimeAdapter::new(Arc::new(FakeEngine::default()));
        let mut summary = ContainerSummary {
            id: "abc".to_string(),
            image: "img".to_string(),
            image_id: "sha".to_string(),
            labels: identity().labels("hello"),
            state: ContainerState::Exited {
                exit_code: 3,
                started_at: None,
                finished_at: Some(1_791_146_370),
            },
        };
        let status = adapter.runtime_status(&summary);
        assert_eq!(
            status.state,
            ContainerRuntimeState::Terminated {
                exit_code: 3,
                started_at: None,
                finished_at: "2026-10-04T20:39:30Z".to_string(),
            }
        );
        summary.state = ContainerState::Idle("created".to_string());
        assert_eq!(
            adapter.runtime_status(&summary).state,
            ContainerRuntimeState::Waiting {
                reason: "ContainerCreating (created)".to_string()
            }
        );
    }
}
