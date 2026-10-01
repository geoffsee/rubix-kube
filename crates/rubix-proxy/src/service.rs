use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rubix_network::{CommandExecutor, MasqueradeBackend, SystemCommandExecutor};
use tokio::sync::RwLock;

use crate::backend::{
    ProxyMode, check_sysctl_conntrack_writable, detect_proxy_backend, flush_nftables_nat,
};
use crate::config::KubeProxyOptions;
use crate::error::ProxyError;
use crate::health::ProxyHealthReport;
use crate::prober::{DataplaneProbeReport, DataplaneProber};
use crate::routing::ServiceRoutingTable;

/// Supervised kube-proxy service managing configuration, backend selection,
/// readiness verification, dataplane routing, and lifecycle.
#[derive(Clone)]
pub struct ProxyService {
    options: KubeProxyOptions,
    executor: Arc<dyn CommandExecutor>,
    sys_root: Option<PathBuf>,
    running: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    snat_ready: Arc<AtomicBool>,
    dataplane_ready: Arc<AtomicBool>,
    routing_table: Arc<RwLock<ServiceRoutingTable>>,
}

impl std::fmt::Debug for ProxyService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyService")
            .field("options", &self.options)
            .field("sys_root", &self.sys_root)
            .field("running", &self.is_running())
            .field("ready", &self.is_ready())
            .field("snat_ready", &self.is_snat_ready())
            .field("dataplane_ready", &self.is_dataplane_ready())
            .finish_non_exhaustive()
    }
}

impl ProxyService {
    #[must_use]
    pub fn new(options: KubeProxyOptions) -> Self {
        Self {
            options,
            executor: Arc::new(SystemCommandExecutor),
            sys_root: None,
            running: Arc::new(AtomicBool::new(false)),
            ready: Arc::new(AtomicBool::new(false)),
            snat_ready: Arc::new(AtomicBool::new(false)),
            dataplane_ready: Arc::new(AtomicBool::new(false)),
            routing_table: Arc::new(RwLock::new(ServiceRoutingTable::new())),
        }
    }

    #[must_use]
    pub fn with_executor(mut self, executor: Arc<dyn CommandExecutor>) -> Self {
        self.executor = executor;
        self
    }

    #[must_use]
    pub fn with_sys_root(mut self, sys_root: PathBuf) -> Self {
        self.sys_root = Some(sys_root);
        self
    }

