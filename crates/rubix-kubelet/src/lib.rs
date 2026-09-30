//! Kubelet configuration, node registration, and single-node workload execution.
//!
//! Provides configuration generation matching official upstream `kubelet.config.k8s.io/v1beta1`,
//! node identity and RBAC integration, CRI runtime attachment for both managed and external providers,
//! Node registration and lease heartbeats, and manually assigned Pod execution.

pub mod config;
pub mod container;
pub mod error;
pub mod health;
pub mod registration;
pub mod service;
pub mod supervisor;
pub mod workload;

pub use config::{
    DEFAULT_CLUSTER_DNS, DEFAULT_CLUSTER_DOMAIN, DEFAULT_HEALTHZ_BIND_ADDRESS,
    DEFAULT_HEALTHZ_PORT, DEFAULT_KUBELET_PORT, DEFAULT_KUBELET_READ_ONLY_PORT,
    DEFAULT_KUBELET_ROOT_DIR, KubeletConfigOptions, render_canonical_yaml,
};
pub use container::{
    CgroupSetupStatus, ContainerEnvironment, Ipv6DisableStatus, KubeletCgroupVersion,
    MountPropagationStatus,
};
pub use error::KubeletError;
pub use health::KubeletHealthReport;
pub use registration::NodeRegistration;
pub use service::KubeletService;
pub use supervisor::{COMPONENT_KUBELET, DEFAULT_STARTUP_TIMEOUT, KubeletAdapter};
pub use workload::{MockRuntimeProvider, PodReconciler, RuntimeProvider};
