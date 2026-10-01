use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use rubix_apiserver::KubernetesApiClient;

use crate::config::{KubeletConfigOptions, detect_host_cpu_count, format_cpuset, parse_cpuset};
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
}

/// Simulated in-memory runtime provider for testing managed and external runtime engines.
#[derive(Debug)]
pub struct MockRuntimeProvider {
    name: String,
    pod_counter: AtomicUsize,
    is_external: bool,
}

impl MockRuntimeProvider {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pod_counter: AtomicUsize::new(1),
            is_external: false,
        }
    }

    #[must_use]
    pub fn new_external(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pod_counter: AtomicUsize::new(1),
            is_external: true,
        }
    }

    pub fn set_external(&mut self, external: bool) {
        self.is_external = external;
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
        Ok(pod_id)
    }

    async fn stop_pod(&self, _pod_id: &str) -> Result<(), KubeletError> {
        Ok(())
    }

    async fn get_pod_status(&self, _pod_id: &str) -> Result<String, KubeletError> {
        Ok("Running".to_string())
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
        }
    }

    #[must_use]
    pub fn with_cpu_manager(mut self, cpu_manager: Arc<CpuManager>) -> Self {
        self.cpu_manager = cpu_manager;
        self
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

        // Determine QoS and CPU pinning eligibility
        let qos = determine_pod_qos(pod);

        // Execute pod on runtime provider
        let sandbox_id = self.runtime.run_pod(pod).await?;

        let empty_containers = Vec::new();
        let containers = pod
            .get("spec")
            .and_then(|s| s.get("containers"))
            .and_then(Value::as_array)
            .map_or(&empty_containers[..], |c| &c[..]);

        let container_statuses =
            self.build_container_statuses(namespace, &sandbox_id, name, qos, containers);

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
