use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use rubix_apiserver::{ApiserverService, KubernetesApiClient};

use crate::config::{KubeletConfigOptions, detect_host_cpu_count, format_cpuset, parse_cpuset};
use crate::error::KubeletError;

/// Result of executing a command in a container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Observed state of one container, as reported by the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerRuntimeState {
    /// Created or otherwise not yet executing.
    Waiting { reason: String },
    /// The container process is alive.
    Running { started_at: String },
    /// The container process has exited.
    Terminated {
        exit_code: i32,
        started_at: Option<String>,
        finished_at: String,
    },
}

/// Observed status of one container belonging to a pod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerRuntimeStatus {
    /// Container name from the pod spec.
    pub name: String,
    /// Runtime-qualified identifier of the real container, such as `podman://<id>`.
    pub container_id: String,
    pub image: String,
    pub image_id: String,
    pub state: ContainerRuntimeState,
}

/// Everything the runtime currently holds for one pod.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PodRuntimeStatus {
    pub containers: Vec<ContainerRuntimeStatus>,
}

/// A pod the runtime still holds containers for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedPodRef {
    pub pod_id: String,
    pub namespace: String,
    pub name: String,
    pub uid: String,
}

/// Summary of one pass over every pod in the cluster.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub bound: usize,
    pub synced: usize,
    pub failed: usize,
    pub orphans_stopped: usize,
}

/// Abstract interface for container runtime providers executing workloads.
#[async_trait]
pub trait RuntimeProvider: std::fmt::Debug + Send + Sync {
    /// Identifier for the runtime provider (e.g. "containerd", "cri-o").
    fn provider_name(&self) -> &str;

    /// Version string reported in the node's `containerRuntimeVersion`.
    fn runtime_version(&self) -> String {
        "v1.35.7".to_string()
    }

    /// Verifies the runtime answers before the node reports Ready.
    async fn check_available(&self) -> Result<(), KubeletError> {
        Ok(())
    }

    /// Reports the live containers of `pod`, or `None` when the provider does not
    /// observe real processes and the reconciler must assume the pod runs.
    async fn inspect_pod(&self, pod: &Value) -> Result<Option<PodRuntimeStatus>, KubeletError> {
        let _ = pod;
        Ok(None)
    }

    /// Lists pods the runtime still holds containers for, so containers whose
    /// Pod object was deleted can be stopped.
    async fn list_managed_pods(&self) -> Result<Vec<ManagedPodRef>, KubeletError> {
        Ok(Vec::new())
    }

    /// Whether this provider requires a live unix domain socket path to exist on the host filesystem.
    fn requires_socket(&self) -> bool {
        false
    }

    /// Whether this provider represents an external runtime whose containers survive Kubelet restart.
    fn is_external(&self) -> bool {
        false
    }

    /// Runs a pod sandbox and starts its containers.
    async fn run_pod(&self, pod: &Value) -> Result<String, KubeletError>;

    /// Stops a running pod sandbox and its containers.
    async fn stop_pod(&self, pod_id: &str) -> Result<(), KubeletError>;

    /// Queries the status of an active pod sandbox.
    async fn get_pod_status(&self, pod_id: &str) -> Result<String, KubeletError>;

    /// Retrieves container logs.
    async fn get_container_logs(
        &self,
        pod_id: &str,
        container_name: &str,
        tail_lines: Option<usize>,
    ) -> Result<String, KubeletError> {
        let _ = (pod_id, container_name, tail_lines);
        Ok(String::new())
    }

    /// Executes a command in a running container.
    async fn exec_in_container(
        &self,
        pod_id: &str,
        container_name: &str,
        cmd: &[String],
    ) -> Result<ExecResult, KubeletError> {
        let _ = (pod_id, container_name, cmd);
        Ok(ExecResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

#[derive(Debug, Default)]
struct MockRuntimeState {
    logs: BTreeMap<String, String>,
    exec_responses: BTreeMap<String, ExecResult>,
    probe_results: BTreeMap<String, bool>,
    active_pods: BTreeSet<String>,
}

/// Simulated in-memory runtime provider for testing managed and external runtime engines.
#[derive(Debug)]
pub struct MockRuntimeProvider {
    name: String,
    pod_counter: AtomicUsize,
    is_external: bool,
    state: Arc<std::sync::Mutex<MockRuntimeState>>,
}

impl MockRuntimeProvider {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pod_counter: AtomicUsize::new(1),
            is_external: false,
            state: Arc::new(std::sync::Mutex::new(MockRuntimeState::default())),
        }
    }

    #[must_use]
    pub fn new_external(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pod_counter: AtomicUsize::new(1),
            is_external: true,
            state: Arc::new(std::sync::Mutex::new(MockRuntimeState::default())),
        }
    }

    pub fn set_external(&mut self, external: bool) {
        self.is_external = external;
    }

    pub fn set_container_logs(&self, key: impl Into<String>, logs: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.logs.insert(key.into(), logs.into());
        }
    }

    pub fn set_exec_response(&self, key: impl Into<String>, response: ExecResult) {
        if let Ok(mut state) = self.state.lock() {
            state.exec_responses.insert(key.into(), response);
        }
    }

    pub fn set_probe_result(&self, key: impl Into<String>, success: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.probe_results.insert(key.into(), success);
        }
    }

    pub fn is_pod_active(&self, pod_id: &str) -> bool {
        self.state
            .lock()
            .is_ok_and(|s| s.active_pods.contains(pod_id))
    }
}

#[async_trait]
impl RuntimeProvider for MockRuntimeProvider {
    fn provider_name(&self) -> &str {
        &self.name
    }

    fn is_external(&self) -> bool {
        self.is_external
    }

    async fn run_pod(&self, pod: &Value) -> Result<String, KubeletError> {
        let name = pod
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("unnamed");

        let id = self.pod_counter.fetch_add(1, Ordering::SeqCst);
        let pod_id = format!("{}-{}-{}", self.name, name, id);
        if let Ok(mut state) = self.state.lock() {
            state.active_pods.insert(pod_id.clone());
        }
        Ok(pod_id)
    }

    async fn stop_pod(&self, pod_id: &str) -> Result<(), KubeletError> {
        if let Ok(mut state) = self.state.lock() {
            state.active_pods.remove(pod_id);
        }
        Ok(())
    }