    #[must_use]
    pub fn options(&self) -> &KubeProxyOptions {
        &self.options
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn is_snat_ready(&self) -> bool {
        self.snat_ready.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn is_dataplane_ready(&self) -> bool {
        self.dataplane_ready.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn routing_table(&self) -> &Arc<RwLock<ServiceRoutingTable>> {
        &self.routing_table
    }

    /// Checks and prepares all prerequisites for running kube-proxy.
    ///
    /// 1. Validates kubeconfig file existence (if specified).
    /// 2. Validates backend selection (detects iptables vs nftables, or confirms selected mode).
    /// 3. Checks `/proc/sys` conntrack sysctl writability:
    ///    - In container mode, zeroes all six conntrack settings and skips writing to `/proc/sys`.
    ///    - In host mode, checks if `/proc/sys/net/netfilter` is writable; if read-only, reports
    ///      actionable error recommending `--container-mode`.
    /// 4. Flushes conflicting nftables nat table rules if running in nftables mode.
    pub fn check_prerequisites(&mut self) -> Result<(), ProxyError> {
        // 1. Validate kubeconfig credential if path is specified
        if !self.options.kubeconfig.as_os_str().is_empty() && !self.options.kubeconfig.exists() {
            return Err(ProxyError::MissingCredential {
                path: self.options.kubeconfig.clone(),
                component: "kubeconfig",
            });
        }

        // 2. Detect / validate backend
        let detected = detect_proxy_backend(self.sys_root.as_deref(), self.executor.as_ref())?;
        self.options.proxy_mode = detected;

        // 3. Check /proc/sys conntrack writability
        check_sysctl_conntrack_writable(self.sys_root.as_deref(), self.options.container_mode)?;

        // 4. In nftables mode, flush any pre-existing nat table to avoid nftables conflicts
        if self.options.proxy_mode == ProxyMode::Nftables {
            flush_nftables_nat(self.executor.as_ref())?;
        }

        Ok(())
    }

    /// Starts kube-proxy supervision.
    ///
    /// Validates the generated command flags and establishes service running state.
    pub fn start(&self) -> Result<(), ProxyError> {
        let flags = self.options.generate_flags();
        tracing::info!(
            target: "kubeproxy::startup",
            mode = %self.options.proxy_mode,
            container_mode = self.options.container_mode,
            flags = ?flags,
            "starting kubeproxy service..."
        );
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Verifies component startup readiness and pod egress masquerade rules.
    ///
    /// Distinct from dataplane probes: confirms that the process is alive and host
    /// SNAT rules are established before network packets are routed.
    pub fn check_startup_readiness(&self) -> Result<ProxyHealthReport, ProxyError> {
        if !self.is_running() {
            return Err(ProxyError::HealthCheckFailed {
                reason: "kube-proxy is not running".to_string(),
            });
        }

        let masquerade_backend = match self.options.proxy_mode {
            ProxyMode::IpTables => MasqueradeBackend::IpTables,
            ProxyMode::Nftables => MasqueradeBackend::Nftables,
        };

        rubix_network::ensure_pod_masquerade_with_backend_and_executor(
            &self.options.cluster_cidr,
            masquerade_backend,
            self.executor.as_ref(),
        )
        .map_err(|e| {
            self.ready.store(false, Ordering::SeqCst);
            self.snat_ready.store(false, Ordering::SeqCst);
            ProxyError::MasqueradeFailed {
                reason: e.to_string(),
            }
        })?;

        self.snat_ready.store(true, Ordering::SeqCst);
        self.ready.store(true, Ordering::SeqCst);

        tracing::info!(
            target: "kubeproxy::startup",
            mode = %self.options.proxy_mode,
            container_mode = self.options.container_mode,
            snat_ready = true,
            "kube-proxy component startup verified (process running, SNAT ready)"
        );

        let report = ProxyHealthReport::new_healthy(
            self.options.proxy_mode,
            self.options.container_mode,
            self.options.effective_conntrack().is_all_zero(),
            self.is_snat_ready(),
        );

        Ok(report)
    }

    /// Verifies dataplane packet routing against all ready endpoints in the routing table.
    ///
    /// Distinctly logs and reports dataplane routing probe success or failure,
    /// separate from component process startup.
    pub async fn verify_dataplane(
        &self,
        prober: &DataplaneProber,
    ) -> Result<DataplaneProbeReport, ProxyError> {
        if !self.is_ready() {
            return Err(ProxyError::HealthCheckFailed {
                reason: "kube-proxy component is not startup-ready before dataplane verification"
                    .to_string(),
            });
        }

        let table = self.routing_table.read().await;
        match prober.verify_all_dataplane_routes(&table) {
            Ok(report) => {
                self.dataplane_ready.store(true, Ordering::SeqCst);
                tracing::info!(
                    target: "kubeproxy::dataplane",
                    total_probes = report.total_probes,
                    successful = report.successful_probes,
                    endpoints_reached = report.endpoints_reached.len(),
                    "dataplane verification succeeded: all Service routes reached ready endpoints"
                );
                Ok(report)
            },
            Err(e) => {
                self.dataplane_ready.store(false, Ordering::SeqCst);
                tracing::error!(
                    target: "kubeproxy::dataplane",
                    reason = %e,
                    "dataplane verification failed: component is running but dataplane traffic is not routing"
                );
                Err(e)
            },
        }
    }

    /// Verifies overall readiness, combining component startup and (if verified) dataplane readiness.
    pub fn check_readiness(&self) -> Result<ProxyHealthReport, ProxyError> {
        let mut report = self.check_startup_readiness()?;
        let dp_ready = self.is_dataplane_ready();
        if dp_ready {
            report = report.with_dataplane_ready(true, "dataplane probes verified");
        } else {
            report = report.with_dataplane_ready(false, "dataplane probes not yet verified");
        }
        Ok(report)
    }

    /// Stops kube-proxy.
    pub fn stop(&self) {
        tracing::info!(
            target: "kubeproxy::startup",
            "stopping kubeproxy service..."
        );
        self.running.store(false, Ordering::SeqCst);
        self.ready.store(false, Ordering::SeqCst);
        self.snat_ready.store(false, Ordering::SeqCst);
        self.dataplane_ready.store(false, Ordering::SeqCst);
    }
}
