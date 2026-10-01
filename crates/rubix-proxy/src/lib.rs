//! Kube-proxy configuration, backend selection, and supervised lifecycle.
//!
//! Provides configuration generation matching official upstream `kubeproxy.config.k8s.io/v1alpha1`,
//! automatic detection and selection between `iptables` and native `nftables` backends,
//! conntrack zeroing in container mode to accommodate read-only `/proc/sys` environments,
//! preservation of upstream host defaults, and integration with `rubix-supervisor`.

pub mod backend;
pub mod config;
pub mod error;
pub mod health;
pub mod service;
pub mod supervisor;

pub use backend::{
    ProxyMode, check_sysctl_conntrack_writable, detect_proxy_backend, flush_nftables_nat,
};
pub use config::{
    ConntrackConfiguration, DEFAULT_CLUSTER_CIDR, DEFAULT_CONNTRACK_MAX_PER_CORE,
    DEFAULT_CONNTRACK_MIN, DEFAULT_CONNTRACK_TCP_CLOSE_WAIT_TIMEOUT,
    DEFAULT_CONNTRACK_TCP_ESTABLISHED_TIMEOUT, DEFAULT_CONNTRACK_UDP_STREAM_TIMEOUT,
    DEFAULT_CONNTRACK_UDP_TIMEOUT, DEFAULT_HEALTHZ_BIND_ADDRESS, DEFAULT_HEALTHZ_PORT,
    DEFAULT_METRICS_BIND_ADDRESS, DEFAULT_OOM_SCORE_ADJ, IptablesConfiguration,
    KubeProxyConfiguration, KubeProxyOptions, NftablesConfiguration,
};
pub use error::ProxyError;
pub use health::ProxyHealthReport;
pub use service::ProxyService;
pub use supervisor::{COMPONENT_PROXY, DEFAULT_STARTUP_TIMEOUT, ProxyAdapter};