    async fn get_pod_status(&self, pod_id: &str) -> Result<String, KubeletError> {
        let active = self
            .state
            .lock()
            .map_or(true, |s| s.active_pods.contains(pod_id));
        if active {
            Ok("Running".to_string())
        } else {
            Ok("Stopped".to_string())
        }
    }

    async fn get_container_logs(
        &self,
        pod_id: &str,
        container_name: &str,
        tail_lines: Option<usize>,
    ) -> Result<String, KubeletError> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key_full = format!("{pod_id}:{container_name}");
        let raw_logs = state
            .logs
            .get(&key_full)
            .or_else(|| state.logs.get(container_name))
            .cloned()
            .unwrap_or_else(|| format!("container {container_name} is running in {pod_id}\n"));

        if let Some(n) = tail_lines {
            let lines: Vec<&str> = raw_logs.lines().collect();
            let start = lines.len().saturating_sub(n);
            let tailed = lines[start..].join("\n");
            if raw_logs.ends_with('\n') && !tailed.is_empty() {
                Ok(format!("{tailed}\n"))
            } else {
                Ok(tailed)
            }
        } else {
            Ok(raw_logs)
        }
    }

    async fn exec_in_container(
        &self,
        _pod_id: &str,
        container_name: &str,
        cmd: &[String],
    ) -> Result<ExecResult, KubeletError> {
        let cmd_str = cmd.join(" ");
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key_full = format!("{container_name}:{cmd_str}");

        if let Some(res) = state
            .exec_responses
            .get(&key_full)
            .or_else(|| state.exec_responses.get(&cmd_str))
            .or_else(|| cmd.first().and_then(|c| state.exec_responses.get(c)))
        {
            return Ok(res.clone());
        }

        if let Some(&pass) = state
            .probe_results
            .get(&key_full)
            .or_else(|| state.probe_results.get(&cmd_str))
            .or_else(|| state.probe_results.get(container_name))
        {
            return Ok(ExecResult {
                exit_code: i32::from(!pass),
                stdout: if pass {
                    "probe succeeded\n".to_string()
                } else {
                    "probe failed\n".to_string()
                },
                stderr: if pass {
                    String::new()
                } else {
                    "failure\n".to_string()
                },
            });
        }

        if cmd.first().is_some_and(|c| c == "echo") {
            let out = cmd[1..].join(" ");
            return Ok(ExecResult {
                exit_code: 0,
                stdout: format!("{out}\n"),
                stderr: String::new(),
            });
        }

        Ok(ExecResult {
            exit_code: 0,
            stdout: format!("executed: {cmd_str}\n"),
            stderr: String::new(),
        })
    }
}

/// Kubernetes Quality of Service (`QoS`) classes for pods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PodQoSClass {
    Guaranteed,
    Burstable,
    BestEffort,
}

/// Parses a CPU quantity string into millicores.
/// Supports integers ("1" -> 1000), millicores ("500m" -> 500), and decimals ("1.5" -> 1500).
#[must_use]
pub fn parse_cpu_quantity_milli(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_suffix('m') {
        rest.parse::<u64>().ok()
    } else if let Ok(val) = s.parse::<u64>() {
        val.checked_mul(1000)
    } else if let Ok(f) = s.parse::<f64>() {
        #[allow(clippy::cast_precision_loss)]
        let max_f64 = (u64::MAX / 1000) as f64;
        if f.is_sign_negative() || f.is_nan() || f.is_infinite() || f > max_f64 {
            None
        } else {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let val = (f * 1000.0).round() as u64;
            Some(val)
        }
    } else {
        None
    }
}

/// Parses a memory quantity string into bytes.
#[must_use]
pub fn parse_memory_quantity_bytes(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(bytes) = s.parse::<u64>() {
        return Some(bytes);
    }
    let (num_str, mult) = if let Some(r) = s.strip_suffix("Ki") {
        (r, 1024u64)
    } else if let Some(r) = s.strip_suffix("Mi") {
        (r, 1024u64.saturating_pow(2))
    } else if let Some(r) = s.strip_suffix("Gi") {
        (r, 1024u64.saturating_pow(3))
    } else if let Some(r) = s.strip_suffix("Ti") {
        (r, 1024u64.saturating_pow(4))
    } else if let Some(r) = s.strip_suffix('k') {
        (r, 1000u64)
    } else if let Some(r) = s.strip_suffix('M') {
        (r, 1_000_000u64)
    } else {
        let r = s.strip_suffix('G')?;
        (r, 1_000_000_000u64)
    };
    num_str.parse::<u64>().ok().map(|n| n.saturating_mul(mult))
}

/// Determines the `QoS` class of a pod based on its container resource requirements.
#[must_use]
pub fn determine_pod_qos(pod: &Value) -> PodQoSClass {
    let containers = pod
        .get("spec")
        .and_then(|s| s.get("containers"))
        .and_then(Value::as_array);

    let Some(containers) = containers else {
        return PodQoSClass::BestEffort;
    };

    if containers.is_empty() {
        return PodQoSClass::BestEffort;
    }

    let mut has_requests_or_limits = false;
    let mut all_guaranteed = true;

    for container in containers {
        let resources = container.get("resources");
        let requests = resources.and_then(|r| r.get("requests"));
        let limits = resources.and_then(|r| r.get("limits"));

        let cpu_req = requests.and_then(|r| r.get("cpu")).and_then(Value::as_str);
        let cpu_lim = limits.and_then(|l| l.get("cpu")).and_then(Value::as_str);
        let mem_req = requests
            .and_then(|r| r.get("memory"))
            .and_then(Value::as_str);
        let mem_lim = limits.and_then(|l| l.get("memory")).and_then(Value::as_str);

        if cpu_req.is_some() || cpu_lim.is_some() || mem_req.is_some() || mem_lim.is_some() {
            has_requests_or_limits = true;
        }

        // For Guaranteed:
        // CPU and memory limits must be set and > 0.
        // Requests must either equal limits, or if omitted, default to limits.
        let is_cpu_guaranteed = match (cpu_req, cpu_lim) {
            (Some(req), Some(lim)) => {
                let req_m = parse_cpu_quantity_milli(req);
                let lim_m = parse_cpu_quantity_milli(lim);
                req_m.is_some() && req_m == lim_m && req_m.unwrap_or(0) > 0
            },
            (None, Some(lim)) => parse_cpu_quantity_milli(lim).is_some_and(|m| m > 0),
            _ => false,
        };

        let is_mem_guaranteed = match (mem_req, mem_lim) {
            (Some(req), Some(lim)) => {
                let req_b = parse_memory_quantity_bytes(req);
                let lim_b = parse_memory_quantity_bytes(lim);
                req_b.is_some() && req_b == lim_b && req_b.unwrap_or(0) > 0
            },
            (None, Some(lim)) => parse_memory_quantity_bytes(lim).is_some_and(|b| b > 0),
            _ => false,
        };

        if !is_cpu_guaranteed || !is_mem_guaranteed {
            all_guaranteed = false;
        }
    }

    if all_guaranteed {
        PodQoSClass::Guaranteed
    } else if has_requests_or_limits {
        PodQoSClass::Burstable
    } else {
        PodQoSClass::BestEffort
    }
}

