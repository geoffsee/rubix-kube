//! Container-engine adapter.
//!
//! [`EngineRuntimeAdapter`] implements [`RuntimeProvider`] for any engine that
//! can run, list, signal, remove, log and exec labelled containers: podman,
//! docker, nerdctl. It is the dockershim shape. The adapter owns the Kubernetes
//! semantics: one container per Pod container, correlation back to the Pod
//! through the `io.kubernetes.*` labels upstream tooling expects, `command` and
//! `args` mapped onto entrypoint and arguments, restart attempts, and graceful
//! termination signals. Engines own only their command line.
//!
//! A CRI runtime such as containerd already speaks pods and should implement
//! [`RuntimeProvider`] directly rather than sit beneath this adapter.
//!
//! Not covered: pod sandboxes (containers of one pod do not share namespaces,
//! so `podIP` is the node IP), volumes, probes, ports and init containers.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::KubeletError;
use crate::registration::rfc3339_seconds;
use crate::workload::{
    ContainerRuntimeState, ContainerRuntimeStatus, ExecResult, LogOptions, ManagedPodRef,
    PodRuntimeStatus, PodSignal, RuntimeProvider,
};

/// Marks containers this kubelet created; other `io.kubernetes.*` users are left alone.
pub const LABEL_MANAGED_BY: &str = "io.rubix.managed-by";
pub const MANAGED_BY: &str = "rubix-kubelet";
/// Standard kubelet container labels, as crictl and `podman kube` understand them.
pub const LABEL_POD_NAME: &str = "io.kubernetes.pod.name";
pub const LABEL_POD_NAMESPACE: &str = "io.kubernetes.pod.namespace";
pub const LABEL_POD_UID: &str = "io.kubernetes.pod.uid";
pub const LABEL_CONTAINER_NAME: &str = "io.kubernetes.container.name";
pub const LABEL_RESTART_COUNT: &str = "io.kubernetes.container.restartCount";

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
    /// Image identity, preferably `sha256:<digest>` or `repo@sha256:<digest>`.
    pub image_id: String,
    pub labels: BTreeMap<String, String>,
    pub state: ContainerState,
}

impl ContainerSummary {
    fn label(&self, key: &str) -> String {
        self.labels.get(key).cloned().unwrap_or_default()
    }
}

/// Container-level operations of a container engine.
#[async_trait]
pub trait ContainerEngine: std::fmt::Debug + Send + Sync {
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

    /// Sends a signal (`TERM`, `KILL`) to running containers without waiting.
    async fn signal(&self, ids: &[String], signal: &str) -> Result<(), KubeletError>;

    /// Stops (if needed) and removes containers.
    async fn remove(&self, ids: &[String]) -> Result<(), KubeletError>;

    /// Captured stdout and stderr of a container, interleaved in time order.
    async fn logs(&self, id: &str, options: &LogOptions) -> Result<String, KubeletError>;

    /// Runs a command inside a running container.
    async fn exec(&self, id: &str, command: &[String]) -> Result<ExecResult, KubeletError>;
}

/// [`RuntimeProvider`] over any [`ContainerEngine`].
#[derive(Clone, Debug)]
pub struct EngineRuntimeAdapter {
    engine: Arc<dyn ContainerEngine>,
}

impl EngineRuntimeAdapter {
    #[must_use]
    pub fn new(engine: Arc<dyn ContainerEngine>) -> Self {
        Self { engine }
    }

    #[must_use]
    pub fn engine(&self) -> &Arc<dyn ContainerEngine> {
        &self.engine
    }

    fn qualified_id(&self, id: &str) -> String {
        format!("{}://{id}", self.engine.name())
    }

    async fn managed(&self, extra: &[(&str, &str)]) -> Result<Vec<ContainerSummary>, KubeletError> {
        let mut filters = vec![(LABEL_MANAGED_BY, MANAGED_BY)];
        filters.extend_from_slice(extra);
        self.engine.list(&filters).await
    }

