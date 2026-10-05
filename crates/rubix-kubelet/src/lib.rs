//! Kubelet configuration, node registration, and single-node workload execution.
//!
//! Provides configuration generation matching official upstream `kubelet.config.k8s.io/v1beta1`,
//! node identity and RBAC integration, CRI runtime attachment for both managed and external providers,
//! Node registration and lease heartbeats, and manually assigned Pod execution.

pub mod config;
pub mod container;
pub mod error;
pub mod health;
pub mod oci;
pub mod podman;
pub mod registration;
pub mod service;
pub mod supervisor;
pub mod workload;

pub use config::{
    CPU_MANAGER_CHECKPOINT_FILE, CpuManagerSettings, DEFAULT_CLUSTER_DNS, DEFAULT_CLUSTER_DOMAIN,
    DEFAULT_HEALTHZ_BIND_ADDRESS, DEFAULT_HEALTHZ_PORT, DEFAULT_KUBELET_PORT,
    DEFAULT_KUBELET_READ_ONLY_PORT, DEFAULT_KUBELET_ROOT_DIR, KubeletConfigOptions,
    detect_host_cpu_count, format_cpuset, parse_cpuset, render_canonical_yaml,
};
pub use container::{
    CgroupSetupStatus, ContainerEnvironment, Ipv6DisableStatus, KubeletCgroupVersion,
    MountPropagationStatus,
};
pub use error::KubeletError;
pub use health::KubeletHealthReport;
pub use oci::{
    ContainerSpec, ContainerState, ContainerSummary, OciEngine, OciRuntimeAdapter, PodIdentity,
    PullPolicy,
};
pub use podman::PodmanEngine;
pub use registration::NodeRegistration;
pub use service::{KubeletLogSource, KubeletService};
pub use supervisor::{COMPONENT_KUBELET, DEFAULT_STARTUP_TIMEOUT, KubeletAdapter};
pub use workload::{
    ContainerRuntimeState, ContainerRuntimeStatus, CpuManager, CpuManagerState, ExecResult,
    ManagedPodRef, MockRuntimeProvider, PodQoSClass, PodReconciler, PodRuntimeStatus,
    ReconcileReport, RuntimeProvider, WorkloadRestartReport, determine_pod_qos,
    is_container_cpu_pinning_eligible, observed_phase, observed_pod_status,
    parse_cpu_quantity_milli, parse_memory_quantity_bytes,
};