/// Checks whether a container in a pod is eligible for exclusive CPU allocation.
///
/// Returns `Some(cores_count)` if eligible, or `None` if the container runs in the shared pool.
/// Requirements:
/// 1. Pod must have Guaranteed `QoS`.
/// 2. Container must have an integer number of CPU cores requested (e.g. 1, 2, 1000m, 2000m).
#[must_use]
pub fn is_container_cpu_pinning_eligible(qos: PodQoSClass, container: &Value) -> Option<usize> {
    if qos != PodQoSClass::Guaranteed {
        return None;
    }

    let resources = container.get("resources")?;
    let cpu_str = resources
        .get("requests")
        .and_then(|r| r.get("cpu"))
        .or_else(|| resources.get("limits").and_then(|l| l.get("cpu")))?
        .as_str()?;

    let milli = parse_cpu_quantity_milli(cpu_str)?;
    if milli >= 1000 && milli % 1000 == 0 {
        Some((milli / 1000) as usize)
    } else {
        None
    }
}

/// CPU manager persisted state in `<root-dir>/cpu_manager_state`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CpuManagerState {
    #[serde(rename = "policyName")]
    pub policy_name: String,
    #[serde(rename = "defaultCpuSet")]
    pub default_cpuset: String,
    #[serde(rename = "entries", default)]
    pub entries: BTreeMap<String, String>,
}

/// CPU manager coordinating exclusive CPU core allocations and checkpoint persistence.
#[derive(Debug, Clone)]
pub struct CpuManager {
    policy: String,
    policy_options: BTreeMap<String, String>,
    reserved_cpus: BTreeSet<usize>,
    host_cpus: usize,
    checkpoint_path: PathBuf,
    allocations: Arc<std::sync::Mutex<BTreeMap<String, BTreeSet<usize>>>>,
}

impl CpuManager {
    #[must_use]
    pub fn new(
        policy: impl Into<String>,
        policy_options: BTreeMap<String, String>,
        reserved_cpus: BTreeSet<usize>,
        host_cpus: usize,
        checkpoint_path: PathBuf,
    ) -> Self {
        let policy_str = policy.into();
        let allocations = Arc::new(std::sync::Mutex::new(BTreeMap::new()));

        if policy_str == "static"
            && checkpoint_path.exists()
            && let Ok(content) = std::fs::read_to_string(&checkpoint_path)
            && let Ok(state) = serde_json::from_str::<CpuManagerState>(&content)
            && state.policy_name == "static"
            && let Ok(mut map) = allocations.lock()
        {
            for (container_id, cpuset_str) in state.entries {
                if let Ok(cpus) = parse_cpuset(&cpuset_str) {
                    map.insert(container_id, cpus);
                }
            }
        }

        Self {
            policy: policy_str,
            policy_options,
            reserved_cpus,
            host_cpus,
            checkpoint_path,
            allocations,
        }
    }

    #[must_use]
    pub fn from_options(options: &KubeletConfigOptions) -> Self {
        let host_cpus = detect_host_cpu_count();
        let reserved = if !options.reserved_cpus.is_empty() {
            parse_cpuset(&options.reserved_cpus).unwrap_or_default()
        } else if options.cpu_manager_policy == "static" {
            let count = options
                .system_reserved
                .get("cpu")
                .and_then(|q| parse_cpu_quantity_milli(q))
                .map_or(1, |m| usize::try_from(m.div_ceil(1000)).unwrap_or(1))
                .min(host_cpus)
                .max(1);
            (0..count).collect()
        } else {
            BTreeSet::new()
        };

        Self::new(
            &options.cpu_manager_policy,
            options.cpu_manager_policy_options.clone(),
            reserved,
            host_cpus,
            options.cpu_manager_checkpoint_path(),
        )
    }

    #[must_use]
    pub fn policy(&self) -> &str {
        &self.policy
    }

    #[must_use]
    pub fn host_cpus(&self) -> usize {
        self.host_cpus
    }

    #[must_use]
    pub fn reserved_cpus(&self) -> &BTreeSet<usize> {
        &self.reserved_cpus
    }

    #[must_use]
    pub fn policy_options(&self) -> &BTreeMap<String, String> {
        &self.policy_options
    }

