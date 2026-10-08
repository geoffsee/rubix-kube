//! Pod reconciler: drives pods through a CRI-shaped runtime the way the
//! upstream kubelet does.
//!
//! One pass per pod: ensure a ready sandbox, bring every spec container to its
//! latest attempt (pull, create, start), probe running containers, restart
//! exited ones under `restartPolicy` with the kubelet's back-off, then write
//! the status the runtime reports. Deletion stops containers with the grace
//! period, removes the sandbox and deletes the Pod object.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use rubix_apiserver::time::{now_unix, parse_rfc3339_seconds};
use rubix_apiserver::{ApiserverError, ApiserverService, KubernetesApiClient};

use crate::config::{detect_host_cpu_count, format_cpuset};
use crate::error::KubeletError;
use crate::status::{
    ContainerView, CpuAssignment, attempt_of, attempts_by_name, pending_status, pod_status,
};
use crate::workload::{
    ANNOTATION_RESTART_COUNT, CpuManager, ExecResult, LABEL_CONTAINER_NAME, LABEL_MANAGED_BY,
    LABEL_POD_UID, LogOptions, MANAGED_BY, ReconcileReport, RestartPolicy, RuntimeProvider,
    WorkloadRestartReport, check_pod_restart_need, container_labels, cri, determine_pod_qos,
    is_container_cpu_pinning_eligible, sandbox_labels, secs_from_nanos, stage_configmap_files,
    stage_projected_downward_api, stage_projected_sa_token, stage_secret_files,
};

const RESTART_BACKOFF_INITIAL_SECS: u64 = 10;
const RESTART_BACKOFF_MAX_SECS: u64 = 300;
const RESTART_BACKOFF_RESET_SECS: u64 = 600;
/// Upper bound for a probe command.
const EXEC_TIMEOUT_SECS: i64 = 10;
/// Attempts kept per container: the current one and the previous for `logs --previous`.
const KEPT_ATTEMPTS: usize = 2;

#[derive(Debug, Clone, Copy)]
struct RestartBackoff {
    delay_secs: u64,
    not_before: u64,
}

/// Next restart delay: doubles after each quick failure, resets after a long run.
fn next_restart_backoff(
    previous: Option<RestartBackoff>,
    ran_for: u64,
    now: u64,
) -> RestartBackoff {
    let delay_secs = match previous {
        Some(backoff) if ran_for < RESTART_BACKOFF_RESET_SECS => {
            (backoff.delay_secs * 2).min(RESTART_BACKOFF_MAX_SECS)
        },
        _ => RESTART_BACKOFF_INITIAL_SECS,
    };
    RestartBackoff {
        delay_secs,
        not_before: now + delay_secs,
    }
}

/// Per-pass bookkeeping while bringing containers to their desired attempts.
#[derive(Debug, Default)]
struct SyncState {
    ready: BTreeMap<String, bool>,
    backoffs: BTreeMap<String, String>,
    waiting: BTreeMap<String, (&'static str, String)>,
    started_now: BTreeSet<String>,
    restarted: BTreeSet<String>,
}

fn container_name_of(container: &Value) -> &str {
    container
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("main")
}

/// Why a container attempt could not be started; reported as a waiting state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartFailure {
    pub reason: &'static str,
    pub message: String,
}

fn state_of(status: &cri::ContainerStatus) -> cri::ContainerState {
    cri::ContainerState::try_from(status.state).unwrap_or(cri::ContainerState::ContainerUnknown)
}