    /// The newest attempt of one container of a pod.
    async fn container_of(
        &self,
        pod_uid: &str,
        container_name: &str,
    ) -> Result<String, KubeletError> {
        let containers = self
            .managed(&[
                (LABEL_POD_UID, pod_uid),
                (LABEL_CONTAINER_NAME, container_name),
            ])
            .await?;
        containers
            .into_iter()
            .max_by_key(restart_count)
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
            restart_count: restart_count(summary),
            state,
        }
    }
}

fn restart_count(summary: &ContainerSummary) -> u32 {
    summary
        .labels
        .get(LABEL_RESTART_COUNT)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

#[async_trait]
impl RuntimeProvider for EngineRuntimeAdapter {
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
            let spec = container_spec(pod, &identity, container, 0)?;
            if let Err(e) = self.engine.run_detached(&spec).await {
                let _ = self.stop_pod(&identity.uid).await;
                return Err(e);
            }
        }
        Ok(identity.uid)
    }

    async fn restart_container(
        &self,
        pod: &Value,
        container_name: &str,
        attempt: u32,
    ) -> Result<(), KubeletError> {
        let identity = PodIdentity::from_pod(pod)?;
        let container = pod
            .pointer("/spec/containers")
            .and_then(Value::as_array)
            .and_then(|containers| {
                containers
                    .iter()
                    .find(|c| c.get("name").and_then(Value::as_str) == Some(container_name))
            })
            .ok_or_else(|| KubeletError::ContainerOperationFailed {
                container: container_name.to_string(),
                reason: "container is not in the pod spec".to_string(),
            })?;
        let previous: Vec<String> = self
            .managed(&[
                (LABEL_POD_UID, &identity.uid),
                (LABEL_CONTAINER_NAME, container_name),
            ])
            .await?
            .into_iter()
            .map(|c| c.id)
            .collect();
        if !previous.is_empty() {
            self.engine.remove(&previous).await?;
        }
        let spec = container_spec(pod, &identity, container, attempt)?;
        self.engine.run_detached(&spec).await.map(drop)
    }

    async fn signal_pod(&self, pod_id: &str, signal: PodSignal) -> Result<(), KubeletError> {
        let running: Vec<String> = self
            .managed(&[(LABEL_POD_UID, pod_id)])
            .await?
            .into_iter()
            .filter(|c| matches!(c.state, ContainerState::Running { .. }))
            .map(|c| c.id)
            .collect();
        if running.is_empty() {
            return Ok(());
        }
        let name = match signal {
            PodSignal::Terminate => "TERM",
            PodSignal::Kill => "KILL",
        };
        self.engine.signal(&running, name).await
    }

    async fn stop_pod(&self, pod_id: &str) -> Result<(), KubeletError> {
        let ids: Vec<String> = self
            .managed(&[(LABEL_POD_UID, pod_id)])
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
            .managed(&[(LABEL_POD_UID, pod_id)])
            .await?
            .iter()
            .any(|c| matches!(c.state, ContainerState::Running { .. }));
        Ok(if running { "Running" } else { "Stopped" }.to_string())
    }

    async fn inspect_pod(&self, pod: &Value) -> Result<Option<PodRuntimeStatus>, KubeletError> {
        let identity = PodIdentity::from_pod(pod)?;
        let containers = self
            .managed(&[(LABEL_POD_UID, &identity.uid)])
            .await?
            .iter()
            .map(|summary| self.runtime_status(summary))
            .collect();
        Ok(Some(PodRuntimeStatus { containers }))
    }

    async fn list_managed_pods(&self) -> Result<Vec<ManagedPodRef>, KubeletError> {
        let mut pods: BTreeMap<String, ManagedPodRef> = BTreeMap::new();
        for container in self.managed(&[]).await? {
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
        let options = LogOptions {
            tail_lines,
            ..LogOptions::default()
        };
        self.read_container_logs(pod_id, container_name, &options)
            .await
    }

    async fn read_container_logs(
        &self,
        pod_id: &str,
        container_name: &str,
        options: &LogOptions,
    ) -> Result<String, KubeletError> {
        let id = self.container_of(pod_id, container_name).await?;
        self.engine.logs(&id, options).await
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

    /// Labels that tie a container attempt back to this pod.
    #[must_use]
    pub fn labels(&self, container_name: &str, attempt: u32) -> BTreeMap<String, String> {
        BTreeMap::from([
            (LABEL_MANAGED_BY.to_string(), MANAGED_BY.to_string()),
            (LABEL_POD_UID.to_string(), self.uid.clone()),
            (LABEL_POD_NAMESPACE.to_string(), self.namespace.clone()),
            (LABEL_POD_NAME.to_string(), self.name.clone()),
            (LABEL_CONTAINER_NAME.to_string(), container_name.to_string()),
            (LABEL_RESTART_COUNT.to_string(), attempt.to_string()),
        ])
    }
}

/// Translates one Pod container into an engine-neutral [`ContainerSpec`].
///
/// Follows the kubelet: an absent `imagePullPolicy` means `Always` for `:latest`
/// or untagged images and `IfNotPresent` otherwise; `env.valueFrom.fieldRef`
/// resolves pod metadata and status fields; other `valueFrom` sources are
/// configuration errors.
pub fn container_spec(
    pod: &Value,
    identity: &PodIdentity,
    container: &Value,
    attempt: u32,
) -> Result<ContainerSpec, KubeletError> {
    let name = container
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("main");
    let image = container
        .get("image")
        .and_then(Value::as_str)
        .filter(|image| !image.is_empty())
        .ok_or_else(|| KubeletError::InvalidConfiguration {
            field: format!("spec.containers[{name}].image"),
            reason: "container has no image".to_string(),
        })?;
    let pull = match container.get("imagePullPolicy").and_then(Value::as_str) {
        Some("Never") => PullPolicy::Never,
        Some("Always") => PullPolicy::Always,
        None if is_latest(image) => PullPolicy::Always,
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
    let mut env = Vec::new();
    for entry in container
        .get("env")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(key) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        let value = resolve_env_value(pod, identity, name, key, entry)?;
        env.push((key.to_string(), value));
    }
    Ok(ContainerSpec {
        name: format!(
            "k8s_{name}_{}_{}_{}_{attempt}",
            identity.name, identity.namespace, identity.uid
        ),
        image: image.to_string(),
        pull,
        entrypoint: strings("command"),
        args: strings("args").unwrap_or_default(),
        env,
        working_dir: container
            .get("workingDir")
            .and_then(Value::as_str)
            .map(str::to_owned),
        labels: identity.labels(name, attempt),
    })
}

/// `:latest` or no tag at all, without a digest.
fn is_latest(image: &str) -> bool {
    if image.contains('@') {
        return false;
    }
    let last = image.rsplit('/').next().unwrap_or(image);
    match last.rsplit_once(':') {
        Some((_, tag)) => tag == "latest",
        None => true,
    }
}

fn resolve_env_value(
    pod: &Value,
    identity: &PodIdentity,
    container: &str,
    key: &str,
    entry: &Value,
) -> Result<String, KubeletError> {
    if let Some(value) = entry.get("value").and_then(Value::as_str) {
        return Ok(value.to_string());
    }
    let Some(source) = entry.get("valueFrom") else {
        return Ok(String::new());
    };
    let Some(field) = source
        .pointer("/fieldRef/fieldPath")
        .and_then(Value::as_str)
    else {
        let kind = source
            .as_object()
            .and_then(|map| map.keys().next().cloned())
            .unwrap_or_else(|| "valueFrom".to_string());
        return Err(KubeletError::InvalidConfiguration {
            field: format!("spec.containers[{container}].env[{key}]"),
            reason: format!("env source {kind} is not supported by this runtime"),
        });
    };
    let value = match field {
        "metadata.name" => identity.name.clone(),
        "metadata.namespace" => identity.namespace.clone(),
        "metadata.uid" => identity.uid.clone(),
        "spec.nodeName" | "spec.serviceAccountName" | "status.hostIP" | "status.podIP" => pod
            .pointer(&format!("/{}", field.replace('.', "/")))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        other => {
            return Err(KubeletError::InvalidConfiguration {
                field: format!("spec.containers[{container}].env[{key}]"),
                reason: format!("fieldRef {other} is not supported"),
            });
        },
    };
    Ok(value)
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
            "spec": {
                "nodeName": "node-a",
                "containers": [
                    { "name": "hello", "image": "localhost/rubix-hello:latest", "imagePullPolicy": "Never", "command": ["/rubix-hello", "600"] },
                    { "name": "side", "image": "img2:1.0" }
                ]
            },
            "status": { "hostIP": "10.0.0.5" }
        })
    }

    #[test]
    fn container_spec_maps_command_args_env_and_labels() {
        let container = json!({
            "name": "hello",
            "image": "localhost/rubix-hello:latest",
            "imagePullPolicy": "Never",
            "command": ["/rubix-hello"],
            "args": ["600"],
            "env": [
                {"name": "GREETING", "value": "hi"},
                {"name": "EMPTY"},
                {"name": "POD", "valueFrom": {"fieldRef": {"fieldPath": "metadata.name"}}},
                {"name": "NODE", "valueFrom": {"fieldRef": {"fieldPath": "spec.nodeName"}}},
                {"name": "HOST", "valueFrom": {"fieldRef": {"fieldPath": "status.hostIP"}}}
            ],
            "workingDir": "/work"
        });
        let spec = container_spec(&pod(), &identity(), &container, 2).unwrap();
        assert_eq!(spec.name, "k8s_hello_hello_default_uid-pods-hello-12_2");
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
                ("EMPTY".to_string(), String::new()),
                ("POD".to_string(), "hello".to_string()),
                ("NODE".to_string(), "node-a".to_string()),
                ("HOST".to_string(), "10.0.0.5".to_string()),
            ]
        );
        assert_eq!(spec.working_dir.as_deref(), Some("/work"));
        assert_eq!(spec.labels[LABEL_POD_UID], "uid-pods-hello-12");
        assert_eq!(spec.labels[LABEL_POD_NAME], "hello");
        assert_eq!(spec.labels[LABEL_POD_NAMESPACE], "default");
        assert_eq!(spec.labels[LABEL_CONTAINER_NAME], "hello");
        assert_eq!(spec.labels[LABEL_RESTART_COUNT], "2");
        assert_eq!(spec.labels[LABEL_MANAGED_BY], MANAGED_BY);
    }

    #[test]
    fn pull_policy_defaults_follow_the_kubelet() {
        let spec = |image: &str| {
            container_spec(
                &pod(),
                &identity(),
                &json!({"name": "x", "image": image}),
                0,
            )
            .unwrap()
            .pull
        };
        assert_eq!(spec("nginx"), PullPolicy::Always);
        assert_eq!(spec("nginx:latest"), PullPolicy::Always);
        assert_eq!(spec("localhost:5000/app"), PullPolicy::Always);
        assert_eq!(spec("nginx:1.27"), PullPolicy::Missing);
        assert_eq!(spec("localhost:5000/app:v1"), PullPolicy::Missing);
        assert_eq!(spec("nginx@sha256:abcd"), PullPolicy::Missing);
    }

    #[test]
    fn container_spec_rejects_missing_image_and_unsupported_env_sources() {
        let err = container_spec(&pod(), &identity(), &json!({"name": "x"}), 0).unwrap_err();
        assert!(matches!(err, KubeletError::InvalidConfiguration { .. }));
        let container = json!({
            "name": "x", "image": "img:1",
            "env": [{"name": "S", "valueFrom": {"secretKeyRef": {"name": "s", "key": "k"}}}]
        });
        let err = container_spec(&pod(), &identity(), &container, 0).unwrap_err();
        assert!(err.to_string().contains("secretKeyRef"), "{err}");
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
        next_id: std::sync::atomic::AtomicUsize,
        containers: Mutex<Vec<ContainerSummary>>,
        fail_on: Option<String>,
        removed: Mutex<Vec<Vec<String>>>,
        signals: Mutex<Vec<(Vec<String>, String)>>,
    }

    #[async_trait]
    impl ContainerEngine for FakeEngine {
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
            let id = format!(
                "id{}",
                self.next_id
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    + 1
            );
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
        async fn signal(&self, ids: &[String], signal: &str) -> Result<(), KubeletError> {
            self.signals
                .lock()
                .unwrap()
                .push((ids.to_vec(), signal.to_string()));
            Ok(())
        }
        async fn remove(&self, ids: &[String]) -> Result<(), KubeletError> {
            self.containers
                .lock()
                .unwrap()
                .retain(|c| !ids.contains(&c.id));
            self.removed.lock().unwrap().push(ids.to_vec());
            Ok(())
        }
        async fn logs(&self, id: &str, options: &LogOptions) -> Result<String, KubeletError> {
            Ok(format!(
                "logs {id} {:?} {}",
                options.tail_lines, options.timestamps
            ))
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
    async fn adapter_runs_labels_inspects_signals_and_removes_pod_containers() {
        let engine = Arc::new(FakeEngine::default());
        let adapter = EngineRuntimeAdapter::new(engine.clone());
        assert_eq!(adapter.provider_name(), "fake");
        assert_eq!(adapter.runtime_version(), "9.9");

        let sandbox = adapter.run_pod(&pod()).await.unwrap();
        assert_eq!(sandbox, "uid-pods-hello-12");
        let observed = adapter.inspect_pod(&pod()).await.unwrap().unwrap();
        assert_eq!(observed.containers.len(), 2);
        assert_eq!(observed.containers[0].name, "hello");
        assert_eq!(observed.containers[0].container_id, "fake://id1");
        assert_eq!(observed.containers[0].restart_count, 0);
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

        let options = LogOptions {
            tail_lines: Some(3),
            timestamps: true,
            since_seconds: None,
        };
        assert_eq!(
            adapter
                .read_container_logs(&sandbox, "side", &options)
                .await
                .unwrap(),
            "logs id2 Some(3) true"
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

        adapter
            .signal_pod(&sandbox, PodSignal::Terminate)
            .await
            .unwrap();
        assert_eq!(
            *engine.signals.lock().unwrap(),
            vec![(
                vec!["id1".to_string(), "id2".to_string()],
                "TERM".to_string()
            )]
        );

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
        adapter.stop_pod("nope").await.unwrap();
        assert_eq!(engine.removed.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn adapter_restarts_one_container_as_a_new_attempt() {
        let engine = Arc::new(FakeEngine::default());
        let adapter = EngineRuntimeAdapter::new(engine.clone());
        adapter.run_pod(&pod()).await.unwrap();
        adapter.restart_container(&pod(), "hello", 1).await.unwrap();
        assert_eq!(
            *engine.removed.lock().unwrap(),
            vec![vec!["id1".to_string()]]
        );
        let observed = adapter.inspect_pod(&pod()).await.unwrap().unwrap();
        let hello: Vec<_> = observed
            .containers
            .iter()
            .filter(|c| c.name == "hello")
            .collect();
        assert_eq!(hello.len(), 1);
        assert_eq!(hello[0].restart_count, 1);
        assert_eq!(hello[0].container_id, "fake://id3");
        let err = adapter
            .restart_container(&pod(), "ghost", 1)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not in the pod spec"), "{err}");
    }

    #[tokio::test]
    async fn adapter_rolls_back_started_containers_when_a_later_one_fails() {
        let engine = Arc::new(FakeEngine {
            fail_on: Some("side".to_string()),
            ..FakeEngine::default()
        });
        let adapter = EngineRuntimeAdapter::new(engine.clone());
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
        let adapter = EngineRuntimeAdapter::new(Arc::new(FakeEngine::default()));
        let mut summary = ContainerSummary {
            id: "abc".to_string(),
            image: "img".to_string(),
            image_id: "sha256:abc".to_string(),
            labels: identity().labels("hello", 3),
            state: ContainerState::Exited {
                exit_code: 3,
                started_at: None,
                finished_at: Some(1_791_146_370),
            },
        };
        let status = adapter.runtime_status(&summary);
        assert_eq!(status.restart_count, 3);
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