    /// Returns the currently allocated exclusive CPUs for all containers.
    pub fn current_allocations(&self) -> BTreeMap<String, BTreeSet<usize>> {
        self.allocations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Drops all in-memory allocations (e.g. after checkpoint invalidation).
    pub fn reset(&self) {
        self.allocations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// Computes the shared CPU pool (all host CPUs excluding reserved and exclusively allocated cores).
    pub fn shared_pool(&self) -> BTreeSet<usize> {
        let map = self
            .allocations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut shared = BTreeSet::new();
        for cpu in 0..self.host_cpus {
            if !self.reserved_cpus.contains(&cpu) {
                let allocated = map.values().any(|set| set.contains(&cpu));
                if !allocated {
                    shared.insert(cpu);
                }
            }
        }
        shared
    }

    /// Allocates `count` exclusive CPUs for `container_key`.
    pub fn allocate_exclusive_cpus(
        &self,
        container_key: &str,
        count: usize,
    ) -> Result<BTreeSet<usize>, KubeletError> {
        if self.policy != "static" {
            return Ok(BTreeSet::new());
        }

        let mut map =
            self.allocations
                .lock()
                .map_err(|e| KubeletError::PodReconciliationFailed {
                    pod: container_key.to_string(),
                    reason: format!("CPU manager lock poisoned: {e}"),
                })?;

        if let Some(existing) = map.get(container_key) {
            return Ok(existing.clone());
        }

        let mut available: Vec<usize> = Vec::new();
        for cpu in 0..self.host_cpus {
            if !self.reserved_cpus.contains(&cpu) && !map.values().any(|set| set.contains(&cpu)) {
                available.push(cpu);
            }
        }

        if available.len() < count {
            return Err(KubeletError::PodReconciliationFailed {
                pod: container_key.to_string(),
                reason: format!(
                    "insufficient exclusive CPUs available: requested {count}, available {}",
                    available.len()
                ),
            });
        }

        let allocated: BTreeSet<usize> = available.into_iter().take(count).collect();
        map.insert(container_key.to_string(), allocated.clone());

        // Update checkpoint
        self.persist_checkpoint_locked(&map)?;

        Ok(allocated)
    }

    /// Releases exclusive CPUs for `container_key`.
    pub fn release_exclusive_cpus(&self, container_key: &str) -> Result<(), KubeletError> {
        if self.policy != "static" {
            return Ok(());
        }

        let mut map =
            self.allocations
                .lock()
                .map_err(|e| KubeletError::PodReconciliationFailed {
                    pod: container_key.to_string(),
                    reason: format!("CPU manager lock poisoned: {e}"),
                })?;

        if map.remove(container_key).is_some() {
            self.persist_checkpoint_locked(&map)?;
        }
        Ok(())
    }

    fn persist_checkpoint_locked(
        &self,
        map: &BTreeMap<String, BTreeSet<usize>>,
    ) -> Result<(), KubeletError> {
        let mut shared = BTreeSet::new();
        for cpu in 0..self.host_cpus {
            if !self.reserved_cpus.contains(&cpu) {
                let allocated = map.values().any(|set| set.contains(&cpu));
                if !allocated {
                    shared.insert(cpu);
                }
            }
        }

        let mut entries = BTreeMap::new();
        for (k, set) in map {
            entries.insert(k.clone(), format_cpuset(set));
        }

        let state = CpuManagerState {
            policy_name: self.policy.clone(),
            default_cpuset: format_cpuset(&shared),
            entries,
        };

        if let Some(parent) = self.checkpoint_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let json = serde_json::to_string_pretty(&state).map_err(|e| {
            KubeletError::InvalidConfiguration {
                field: "checkpoint".to_string(),
                reason: format!("failed to serialize cpu_manager_state: {e}"),
            }
        })?;

        std::fs::write(&self.checkpoint_path, json).map_err(|e| {
            KubeletError::InvalidConfiguration {
                field: "checkpoint".to_string(),
                reason: format!("failed to write cpu_manager_state: {e}"),
            }
        })?;

        Ok(())
    }
}

/// Diagnostic report for an active container that needs restart after a CPU manager checkpoint invalidation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadRestartReport {
    pub namespace: String,
    pub pod_name: String,
    pub container_name: String,
    pub container_id: String,
    pub reason: String,
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

    pub async fn get_container_logs(
        &self,
        pod_id: &str,
        container_name: &str,
        tail_lines: Option<usize>,
    ) -> Result<String, KubeletError> {
        self.runtime
            .get_container_logs(pod_id, container_name, tail_lines)
            .await
    }

    pub async fn exec_in_container(
        &self,
        pod_id: &str,
        container_name: &str,
        cmd: &[String],
    ) -> Result<ExecResult, KubeletError> {
        self.runtime
            .exec_in_container(pod_id, container_name, cmd)
            .await
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

        let pod_list = self
            .client
            .list_pods(namespace)
            .await
            .map_err(KubeletError::from)?;

        let Some(items) = pod_list.get("items").and_then(Value::as_array) else {
            return Ok(reports);
        };

        for pod in items {
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

    async fn evaluate_probe(
        &self,
        sandbox_id: &str,
        container_name: &str,
        probe: &Value,
    ) -> Result<bool, KubeletError> {
        if let Some(exec) = probe.get("exec")
            && let Some(cmd_val) = exec.get("command").and_then(Value::as_array)
        {
            let cmd: Vec<String> = cmd_val
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect();
            let res = self
                .runtime
                .exec_in_container(sandbox_id, container_name, &cmd)
                .await?;
            return Ok(res.exit_code == 0);
        }

        if let Some(http) = probe.get("httpGet") {
            let path = http.get("path").and_then(Value::as_str).unwrap_or("/");
            let cmd = vec!["curl".to_string(), path.to_string()];
            let res = self
                .runtime
                .exec_in_container(sandbox_id, container_name, &cmd)
                .await;
            if let Ok(r) = res {
                return Ok(r.exit_code == 0);
            }
        }

        if let Some(tcp) = probe.get("tcpSocket") {
            let port = tcp.get("port").and_then(Value::as_i64).unwrap_or(80);
            let cmd = vec![
                "nc".to_string(),
                "-z".to_string(),
                "127.0.0.1".to_string(),
                port.to_string(),
            ];
            let res = self
                .runtime
                .exec_in_container(sandbox_id, container_name, &cmd)
                .await;
            if let Ok(r) = res {
                return Ok(r.exit_code == 0);
            }
        }

        Ok(true)
    }

    async fn evaluate_container_probes(
        &self,
        sandbox_id: &str,
        c_name: &str,
        c: &Value,
    ) -> Result<(bool, bool), KubeletError> {
        let mut liveness_ok = true;
        if let Some(liveness) = c.get("livenessProbe") {
            liveness_ok = self.evaluate_probe(sandbox_id, c_name, liveness).await?;
        }

        let mut readiness_ok = true;
        if !liveness_ok {
            readiness_ok = false;
        } else if let Some(readiness) = c.get("readinessProbe") {
            readiness_ok = self.evaluate_probe(sandbox_id, c_name, readiness).await?;
        }

        Ok((liveness_ok, readiness_ok))
    }

    fn build_container_statuses(
        &self,
        namespace: &str,
        sandbox_id: &str,
        pod_name: &str,
        qos: PodQoSClass,
        containers: &[Value],
    ) -> Vec<Value> {
        let mut container_statuses = Vec::new();
        for (idx, c) in containers.iter().enumerate() {
            let c_name = c.get("name").and_then(Value::as_str).unwrap_or("main");
            let c_image = c.get("image").and_then(Value::as_str).unwrap_or("unknown");
            let container_key = format!("{namespace}/{pod_name}/{c_name}");

            let (assigned_cpuset, is_exclusive) =
                if let Some(cores) = is_container_cpu_pinning_eligible(qos, c) {
                    match self
                        .cpu_manager
                        .allocate_exclusive_cpus(&container_key, cores)
                    {
                        Ok(cpus) => (format_cpuset(&cpus), true),
                        Err(e) => {
                            eprintln!("exclusive CPU allocation failed for {container_key}: {e}");
                            (format_cpuset(&self.cpu_manager.shared_pool()), false)
                        },
                    }
                } else {
                    (format_cpuset(&self.cpu_manager.shared_pool()), false)
                };

            let mut status_obj = json!({
                "name": c_name,
                "ready": true,
                "restartCount": 0,
                "image": c_image,
                "imageID": format!("{}-image-{}", self.runtime.provider_name(), c_image),
                "containerID": format!("{}://{}-c-{}", self.runtime.provider_name(), sandbox_id, idx),
                "cpuset": assigned_cpuset,
                "exclusiveCPU": is_exclusive,
                "state": {
                    "running": {
                        "startedAt": "2026-09-30T12:00:00Z"
                    }
                }
            });

            if is_exclusive {
                status_obj["allocatedResources"] = json!({
                    "cpu": assigned_cpuset
                });
            }

            container_statuses.push(status_obj);
        }
        container_statuses
    }

    async fn sync_running_pod(&self, namespace: &str, pod: &Value) -> Result<(), KubeletError> {
        let name = pod
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        let empty_containers = Vec::new();
        let containers = pod
            .get("spec")
            .and_then(|s| s.get("containers"))
            .and_then(Value::as_array)
            .map_or(&empty_containers[..], |c| &c[..]);

        let empty_statuses = Vec::new();
        let existing_statuses = pod
            .get("status")
            .and_then(|s| s.get("containerStatuses"))
            .and_then(Value::as_array)
            .map_or(&empty_statuses[..], |s| &s[..]);

        let sandbox_id = existing_statuses
            .first()
            .and_then(|cs| cs.get("containerID").and_then(Value::as_str))
            .and_then(|cid| {
                let after_slash = cid.split("://").nth(1)?;
                let (sb_id, _) = after_slash.rsplit_once("-c-")?;
                Some(sb_id.to_string())
            })
            .unwrap_or_else(|| format!("{}-{}-1", self.runtime.provider_name(), name));

        let restart_policy = pod
            .get("spec")
            .and_then(|s| s.get("restartPolicy"))
            .and_then(Value::as_str)
            .unwrap_or("Always");

        let mut updated_statuses = Vec::new();
        let mut all_ready = true;

        for (idx, c) in containers.iter().enumerate() {
            let c_name = c.get("name").and_then(Value::as_str).unwrap_or("main");
            let existing_status = existing_statuses
                .iter()
                .find(|s| s.get("name").and_then(Value::as_str) == Some(c_name));

            let mut restart_count = existing_status
                .and_then(|s| s.get("restartCount"))
                .and_then(Value::as_i64)
                .unwrap_or(0);

            let (liveness_ok, ready) = self
                .evaluate_container_probes(&sandbox_id, c_name, c)
                .await?;
            if !liveness_ok && restart_policy != "Never" {
                restart_count += 1;
                let _ = self.runtime.stop_pod(&sandbox_id).await;
            }

            if !ready {
                all_ready = false;
            }

            let mut st = existing_status.cloned().unwrap_or_else(|| {
                json!({
                    "name": c_name,
                    "image": c.get("image").and_then(Value::as_str).unwrap_or("unknown"),
                    "containerID": format!("{}://{}-c-{}", self.runtime.provider_name(), sandbox_id, idx),
                })
            });

            st["ready"] = json!(ready);
            st["restartCount"] = json!(restart_count);
            updated_statuses.push(st);
        }

        let status = json!({
            "phase": "Running",
            "conditions": build_pod_conditions(all_ready),
            "containerStatuses": updated_statuses
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

    /// Reconciles every pod in the cluster: binds unscheduled pods to this node,
    /// syncs the pods assigned here, and stops containers whose Pod is gone.
    pub async fn reconcile_all(&self) -> Result<ReconcileReport, KubeletError> {
        let pod_list = self
            .client
            .list_all_pods()
            .await
            .map_err(KubeletError::from)?;
        let mut report = ReconcileReport::default();
        let mut live_uids = BTreeSet::new();
        let empty = Vec::new();
        let items = pod_list
            .get("items")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for pod in items {
            if let Some(uid) = pod.pointer("/metadata/uid").and_then(Value::as_str) {
                live_uids.insert(uid.to_string());
            }
            let namespace = pod
                .pointer("/metadata/namespace")
                .and_then(Value::as_str)
                .unwrap_or("default")
                .to_string();
            let pod_name = pod
                .pointer("/metadata/name")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let assigned = pod.pointer("/spec/nodeName").and_then(Value::as_str);
            let pod = match assigned {
                Some(node) if node == self.node_name => pod.clone(),
                Some(_) => continue,
                None => match self.bind_pod_to_node(&namespace, pod).await {
                    Ok(bound) => {
                        report.bound += 1;
                        bound
                    },
                    Err(e) => {
                        report.failed += 1;
                        eprintln!("Failed to bind pod {namespace}/{pod_name}: {e}");
                        continue;
                    },
                },
            };
            match self.sync_pod(&namespace, &pod).await {
                Ok(()) => report.synced += 1,
                Err(e) => {
                    report.failed += 1;
                    eprintln!("Failed to sync pod {namespace}/{pod_name}: {e}");
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
        let name = pod
            .pointer("/metadata/name")
            .and_then(Value::as_str)
            .ok_or_else(|| KubeletError::PodReconciliationFailed {
                pod: "unknown".to_string(),
                reason: "pod missing metadata.name".to_string(),
            })?;
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

    /// Stops runtime containers whose Pod object no longer exists.
    pub async fn stop_orphaned_pods(
        &self,
        live_uids: &BTreeSet<String>,
    ) -> Result<usize, KubeletError> {
        let mut stopped = 0;
        for managed in self.runtime.list_managed_pods().await? {
            if live_uids.contains(&managed.uid) {
                continue;
            }
            self.runtime.stop_pod(&managed.pod_id).await?;
            eprintln!(
                "stopped containers of deleted pod {}/{} ({})",
                managed.namespace, managed.name, managed.uid
            );
            stopped += 1;
        }
        Ok(stopped)
    }

    /// Drives a pod whose runtime reports real container state.
    ///
    /// Status follows the processes: `Pending` until something runs, `Running`
    /// while a container is alive, then `Succeeded` or `Failed` from exit codes.
    async fn sync_observed_pod(
        &self,
        namespace: &str,
        name: &str,
        pod: &Value,
        mut observed: PodRuntimeStatus,
    ) -> Result<(), KubeletError> {
        if observed.containers.is_empty() {
            if pod.pointer("/status/phase").is_none() {
                let pending = observed_pod_status(pod, &observed, &self.node_ip);
                self.patch_status_if_changed(namespace, name, pod, pending)
                    .await?;
            }
            if let Err(e) = self.runtime.run_pod(pod).await {
                let mut failed = observed_pod_status(pod, &observed, &self.node_ip);
                mark_create_error(&mut failed, &e);
                self.patch_status_if_changed(namespace, name, pod, failed)
                    .await?;
                return Err(e);
            }
            observed = self.runtime.inspect_pod(pod).await?.unwrap_or_default();
        }
        let status = observed_pod_status(pod, &observed, &self.node_ip);
        self.patch_status_if_changed(namespace, name, pod, status)
            .await
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

        // Terminal pods keep their final status; nothing is restarted here.
        if matches!(
            pod.pointer("/status/phase").and_then(Value::as_str),
            Some("Succeeded" | "Failed")
        ) {
            return Ok(());
        }

        // 1. Prepare and stage volumes on host filesystem
        self.prepare_pod_volumes(namespace, pod).await?;

        // Providers that observe real processes drive status from them.
        if let Some(observed) = self.runtime.inspect_pod(pod).await? {
            return self.sync_observed_pod(namespace, name, pod, observed).await;
        }

        // 2. If the pod is already Running, evaluate probes and sync status
        if let Some(phase) = pod
            .get("status")
            .and_then(|s| s.get("phase"))
            .and_then(Value::as_str)
            && phase == "Running"
        {
            return self.sync_running_pod(namespace, pod).await;
        }

        // 3. Determine QoS and execute pod on runtime provider
        let qos = determine_pod_qos(pod);
        let sandbox_id = self.runtime.run_pod(pod).await?;

        let empty_containers = Vec::new();
        let containers = pod
            .get("spec")
            .and_then(|s| s.get("containers"))
            .and_then(Value::as_array)
            .map_or(&empty_containers[..], |c| &c[..]);

        let mut container_statuses =
            self.build_container_statuses(namespace, &sandbox_id, name, qos, containers);

        // Evaluate initial probes
        let mut all_ready = true;
        for (idx, c) in containers.iter().enumerate() {
            let c_name = c.get("name").and_then(Value::as_str).unwrap_or("main");
            let (_, ready) = self
                .evaluate_container_probes(&sandbox_id, c_name, c)
                .await?;
            if !ready {
                all_ready = false;
                if let Some(st) = container_statuses.get_mut(idx) {
                    st["ready"] = json!(false);
                }
            }
        }

        let status = json!({
            "phase": "Running",
            "qosClass": match qos {
                PodQoSClass::Guaranteed => "Guaranteed",
                PodQoSClass::Burstable => "Burstable",
                PodQoSClass::BestEffort => "BestEffort",
            },
            "hostIP": self.node_ip,
            "podIP": self.node_ip,
            "startTime": "2026-09-30T12:00:00Z",
            "conditions": build_pod_conditions(all_ready),
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

fn safe_volume_path(vol_dir: &Path, rel: &str) -> Result<PathBuf, KubeletError> {
    let p = Path::new(rel);
    if rel.is_empty()
        || p.is_absolute()
        || p.components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(KubeletError::InvalidConfiguration {
            field: "volume.items.path".into(),
            reason: format!("invalid volume path '{rel}'"),
        });
    }
    Ok(vol_dir.join(p))
}

fn write_volume_file(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, content)
}

fn stage_secret_files(
    vol_dir: &Path,
    vol_spec: &Value,
    sec_obj: &Value,
) -> Result<(), KubeletError> {
    let data_map = sec_obj.get("data").and_then(Value::as_object);
    let string_data_map = sec_obj.get("stringData").and_then(Value::as_object);

    if let Some(items) = vol_spec.get("items").and_then(Value::as_array) {
        for item in items {
            let key = item.get("key").and_then(Value::as_str).unwrap_or("");
            let path = item.get("path").and_then(Value::as_str).unwrap_or(key);
            let file_path = safe_volume_path(vol_dir, path)?;
            if let Some(val) = data_map.and_then(|m| m.get(key)).and_then(Value::as_str) {
                let bytes = rubix_pki::base64_decode(val.trim())
                    .unwrap_or_else(|_| val.as_bytes().to_vec());
                write_volume_file(&file_path, &bytes)?;
            } else if let Some(val) = string_data_map
                .and_then(|m| m.get(key))
                .and_then(Value::as_str)
            {
                write_volume_file(&file_path, val.as_bytes())?;
            }
        }
    } else {
        if let Some(data) = data_map {
            for (key, val_v) in data {
                if let Some(val_str) = val_v.as_str() {
                    let file_path = safe_volume_path(vol_dir, key)?;
                    let bytes = rubix_pki::base64_decode(val_str.trim())
                        .unwrap_or_else(|_| val_str.as_bytes().to_vec());
                    write_volume_file(&file_path, &bytes)?;
                }
            }
        }
        if let Some(str_data) = string_data_map {
            for (key, val_v) in str_data {
                if let Some(val_str) = val_v.as_str() {
                    let file_path = safe_volume_path(vol_dir, key)?;
                    write_volume_file(&file_path, val_str.as_bytes())?;
                }
            }
        }
    }
    Ok(())
}

fn stage_configmap_files(
    vol_dir: &Path,
    vol_spec: &Value,
    cm_obj: &Value,
) -> Result<(), KubeletError> {
    let data_map = cm_obj.get("data").and_then(Value::as_object);
    let binary_data_map = cm_obj.get("binaryData").and_then(Value::as_object);

    if let Some(items) = vol_spec.get("items").and_then(Value::as_array) {
        for item in items {
            let key = item.get("key").and_then(Value::as_str).unwrap_or("");
            let path = item.get("path").and_then(Value::as_str).unwrap_or(key);
            let file_path = safe_volume_path(vol_dir, path)?;
            if let Some(val) = data_map.and_then(|m| m.get(key)).and_then(Value::as_str) {
                write_volume_file(&file_path, val.as_bytes())?;
            } else if let Some(val) = binary_data_map
                .and_then(|m| m.get(key))
                .and_then(Value::as_str)
            {
                let bytes = rubix_pki::base64_decode(val.trim())
                    .unwrap_or_else(|_| val.as_bytes().to_vec());
                write_volume_file(&file_path, &bytes)?;
            }
        }
    } else {
        if let Some(data) = data_map {
            for (key, val_v) in data {
                if let Some(val_str) = val_v.as_str() {
                    let file_path = safe_volume_path(vol_dir, key)?;
                    write_volume_file(&file_path, val_str.as_bytes())?;
                }
            }
        }
        if let Some(bin_data) = binary_data_map {
            for (key, val_v) in bin_data {
                if let Some(val_str) = val_v.as_str() {
                    let file_path = safe_volume_path(vol_dir, key)?;
                    let bytes = rubix_pki::base64_decode(val_str.trim())
                        .unwrap_or_else(|_| val_str.as_bytes().to_vec());
                    write_volume_file(&file_path, &bytes)?;
                }
            }
        }
    }
    Ok(())
}

fn stage_projected_sa_token(
    vol_dir: &Path,
    namespace: &str,
    pod: &Value,
    sa_tok: &Value,
    apiserver: Option<&Arc<ApiserverService>>,
) -> Result<(), KubeletError> {
    let rel_path = sa_tok
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("token");
    let audience = sa_tok
        .get("audience")
        .and_then(Value::as_str)
        .unwrap_or("api");
    let lifetime = sa_tok
        .get("expirationSeconds")
        .and_then(Value::as_u64)
        .unwrap_or(3600);
    let sa_name = pod
        .get("spec")
        .and_then(|s| s.get("serviceAccountName"))
        .and_then(Value::as_str)
        .unwrap_or("default");
    let pod_name = pod
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    let token_str = if let Some(srv) = apiserver {
        srv.issue_service_account_token(
            namespace,
            sa_name,
            &[audience.to_string()],
            std::time::Duration::from_secs(lifetime),
        )
        .map_err(|e| KubeletError::PodReconciliationFailed {
            pod: pod_name.to_string(),
            reason: format!("failed to issue service account token for {namespace}/{sa_name}: {e}"),
        })?
    } else {
        format!("mock-token-{namespace}-{sa_name}")
    };

    let token_path = safe_volume_path(vol_dir, rel_path)?;
    write_volume_file(&token_path, token_str.as_bytes())?;

    let ns_path = safe_volume_path(vol_dir, "namespace")?;
    if !ns_path.exists() {
        let _ = write_volume_file(&ns_path, namespace.as_bytes());
    }

    let ca_path = safe_volume_path(vol_dir, "ca.crt")?;
    if !ca_path.exists() {
        let ca_bytes = apiserver
            .and_then(|srv| {
                if srv.config().client_ca_file.exists() {
                    std::fs::read(&srv.config().client_ca_file).ok()
                } else {
                    None
                }
            })
            .unwrap_or_else(|| b"mock-ca-cert".to_vec());
        let _ = write_volume_file(&ca_path, &ca_bytes);
    }

    Ok(())
}

fn stage_projected_downward_api(
    vol_dir: &Path,
    namespace: &str,
    pod: &Value,
    items: &[Value],
) -> Result<(), KubeletError> {
    for item in items {
        let path = item.get("path").and_then(Value::as_str).unwrap_or("");
        let field_path = item
            .get("fieldRef")
            .and_then(|f| f.get("fieldPath"))
            .and_then(Value::as_str)
            .unwrap_or("");

        let val = match field_path {
            "metadata.name" => pod
                .get("metadata")
                .and_then(|m| m.get("name"))
                .and_then(Value::as_str)
                .unwrap_or(""),
            "metadata.namespace" => namespace,
            "metadata.uid" => pod
                .get("metadata")
                .and_then(|m| m.get("uid"))
                .and_then(Value::as_str)
                .unwrap_or(""),
            _ => "",
        };

        let target = safe_volume_path(vol_dir, path)?;
        write_volume_file(&target, val.as_bytes())?;
    }
    Ok(())
}

/// Builds a Pod status from the containers the runtime actually reports.
#[must_use]
pub fn observed_pod_status(pod: &Value, observed: &PodRuntimeStatus, node_ip: &str) -> Value {
    let empty = Vec::new();
    let spec_containers = pod
        .pointer("/spec/containers")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let container_statuses: Vec<Value> = spec_containers
        .iter()
        .map(|container| {
            let c_name = container
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("main");
            let image = container
                .get("image")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            observed
                .containers
                .iter()
                .find(|c| c.name == c_name)
                .map_or_else(
                    || {
                        json!({
                            "name": c_name,
                            "image": image,
                            "imageID": "",
                            "ready": false,
                            "started": false,
                            "restartCount": 0,
                            "state": { "waiting": { "reason": "ContainerCreating" } }
                        })
                    },
                    observed_container_status,
                )
        })
        .collect();
    let phase = observed_phase(observed, spec_containers.len());
    let all_running = !container_statuses.is_empty()
        && container_statuses
            .iter()
            .all(|s| s["ready"].as_bool() == Some(true));
    let mut conditions = build_pod_conditions(all_running);
    if matches!(phase, "Succeeded" | "Failed") {
        for condition in &mut conditions {
            if matches!(
                condition["type"].as_str(),
                Some("Ready" | "ContainersReady")
            ) {
                condition["reason"] = json!("PodCompleted");
                condition["message"] = json!("all containers have terminated");
            }
        }
    }
    let start_time = pod
        .pointer("/status/startTime")
        .cloned()
        .unwrap_or_else(|| json!(crate::registration::now_rfc3339_seconds()));
    let qos = match determine_pod_qos(pod) {
        PodQoSClass::Guaranteed => "Guaranteed",
        PodQoSClass::Burstable => "Burstable",
        PodQoSClass::BestEffort => "BestEffort",
    };
    json!({
        "phase": phase,
        "qosClass": qos,
        "hostIP": node_ip,
        "podIP": node_ip,
        "startTime": start_time,
        "conditions": conditions,
        "containerStatuses": container_statuses
    })
}

/// Records a container start failure on every container of a pending status.
fn mark_create_error(status: &mut Value, error: &KubeletError) {
    let Some(statuses) = status
        .get_mut("containerStatuses")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for container in statuses {
        container["state"] = json!({
            "waiting": { "reason": "CreateContainerError", "message": error.to_string() }
        });
    }
}

/// Maps observed containers onto a Pod phase for `restartPolicy: Never` semantics.
#[must_use]
pub fn observed_phase(observed: &PodRuntimeStatus, expected: usize) -> &'static str {
    let containers = &observed.containers;
    if containers
        .iter()
        .any(|c| matches!(c.state, ContainerRuntimeState::Running { .. }))
    {
        return "Running";
    }
    if !containers.is_empty()
        && containers.len() >= expected
        && containers
            .iter()
            .all(|c| matches!(c.state, ContainerRuntimeState::Terminated { .. }))
    {
        let all_zero = containers.iter().all(|c| {
            matches!(
                c.state,
                ContainerRuntimeState::Terminated { exit_code: 0, .. }
            )
        });
        return if all_zero { "Succeeded" } else { "Failed" };
    }
    "Pending"
}

fn observed_container_status(container: &ContainerRuntimeStatus) -> Value {
    let running = matches!(container.state, ContainerRuntimeState::Running { .. });
    let state = match &container.state {
        ContainerRuntimeState::Waiting { reason } => json!({ "waiting": { "reason": reason } }),
        ContainerRuntimeState::Running { started_at } => {
            json!({ "running": { "startedAt": started_at } })
        },
        ContainerRuntimeState::Terminated {
            exit_code,
            started_at,
            finished_at,
        } => json!({
            "terminated": {
                "exitCode": exit_code,
                "reason": if *exit_code == 0 { "Completed" } else { "Error" },
                "startedAt": started_at,
                "finishedAt": finished_at,
                "containerID": container.container_id
            }
        }),
    };
    json!({
        "name": container.name,
        "image": container.image,
        "imageID": container.image_id,
        "containerID": container.container_id,
        "ready": running,
        "started": running,
        "restartCount": 0,
        "state": state
    })
}

fn build_pod_conditions(all_ready: bool) -> Vec<Value> {
    let ready_status_str = if all_ready { "True" } else { "False" };
    let ready_reason = if all_ready {
        "PodReady"
    } else {
        "ContainersNotReady"
    };
    let ready_msg = if all_ready {
        "pod is ready"
    } else {
        "containers not ready"
    };

    vec![
        json!({
            "type": "PodScheduled",
            "status": "True",
            "reason": "PodScheduled",
            "message": "pod assigned to node"
        }),
        json!({
            "type": "Initialized",
            "status": "True",
            "reason": "PodInitialized",
            "message": "all init containers completed"
        }),
        json!({
            "type": "ContainersReady",
            "status": ready_status_str,
            "reason": ready_reason,
            "message": ready_msg
        }),
        json!({
            "type": "Ready",
            "status": ready_status_str,
            "reason": ready_reason,
            "message": ready_msg
        }),
    ]
}

fn check_pod_restart_need(
    pod: &Value,
    node_name: &str,
    provider_name: &str,
    namespace: &str,
) -> Vec<WorkloadRestartReport> {
    let mut reports = Vec::new();
    let assigned_node = pod
        .get("spec")
        .and_then(|s| s.get("nodeName"))
        .and_then(Value::as_str);
    if assigned_node != Some(node_name) {
        return reports;
    }

    let phase = pod
        .get("status")
        .and_then(|s| s.get("phase"))
        .and_then(Value::as_str);
    if phase != Some("Running") {
        return reports;
    }

    let qos = determine_pod_qos(pod);
    let pod_name = pod
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    let Some(containers) = pod
        .get("spec")
        .and_then(|s| s.get("containers"))
        .and_then(Value::as_array)
    else {
        return reports;
    };

    for c in containers {
        let c_name = c.get("name").and_then(Value::as_str).unwrap_or("unknown");
        if is_container_cpu_pinning_eligible(qos, c).is_some() {
            let c_id = pod
                .get("status")
                .and_then(|s| s.get("containerStatuses"))
                .and_then(Value::as_array)
                .and_then(|cs| {
                    cs.iter()
                        .find(|s| s.get("name").and_then(Value::as_str) == Some(c_name))
                })
                .and_then(|s| s.get("containerID"))
                .and_then(Value::as_str)
                .map_or_else(|| format!("{provider_name}://unknown"), ToString::to_string);
            let reason = format!(
                "CPU manager checkpoint invalidated; surviving external container '{c_name}' in pod '{namespace}/{pod_name}' runs in shared pool until restarted"
            );
            eprintln!("workload restart required: {reason}");
            reports.push(WorkloadRestartReport {
                namespace: namespace.to_string(),
                pod_name: pod_name.to_string(),
                container_name: c_name.to_string(),
                container_id: c_id,
                reason,
            });
        }
    }

    reports
}