fn pod_name(pod: &Value) -> &str {
    pod.pointer("/metadata/name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn pod_uid(pod: &Value) -> Result<&str, KubeletError> {
    pod.pointer("/metadata/uid")
        .and_then(Value::as_str)
        .ok_or_else(|| KubeletError::PodReconciliationFailed {
            pod: pod_name(pod).to_string(),
            reason: "pod missing metadata.uid".to_string(),
        })
}

fn uid_filter(pod_uid: &str) -> cri::PodSandboxFilter {
    cri::PodSandboxFilter {
        label_selector: BTreeMap::from([
            (LABEL_MANAGED_BY.to_string(), MANAGED_BY.to_string()),
            (LABEL_POD_UID.to_string(), pod_uid.to_string()),
        ]),
        ..cri::PodSandboxFilter::default()
    }
}

/// Builds the CRI sandbox configuration for a pod.
#[must_use]
pub fn sandbox_config(pod: &Value, attempt: u32) -> cri::PodSandboxConfig {
    let name = pod_name(pod);
    let namespace = pod
        .pointer("/metadata/namespace")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let uid = pod
        .pointer("/metadata/uid")
        .and_then(Value::as_str)
        .unwrap_or("");
    cri::PodSandboxConfig {
        metadata: Some(cri::PodSandboxMetadata {
            name: name.to_string(),
            uid: uid.to_string(),
            namespace: namespace.to_string(),
            attempt,
        }),
        hostname: pod
            .pointer("/spec/hostname")
            .and_then(Value::as_str)
            .unwrap_or(name)
            .to_string(),
        log_directory: format!("/var/log/pods/{namespace}_{name}_{uid}"),
        labels: sandbox_labels(namespace, name, uid),
        ..cri::PodSandboxConfig::default()
    }
}

/// Builds the CRI container configuration for one attempt of a spec container.
///
/// Follows the kubelet: `command` and `args` map onto the CRI fields of the same
/// name, `env.valueFrom.fieldRef` resolves pod metadata and status fields, and
/// other `valueFrom` sources are configuration errors.
pub fn container_config(
    pod: &Value,
    container: &Value,
    attempt: u32,
) -> Result<cri::ContainerConfig, KubeletError> {
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
    let strings = |key: &str| -> Vec<String> {
        container
            .get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut envs = Vec::new();
    for entry in container
        .get("env")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(key) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        envs.push(cri::KeyValue {
            key: key.to_string(),
            value: resolve_env_value(pod, name, key, entry)?.into_bytes(),
        });
    }
    let namespace = pod
        .pointer("/metadata/namespace")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let uid = pod
        .pointer("/metadata/uid")
        .and_then(Value::as_str)
        .unwrap_or("");
    Ok(cri::ContainerConfig {
        metadata: Some(cri::ContainerMetadata {
            name: name.to_string(),
            attempt,
        }),
        image: Some(cri::ImageSpec {
            image: image.to_string(),
            user_specified_image: image.to_string(),
            ..cri::ImageSpec::default()
        }),
        command: strings("command"),
        args: strings("args"),
        working_dir: container
            .get("workingDir")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        envs,
        labels: container_labels(namespace, pod_name(pod), uid, name),
        annotations: BTreeMap::from([(ANNOTATION_RESTART_COUNT.to_string(), attempt.to_string())]),
        log_path: format!("{name}/{attempt}.log"),
        ..cri::ContainerConfig::default()
    })
}

fn resolve_env_value(
    pod: &Value,
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
        "metadata.name"
        | "metadata.namespace"
        | "metadata.uid"
        | "spec.nodeName"
        | "spec.serviceAccountName"
        | "status.hostIP"
        | "status.podIP" => pod
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

/// Pull behaviour from `imagePullPolicy`, defaulting as the kubelet does:
/// `Always` for `:latest` or untagged images, `IfNotPresent` otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullPolicy {
    Always,
    IfNotPresent,
    Never,
}

impl PullPolicy {
    #[must_use]
    pub fn of(container: &Value) -> Self {
        let image = container.get("image").and_then(Value::as_str).unwrap_or("");
        match container.get("imagePullPolicy").and_then(Value::as_str) {
            Some("Never") => Self::Never,
            Some("Always") => Self::Always,
            None if is_latest(image) => Self::Always,
            _ => Self::IfNotPresent,
        }
    }
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

/// Finds the container id for a pod uid and container name: the latest attempt,
/// or the one before it when `previous` is set.
pub async fn resolve_container(
    runtime: &dyn RuntimeProvider,
    pod_uid: &str,
    container_name: &str,
    previous: bool,
) -> Result<String, KubeletError> {
    let filter = cri::ContainerFilter {
        label_selector: BTreeMap::from([
            (LABEL_MANAGED_BY.to_string(), MANAGED_BY.to_string()),
            (LABEL_POD_UID.to_string(), pod_uid.to_string()),
            (LABEL_CONTAINER_NAME.to_string(), container_name.to_string()),
        ]),
        ..cri::ContainerFilter::default()
    };
    let mut containers = runtime.list_containers(Some(&filter)).await?;
    containers.sort_by_key(|c| c.metadata.as_ref().map_or(0, |m| m.attempt));
    let index = containers.len().checked_sub(if previous { 2 } else { 1 });
    index
        .and_then(|i| containers.get(i))
        .map(|c| c.id.clone())
        .ok_or_else(|| KubeletError::ContainerOperationFailed {
            container: container_name.to_string(),
            reason: format!(
                "no {}container for pod {pod_uid}",
                if previous { "previous " } else { "" }
            ),
        })
}

/// Pod reconciler driving pod lifecycle on the registered node.
#[derive(Clone, Debug)]
pub struct PodReconciler {
    client: Arc<KubernetesApiClient>,
    runtime: Arc<dyn RuntimeProvider>,
    node_name: String,
    node_ip: String,
    cpu_manager: Arc<CpuManager>,
    checkpoint_invalidated: Arc<AtomicBool>,
    root_dir: PathBuf,
    apiserver: Option<Arc<ApiserverService>>,
    backoff: Arc<Mutex<BTreeMap<String, RestartBackoff>>>,
    terminating: Arc<Mutex<BTreeSet<String>>>,
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
        let dummy_cpu_manager = Arc::new(CpuManager::new(
            "none",
            BTreeMap::new(),
            BTreeSet::new(),
            detect_host_cpu_count(),
            PathBuf::from("/tmp/rubix-node/cpu_manager_state"),
        ));
        Self {
            client,
            runtime,
            node_name: node_name.into(),
            node_ip: effective_ip,
            cpu_manager: dummy_cpu_manager,
            checkpoint_invalidated: Arc::new(AtomicBool::new(false)),
            root_dir: PathBuf::from("/var/lib/kubelet"),
            apiserver: None,
            backoff: Arc::new(Mutex::new(BTreeMap::new())),
            terminating: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    #[must_use]
    pub fn with_cpu_manager(mut self, cpu_manager: Arc<CpuManager>) -> Self {
        self.cpu_manager = cpu_manager;
        self
    }

    #[must_use]
    pub fn with_root_dir(mut self, root_dir: impl Into<PathBuf>) -> Self {
        self.root_dir = root_dir.into();
        self
    }

    #[must_use]
    pub fn with_apiserver(mut self, apiserver: Arc<ApiserverService>) -> Self {
        self.apiserver = Some(apiserver);
        self
    }

    #[must_use]
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    #[must_use]
    pub fn cpu_manager(&self) -> &CpuManager {
        &self.cpu_manager
    }

    pub fn mark_checkpoint_invalidated(&self, invalidated: bool) {
        self.checkpoint_invalidated
            .store(invalidated, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_checkpoint_invalidated(&self) -> bool {
        self.checkpoint_invalidated.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn runtime_provider_name(&self) -> &str {
        self.runtime.provider_name()
    }

    #[must_use]
    pub fn get_pod_volume_dir(
        &self,
        pod_uid_or_name: &str,
        plugin_name: &str,
        volume_name: &str,
    ) -> PathBuf {
        self.root_dir
            .join("pods")
            .join(pod_uid_or_name)
            .join("volumes")
            .join(plugin_name)
            .join(volume_name)
    }

    /// Finds the container id for a pod uid and container name: the latest attempt,
    /// or the one before it when `previous` is set.
    pub async fn resolve_container(
        &self,
        pod_uid: &str,
        container_name: &str,
        previous: bool,
    ) -> Result<String, KubeletError> {
        resolve_container(self.runtime.as_ref(), pod_uid, container_name, previous).await
    }

    pub async fn get_container_logs(
        &self,
        pod_uid: &str,
        container_name: &str,
        tail_lines: Option<usize>,
    ) -> Result<String, KubeletError> {
        let options = LogOptions {
            tail_lines,
            ..LogOptions::default()
        };
        self.read_container_logs(pod_uid, container_name, &options)
            .await
    }

    pub async fn read_container_logs(
        &self,
        pod_uid: &str,
        container_name: &str,
        options: &LogOptions,
    ) -> Result<String, KubeletError> {
        let id = self
            .resolve_container(pod_uid, container_name, options.previous)
            .await?;
        self.runtime.container_logs(&id, options).await
    }

    pub async fn exec_in_container(
        &self,
        pod_uid: &str,
        container_name: &str,
        cmd: &[String],
    ) -> Result<ExecResult, KubeletError> {
        let id = self
            .resolve_container(pod_uid, container_name, false)
            .await?;
        self.runtime.exec_sync(&id, cmd, EXEC_TIMEOUT_SECS).await
    }

    /// Reports workload-restart needs for surviving external-runtime containers when
    /// the CPU manager checkpoint was invalidated.
    pub async fn report_workload_restart_needs(
        &self,
        namespace: &str,
    ) -> Result<Vec<WorkloadRestartReport>, KubeletError> {
        let mut reports = Vec::new();
        if !self.runtime.is_external() || !self.is_checkpoint_invalidated() {
            return Ok(reports);
        }
        let pod_list = self.client.list_pods(namespace).await?;
        for pod in pod_list
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            reports.extend(check_pod_restart_need(
                pod,
                &self.node_name,
                self.runtime.provider_name(),
                namespace,
            ));
        }
        Ok(reports)
    }

    /// Reconciles all pods in the specified namespace assigned to this node.
    pub async fn reconcile_namespace(&self, namespace: &str) -> Result<usize, KubeletError> {
        let pod_list = self.client.list_pods(namespace).await?;
        let mut reconciled = 0;
        for pod in pod_list
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if pod.pointer("/spec/nodeName").and_then(Value::as_str) != Some(&self.node_name) {
                continue;
            }
            match self.sync_pod(namespace, pod).await {
                Ok(()) => reconciled += 1,
                Err(e) => eprintln!("Failed to sync pod {namespace}/{}: {e}", pod_name(pod)),
            }
        }
        Ok(reconciled)
    }

    /// Reconciles every pod in the cluster: binds unscheduled pods to this node,
    /// syncs or terminates the pods assigned here, and removes sandboxes whose Pod is gone.
    pub async fn reconcile_all(&self) -> Result<ReconcileReport, KubeletError> {
        let pod_list = self.client.list_all_pods().await?;
        let mut report = ReconcileReport::default();
        let mut live_uids = BTreeSet::new();
        for pod in pod_list
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(uid) = pod.pointer("/metadata/uid").and_then(Value::as_str) {
                live_uids.insert(uid.to_string());
            }
            let namespace = pod
                .pointer("/metadata/namespace")
                .and_then(Value::as_str)
                .unwrap_or("default")
                .to_string();
            let name = pod_name(pod).to_string();
            let pod = match pod.pointer("/spec/nodeName").and_then(Value::as_str) {
                Some(node) if node == self.node_name => pod.clone(),
                Some(_) => continue,
                None => match self.bind_pod_to_node(&namespace, pod).await {
                    Ok(bound) => {
                        report.bound += 1;
                        bound
                    },
                    Err(e) => {
                        report.failed += 1;
                        eprintln!("Failed to bind pod {namespace}/{name}: {e}");
                        continue;
                    },
                },
            };
            let outcome = if pod.pointer("/metadata/deletionTimestamp").is_some() {
                self.terminate_pod(&namespace, &pod).await
            } else {
                self.sync_pod(&namespace, &pod).await
            };
            match outcome {
                Ok(()) => report.synced += 1,
                Err(e) => {
                    report.failed += 1;
                    eprintln!("Failed to sync pod {namespace}/{name}: {e}");
                },
            }
        }
        report.orphans_stopped = self.stop_orphaned_pods(&live_uids).await?;
        Ok(report)
    }

    /// Assigns an unscheduled pod to this node. There is no scheduler in a
    /// single-node distribution, so the kubelet performs the binding itself.
    pub async fn bind_pod_to_node(
        &self,
        namespace: &str,
        pod: &Value,
    ) -> Result<Value, KubeletError> {
        let name = pod_name(pod);
        let mut bound = pod.clone();
        bound["spec"]["nodeName"] = json!(self.node_name);
        self.client
            .update_pod(namespace, name, bound)
            .await
            .map_err(|e| KubeletError::PodReconciliationFailed {
                pod: format!("{namespace}/{name}"),
                reason: format!("failed to bind pod to node {}: {e}", self.node_name),
            })
    }

    /// Removes sandboxes (and their containers) whose Pod object no longer exists.
    pub async fn stop_orphaned_pods(
        &self,
        live_uids: &BTreeSet<String>,
    ) -> Result<usize, KubeletError> {
        let filter = cri::PodSandboxFilter {
            label_selector: BTreeMap::from([(
                LABEL_MANAGED_BY.to_string(),
                MANAGED_BY.to_string(),
            )]),
            ..cri::PodSandboxFilter::default()
        };
        let mut orphaned: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for sandbox in self.runtime.list_pod_sandbox(Some(&filter)).await? {
            let Some(uid) = sandbox.labels.get(LABEL_POD_UID) else {
                continue;
            };
            if !live_uids.contains(uid) {
                orphaned.entry(uid.clone()).or_default().push(sandbox.id);
            }
        }
        for (uid, sandbox_ids) in &orphaned {
            for sandbox_id in sandbox_ids {
                self.remove_sandbox_tree(sandbox_id).await?;
            }
            eprintln!("removed sandbox of deleted pod {uid}");
        }
        Ok(orphaned.len())
    }

    /// Stops and removes a sandbox together with every container in it.
    async fn remove_sandbox_tree(&self, sandbox_id: &str) -> Result<(), KubeletError> {
        let _ = self.runtime.stop_pod_sandbox(sandbox_id).await;
        let filter = cri::ContainerFilter {
            pod_sandbox_id: sandbox_id.to_string(),
            ..cri::ContainerFilter::default()
        };
        for container in self.runtime.list_containers(Some(&filter)).await? {
            self.runtime.remove_container(&container.id).await?;
        }
        self.runtime.remove_pod_sandbox(sandbox_id).await
    }

    /// Full statuses of every container in a sandbox.
    async fn container_statuses(
        &self,
        sandbox_id: &str,
    ) -> Result<Vec<cri::ContainerStatus>, KubeletError> {
        let filter = cri::ContainerFilter {
            pod_sandbox_id: sandbox_id.to_string(),
            ..cri::ContainerFilter::default()
        };
        let mut statuses = Vec::new();
        for container in self.runtime.list_containers(Some(&filter)).await? {
            // A container removed between list and status shows up on the next pass.
            if let Ok(status) = self.runtime.container_status(&container.id).await {
                statuses.push(status);
            }
        }
        Ok(statuses)
    }

    /// Finds the pod's ready sandbox or creates one, removing stale sandboxes.
    async fn ensure_sandbox(
        &self,
        namespace: &str,
        pod: &Value,
    ) -> Result<(String, cri::PodSandboxConfig), KubeletError> {
        let name = pod_name(pod);
        let uid = pod_uid(pod)?;
        let sandboxes = self
            .runtime
            .list_pod_sandbox(Some(&uid_filter(uid)))
            .await?;
        if let Some(ready) = sandboxes
            .iter()
            .find(|s| s.state == cri::PodSandboxState::SandboxReady as i32)
        {
            let attempt = ready.metadata.as_ref().map_or(0, |m| m.attempt);
            return Ok((ready.id.clone(), sandbox_config(pod, attempt)));
        }
        let mut attempt = 0;
        for stale in &sandboxes {
            attempt = attempt.max(stale.metadata.as_ref().map_or(0, |m| m.attempt) + 1);
            self.remove_sandbox_tree(&stale.id).await?;
        }
        if pod.pointer("/status/phase").is_none() {
            let pending = pending_status(pod, &self.node_ip, "ContainerCreating", None);
            self.patch_status_if_changed(namespace, name, pod, pending)
                .await?;
        }
        let config = sandbox_config(pod, attempt);
        let id = self.runtime.run_pod_sandbox(&config).await?;
        Ok((id, config))
    }

    /// Makes the image available under the container's pull policy.
    async fn ensure_image(
        &self,
        container: &Value,
        sandbox_config: &cri::PodSandboxConfig,
    ) -> Result<(), StartFailure> {
        let image = container.get("image").and_then(Value::as_str).unwrap_or("");
        let spec = cri::ImageSpec {
            image: image.to_string(),
            user_specified_image: image.to_string(),
            ..cri::ImageSpec::default()
        };
        let present = self
            .runtime
            .image_status(&spec)
            .await
            .map_err(|e| StartFailure {
                reason: "ErrImagePull",
                message: format!("failed to inspect image {image}: {e}"),
            })?
            .is_some();
        let pull = match PullPolicy::of(container) {
            PullPolicy::Always => true,
            PullPolicy::IfNotPresent => !present,
            PullPolicy::Never if present => false,
            PullPolicy::Never => {
                return Err(StartFailure {
                    reason: "ErrImageNeverPull",
                    message: format!(
                        "Container image \"{image}\" is not present with pull policy of Never"
                    ),
                });
            },
        };
        if pull {
            self.runtime
                .pull_image(&spec, Some(sandbox_config))
                .await
                .map_err(|e| StartFailure {
                    reason: "ErrImagePull",
                    message: format!("failed to pull image {image}: {e}"),
                })?;
        }
        Ok(())
    }

    /// Pulls, creates and starts one attempt of a spec container.
    async fn start_attempt(
        &self,
        sandbox_id: &str,
        sandbox_config: &cri::PodSandboxConfig,
        pod: &Value,
        container: &Value,
        attempt: u32,
    ) -> Result<String, StartFailure> {
        self.ensure_image(container, sandbox_config).await?;
        let config = container_config(pod, container, attempt).map_err(|e| StartFailure {
            reason: "CreateContainerConfigError",
            message: e.to_string(),
        })?;
        let id = self
            .runtime
            .create_container(sandbox_id, &config, sandbox_config)
            .await
            .map_err(|e| StartFailure {
                reason: "CreateContainerError",
                message: e.to_string(),
            })?;
        self.runtime
            .start_container(&id)
            .await
            .map_err(|e| StartFailure {
                reason: "RunContainerError",
                message: e.to_string(),
            })?;
        Ok(id)
    }

    /// Decides whether an exited container restarts now (`Ok(next attempt)`) or
    /// waits (`Err(back-off message)`), recording the back-off.
    fn restart_decision(
        &self,
        namespace: &str,
        pod_name: &str,
        uid: &str,
        container_name: &str,
        latest: &cri::ContainerStatus,
    ) -> Result<u32, String> {
        let now = now_unix();
        let ran_for = secs_from_nanos(latest.started_at)
            .zip(secs_from_nanos(latest.finished_at))
            .map_or(0, |(started, finished)| finished.saturating_sub(started));
        let key = format!("{uid}/{container_name}");
        let mut backoffs = self
            .backoff
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match backoffs.get(&key).copied() {
            Some(backoff) if now < backoff.not_before => Err(format!(
                "back-off {}s restarting failed container={container_name} pod={pod_name}_{namespace}({uid})",
                backoff.delay_secs
            )),
            previous => {
                backoffs.insert(key, next_restart_backoff(previous, ran_for, now));
                Ok(attempt_of(latest) + 1)
            },
        }
    }

    /// CPU-manager placement for a container, allocating exclusive cores when eligible.
    fn cpu_assignment(
        &self,
        namespace: &str,
        pod_name: &str,
        pod: &Value,
        container: &Value,
    ) -> CpuAssignment {
        let c_name = container
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("main");
        let key = format!("{namespace}/{pod_name}/{c_name}");
        if let Some(cores) = is_container_cpu_pinning_eligible(determine_pod_qos(pod), container) {
            match self.cpu_manager.allocate_exclusive_cpus(&key, cores) {
                Ok(cpus) => {
                    return CpuAssignment {
                        cpuset: format_cpuset(&cpus),
                        exclusive: true,
                    };
                },
                Err(e) => eprintln!("exclusive CPU allocation failed for {key}: {e}"),
            }
        }
        CpuAssignment {
            cpuset: format_cpuset(&self.cpu_manager.shared_pool()),
            exclusive: false,
        }
    }

    /// Synchronizes an individual pod's runtime state and updates its API status.
    pub async fn sync_pod(&self, namespace: &str, pod: &Value) -> Result<(), KubeletError> {
        let name = pod_name(pod);
        pod_uid(pod)?;
        if matches!(
            pod.pointer("/status/phase").and_then(Value::as_str),
            Some("Succeeded" | "Failed")
        ) {
            return Ok(());
        }
        self.prepare_pod_volumes(namespace, pod).await?;
        let (sandbox_id, sandbox_config) = match self.ensure_sandbox(namespace, pod).await {
            Ok(sandbox) => sandbox,
            Err(e) => {
                let failed = pending_status(
                    pod,
                    &self.node_ip,
                    "FailedCreatePodSandBox",
                    Some(&e.to_string()),
                );
                self.patch_status_if_changed(namespace, name, pod, failed)
                    .await?;
                return Err(e);
            },
        };
        let empty = Vec::new();
        let spec_containers = pod
            .pointer("/spec/containers")
            .and_then(Value::as_array)
            .unwrap_or(&empty);

        let mut statuses = self.container_statuses(&sandbox_id).await?;
        let mut state = SyncState::default();
        for container in spec_containers {
            let c_name = container_name_of(container);
            let latest = attempts_by_name(&statuses)
                .get(c_name)
                .and_then(|attempts| attempts.last().copied());
            let Some(attempt) = self
                .next_attempt(namespace, pod, container, latest, &mut state)
                .await?
            else {
                continue;
            };
            match self
                .start_attempt(&sandbox_id, &sandbox_config, pod, container, attempt)
                .await
            {
                Ok(_) => {
                    state.started_now.insert(c_name.to_string());
                },
                Err(failure) => {
                    state
                        .waiting
                        .insert(c_name.to_string(), (failure.reason, failure.message));
                },
            }
        }
        if !state.started_now.is_empty() {
            statuses = self.container_statuses(&sandbox_id).await?;
            self.probe_started(spec_containers, &statuses, &mut state)
                .await?;
        }
        self.prune_attempts(&statuses).await;
        let views = self.build_views(namespace, pod, spec_containers, &statuses, &state);
        let pod_ip = self
            .runtime
            .pod_sandbox_status(&sandbox_id)
            .await
            .ok()
            .and_then(|status| status.network)
            .map(|network| network.ip);
        let status = pod_status(
            pod,
            self.runtime.provider_name(),
            &self.node_ip,
            pod_ip.as_deref(),
            true,
            RestartPolicy::of(pod),
            &views,
        );
        self.patch_status_if_changed(namespace, name, pod, status)
            .await
    }

    /// Status inputs for every spec container after this pass.
    fn build_views<'a>(
        &self,
        namespace: &str,
        pod: &'a Value,
        spec_containers: &'a [Value],
        statuses: &'a [cri::ContainerStatus],
        state: &SyncState,
    ) -> Vec<ContainerView<'a>> {
        let grouped = attempts_by_name(statuses);
        spec_containers
            .iter()
            .map(|container| {
                let c_name = container_name_of(container);
                let attempts = grouped.get(c_name).map_or(&[][..], Vec::as_slice);
                ContainerView {
                    spec: container,
                    latest: attempts.last().copied(),
                    previous: attempts
                        .len()
                        .checked_sub(2)
                        .and_then(|i| attempts.get(i))
                        .copied(),
                    ready: state.ready.get(c_name).copied().unwrap_or(false),
                    backoff: state.backoffs.get(c_name).cloned(),
                    waiting: state.waiting.get(c_name).cloned(),
                    cpu: Some(self.cpu_assignment(namespace, pod_name(pod), pod, container)),
                }
            })
            .collect()
    }

    /// Decides what one spec container needs this pass: `Some(attempt)` to start a
    /// new attempt, `None` to leave it alone. Probes running containers and stops
    /// one whose liveness probe failed; records readiness and back-off in `state`.
    async fn next_attempt(
        &self,
        namespace: &str,
        pod: &Value,
        container: &Value,
        latest: Option<&cri::ContainerStatus>,
        state: &mut SyncState,
    ) -> Result<Option<u32>, KubeletError> {
        let c_name = container_name_of(container);
        let policy = RestartPolicy::of(pod);
        let Some(latest) = latest else {
            return Ok(Some(0));
        };
        match state_of(latest) {
            cri::ContainerState::ContainerCreated => {
                self.runtime.start_container(&latest.id).await?;
                state.started_now.insert(c_name.to_string());
                Ok(None)
            },
            cri::ContainerState::ContainerRunning => {
                let (live, is_ready) = self
                    .evaluate_container_probes(&latest.id, container)
                    .await?;
                if live || policy == RestartPolicy::Never {
                    state.ready.insert(c_name.to_string(), is_ready);
                    return Ok(None);
                }
                self.runtime.stop_container(&latest.id, 0).await?;
                state.restarted.insert(c_name.to_string());
                Ok(self.schedule_restart(namespace, pod, c_name, latest, state))
            },
            cri::ContainerState::ContainerExited if policy.restarts(latest.exit_code) => {
                Ok(self.schedule_restart(namespace, pod, c_name, latest, state))
            },
            cri::ContainerState::ContainerExited | cri::ContainerState::ContainerUnknown => {
                Ok(None)
            },
        }
    }

    /// Next attempt number when the back-off allows a restart now; otherwise
    /// records the `CrashLoopBackOff` message and returns `None`.
    fn schedule_restart(
        &self,
        namespace: &str,
        pod: &Value,
        c_name: &str,
        latest: &cri::ContainerStatus,
        state: &mut SyncState,
    ) -> Option<u32> {
        let uid = pod
            .pointer("/metadata/uid")
            .and_then(Value::as_str)
            .unwrap_or("");
        match self.restart_decision(namespace, pod_name(pod), uid, c_name, latest) {
            Ok(attempt) => Some(attempt),
            Err(message) => {
                state.backoffs.insert(c_name.to_string(), message);
                None
            },
        }
    }

    /// Readiness of containers started this pass. A container restarted after a
    /// failed liveness probe is not ready until the next pass proves it; a fresh
    /// start is probed right away.
    async fn probe_started(
        &self,
        spec_containers: &[Value],
        statuses: &[cri::ContainerStatus],
        state: &mut SyncState,
    ) -> Result<(), KubeletError> {
        let grouped = attempts_by_name(statuses);
        for container in spec_containers {
            let c_name = container_name_of(container);
            if !state.started_now.contains(c_name) {
                continue;
            }
            let latest = grouped.get(c_name).and_then(|a| a.last());
            let is_ready = match latest {
                Some(latest) if !state.restarted.contains(c_name) => {
                    self.evaluate_container_probes(&latest.id, container)
                        .await?
                        .1
                },
                _ => false,
            };
            state.ready.insert(c_name.to_string(), is_ready);
        }
        Ok(())
    }

    /// Keeps the current and previous attempt of each container; older ones only hold stale logs.
    async fn prune_attempts(&self, statuses: &[cri::ContainerStatus]) {
        for attempts in attempts_by_name(statuses).values() {
            let stale = attempts.len().saturating_sub(KEPT_ATTEMPTS);
            for old in attempts.iter().take(stale) {
                let _ = self.runtime.remove_container(&old.id).await;
            }
        }
    }

    fn spawn_stop(&self, id: String, grace: i64) {
        // StopContainer blocks while TERM and then KILL run; keep the loop moving.
        let runtime = self.runtime.clone();
        tokio::spawn(async move {
            if let Err(e) = runtime.stop_container(&id, grace).await {
                eprintln!("failed to stop container {id}: {e}");
            }
        });
    }

    /// Drives a pod carrying `deletionTimestamp`: stop its containers with the
    /// remaining grace period (TERM, then KILL), then record the final status,
    /// remove the sandbox and delete the Pod object with a zero grace period.
    async fn terminate_pod(&self, namespace: &str, pod: &Value) -> Result<(), KubeletError> {
        let name = pod_name(pod);
        let uid = pod_uid(pod)?;
        let deadline = pod
            .pointer("/metadata/deletionTimestamp")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_seconds)
            .unwrap_or(0);
        let sandboxes = self
            .runtime
            .list_pod_sandbox(Some(&uid_filter(uid)))
            .await?;
        let mut statuses = Vec::new();
        for sandbox in &sandboxes {
            statuses.extend(self.container_statuses(&sandbox.id).await?);
        }
        let running: Vec<String> = statuses
            .iter()
            .filter(|s| state_of(s) == cri::ContainerState::ContainerRunning)
            .map(|s| s.id.clone())
            .collect();
        if !running.is_empty() {
            let first_pass = self
                .terminating
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(uid.to_string());
            if first_pass {
                let grace = i64::try_from(deadline.saturating_sub(now_unix())).unwrap_or(0);
                for id in running {
                    self.spawn_stop(id, grace);
                }
            }
            return Ok(());
        }
        let grouped = attempts_by_name(&statuses);
        let empty = Vec::new();
        let views: Vec<ContainerView<'_>> = pod
            .pointer("/spec/containers")
            .and_then(Value::as_array)
            .unwrap_or(&empty)
            .iter()
            .map(|container| {
                let c_name = container
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("main");
                let attempts = grouped.get(c_name).map_or(&[][..], Vec::as_slice);
                ContainerView {
                    spec: container,
                    latest: attempts.last().copied(),
                    previous: None,
                    ready: false,
                    backoff: None,
                    waiting: None,
                    cpu: None,
                }
            })
            .collect();
        let status = pod_status(
            pod,
            self.runtime.provider_name(),
            &self.node_ip,
            None,
            false,
            RestartPolicy::Never,
            &views,
        );
        self.patch_status_if_changed(namespace, name, pod, status)
            .await?;
        for sandbox in &sandboxes {
            self.remove_sandbox_tree(&sandbox.id).await?;
        }
        self.finish_deletion(namespace, name, uid).await
    }

    async fn finish_deletion(
        &self,
        namespace: &str,
        name: &str,
        uid: &str,
    ) -> Result<(), KubeletError> {
        self.terminating
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(uid);
        self.backoff
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|key, _| !key.starts_with(&format!("{uid}/")));
        match self
            .client
            .delete_pod_options(namespace, name, Some(0))
            .await
        {
            Ok(_) | Err(ApiserverError::NotFound { .. }) => Ok(()),
            Err(e) => Err(KubeletError::PodReconciliationFailed {
                pod: format!("{namespace}/{name}"),
                reason: format!("failed to delete terminated pod: {e}"),
            }),
        }
    }

    async fn patch_status_if_changed(
        &self,
        namespace: &str,
        name: &str,
        pod: &Value,
        status: Value,
    ) -> Result<(), KubeletError> {
        if pod.get("status") == Some(&status) {
            return Ok(());
        }
        self.client
            .patch_pod_status(namespace, name, status)
            .await
            .map(drop)
            .map_err(|e| KubeletError::PodReconciliationFailed {
                pod: format!("{namespace}/{name}"),
                reason: format!("failed to patch pod status: {e}"),
            })
    }

    async fn evaluate_probe(
        &self,
        container_id: &str,
        probe: &Value,
    ) -> Result<bool, KubeletError> {
        let command: Option<Vec<String>> = if let Some(exec) = probe.get("exec") {
            exec.get("command").and_then(Value::as_array).map(|cmd| {
                cmd.iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
        } else if let Some(http) = probe.get("httpGet") {
            let path = http.get("path").and_then(Value::as_str).unwrap_or("/");
            Some(vec!["curl".to_string(), path.to_string()])
        } else if let Some(tcp) = probe.get("tcpSocket") {
            let port = tcp.get("port").and_then(Value::as_i64).unwrap_or(80);
            Some(vec![
                "nc".to_string(),
                "-z".to_string(),
                "127.0.0.1".to_string(),
                port.to_string(),
            ])
        } else {
            None
        };
        let Some(command) = command else {
            return Ok(true);
        };
        let exec_probe = probe.get("exec").is_some();
        match self
            .runtime
            .exec_sync(container_id, &command, EXEC_TIMEOUT_SECS)
            .await
        {
            Ok(result) => Ok(result.exit_code == 0),
            Err(e) if exec_probe => Err(e),
            // HTTP and TCP probes are emulated through exec; treat a missing tool as unknown.
            Err(_) => Ok(true),
        }
    }

    async fn evaluate_container_probes(
        &self,
        container_id: &str,
        container: &Value,
    ) -> Result<(bool, bool), KubeletError> {
        let mut liveness_ok = true;
        if let Some(liveness) = container.get("livenessProbe") {
            liveness_ok = self.evaluate_probe(container_id, liveness).await?;
        }
        let mut readiness_ok = true;
        if !liveness_ok {
            readiness_ok = false;
        } else if let Some(readiness) = container.get("readinessProbe") {
            readiness_ok = self.evaluate_probe(container_id, readiness).await?;
        }
        Ok((liveness_ok, readiness_ok))
    }

    async fn stage_projected_source(
        &self,
        vol_dir: &Path,
        namespace: &str,
        pod: &Value,
        source: &Value,
    ) -> Result<(), KubeletError> {
        if let Some(sa_tok) = source.get("serviceAccountToken") {
            stage_projected_sa_token(vol_dir, namespace, pod, sa_tok, self.apiserver.as_ref())?;
        }

        if let Some(cm) = source.get("configMap") {
            let cm_name = cm.get("name").and_then(Value::as_str).unwrap_or("");
            if let Ok(cm_obj) = self.client.get_configmap(namespace, cm_name).await {
                let _ = stage_configmap_files(vol_dir, cm, &cm_obj);
            }
        }

        if let Some(sec) = source.get("secret") {
            let sec_name = sec.get("name").and_then(Value::as_str).unwrap_or("");
            if let Ok(sec_obj) = self.client.get_secret(namespace, sec_name).await {
                let _ = stage_secret_files(vol_dir, sec, &sec_obj);
            }
        }

        if let Some(dw) = source.get("downwardAPI")
            && let Some(items) = dw.get("items").and_then(Value::as_array)
        {
            stage_projected_downward_api(vol_dir, namespace, pod, items)?;
        }

        Ok(())
    }

    async fn prepare_single_volume(
        &self,
        namespace: &str,
        pod_name: &str,
        pod_uid: &str,
        pod: &Value,
        vol: &Value,
    ) -> Result<(), KubeletError> {
        let vol_name = vol.get("name").and_then(Value::as_str).unwrap_or("unnamed");

        if let Some(sec) = vol.get("secret") {
            let vol_dir = self.get_pod_volume_dir(pod_uid, "kubernetes.io~secret", vol_name);
            std::fs::create_dir_all(&vol_dir).map_err(|e| {
                KubeletError::PodReconciliationFailed {
                    pod: pod_name.to_string(),
                    reason: format!(
                        "failed to create volume directory {}: {e}",
                        vol_dir.display()
                    ),
                }
            })?;
            let sec_name = sec
                .get("secretName")
                .and_then(Value::as_str)
                .unwrap_or(vol_name);
            let sec_obj = self
                .client
                .get_secret(namespace, sec_name)
                .await
                .map_err(|e| KubeletError::PodReconciliationFailed {
                    pod: pod_name.to_string(),
                    reason: format!("failed to fetch secret '{sec_name}': {e}"),
                })?;
            stage_secret_files(&vol_dir, sec, &sec_obj)?;
        } else if let Some(cm) = vol.get("configMap") {
            let vol_dir = self.get_pod_volume_dir(pod_uid, "kubernetes.io~configmap", vol_name);
            std::fs::create_dir_all(&vol_dir).map_err(|e| {
                KubeletError::PodReconciliationFailed {
                    pod: pod_name.to_string(),
                    reason: format!(
                        "failed to create volume directory {}: {e}",
                        vol_dir.display()
                    ),
                }
            })?;
            let cm_name = cm.get("name").and_then(Value::as_str).unwrap_or(vol_name);
            let cm_obj = self
                .client
                .get_configmap(namespace, cm_name)
                .await
                .map_err(|e| KubeletError::PodReconciliationFailed {
                    pod: pod_name.to_string(),
                    reason: format!("failed to fetch configmap '{cm_name}': {e}"),
                })?;
            stage_configmap_files(&vol_dir, cm, &cm_obj)?;
        } else if let Some(proj) = vol.get("projected")
            && let Some(sources) = proj.get("sources").and_then(Value::as_array)
        {
            let vol_dir = self.get_pod_volume_dir(pod_uid, "kubernetes.io~projected", vol_name);
            std::fs::create_dir_all(&vol_dir).map_err(|e| {
                KubeletError::PodReconciliationFailed {
                    pod: pod_name.to_string(),
                    reason: format!(
                        "failed to create volume directory {}: {e}",
                        vol_dir.display()
                    ),
                }
            })?;
            for source in sources {
                self.stage_projected_source(&vol_dir, namespace, pod, source)
                    .await?;
            }
        }

        Ok(())
    }

    /// Prepares and populates volume mounts (Secrets, `ConfigMaps`, Projected) on the host filesystem.
    pub async fn prepare_pod_volumes(
        &self,
        namespace: &str,
        pod: &Value,
    ) -> Result<(), KubeletError> {
        let name = pod
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let pod_uid = pod
            .get("metadata")
            .and_then(|m| m.get("uid"))
            .and_then(Value::as_str)
            .unwrap_or(name);

        let Some(volumes) = pod
            .get("spec")
            .and_then(|s| s.get("volumes"))
            .and_then(Value::as_array)
        else {
            return Ok(());
        };

        for vol in volumes {
            self.prepare_single_volume(namespace, name, pod_uid, pod, vol)
                .await?;
        }

        Ok(())
    }
}
