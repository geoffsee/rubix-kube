//! Supervised node runtime assembly and C02 lifecycle contract.
//!
//! Provides dependency-aware component coordination distinguishing fatal-core
//! and optional-deployment failure policies, structured JSONL lifecycle logging
//! with debug controls, credential-safe diagnostics, and production API client access.

use std::fmt;
use std::io::{self, Write};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use rubix_apiserver::ApiserverConfig;
use rubix_apiserver::client::KubernetesApiClient;
use rubix_apiserver::service::ApiserverService;
use rubix_apiserver::storage::KubernetesStorage;
use rubix_apiserver::supervisor::ApiserverAdapter;
use rubix_config::ValidatedConfig;
use rubix_controller::ControllerManagerConfig;
use rubix_controller::service::ControllerManagerService;
use rubix_controller::supervisor::ControllerManagerAdapter;
use rubix_datastore::config::DatastoreConfig;
use rubix_datastore::engine::DatastoreEngine;
use rubix_datastore::supervisor::DatastoreAdapter;
use rubix_dns::config::CoreDnsConfig;
use rubix_dns::service::CoreDnsService;
use rubix_dns::supervisor::CoreDnsAdapter;
use rubix_pki::cluster::{ClusterPki, ClusterPkiConfig};
use rubix_portainer::config::PortainerAgentConfig;
use rubix_portainer::service::PortainerService;
use rubix_portainer::supervisor::PortainerAdapter;
use rubix_storage::config::LocalPathConfig;
use rubix_storage::service::LocalPathService;
use rubix_storage::supervisor::LocalPathAdapter;
use rubix_supervisor::{
    Adapter, ComponentKind, ComponentSpec, FailurePolicy, GraphError, LifecycleObserver,
    Registration, StopCause, StopReceiver, Supervisor, SupervisorReport, stop_channel,
};

use crate::lifecycle_logs::{
    DeliveryReport, LifecycleRenderer, LogLevel, consume_lifecycle, log_channel,
};
use crate::lifecycle_policy::LifecyclePolicy;
use crate::lifecycle_sink::{FlushPolicy, SinkReport, configured_log_level, deliver_logs};

pub const COMPONENT_DATASTORE: &str = "datastore";
pub const COMPONENT_APISERVER: &str = "apiserver";
pub const COMPONENT_CONTROLLER_MANAGER: &str = "controller-manager";
pub const COMPONENT_COREDNS: &str = "coredns";
pub const COMPONENT_LOCAL_PATH: &str = "local-path-provisioner";
pub const COMPONENT_PORTAINER: &str = "portainer-agent";
pub const COMPONENT_PROXY: &str = "kube-proxy";
pub const COMPONENT_KUBELET: &str = "kubelet";

/// Errors encountered while constructing or initializing the node runtime.
#[derive(Debug)]
pub enum RuntimeError {
    Pki(rubix_pki::PkiError),
    Datastore(rubix_datastore::DatastoreError),
    Apiserver(rubix_apiserver::ApiserverError),
    Supervisor(GraphError),
    Io(io::Error),
    ChannelCapacity,
}

impl RuntimeError {
    #[must_use]
    pub fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::Pki(_) => "pki_failure",
            Self::Datastore(_) => "datastore_failure",
            Self::Apiserver(_) => "apiserver_failure",
            Self::Supervisor(_) => "supervisor_failure",
            Self::Io(_) => "io_failure",
            Self::ChannelCapacity => "channel_capacity_error",
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pki(e) => write!(f, "PKI initialization failure: {e}"),
            Self::Datastore(e) => write!(f, "datastore initialization failure: {e}"),
            Self::Apiserver(e) => write!(f, "apiserver initialization failure: {e}"),
            Self::Supervisor(e) => write!(f, "supervisor configuration error: {e}"),
            Self::Io(e) => write!(f, "I/O failure during runtime assembly: {e}"),
            Self::ChannelCapacity => write!(f, "invalid log channel capacity"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<rubix_pki::PkiError> for RuntimeError {
    fn from(e: rubix_pki::PkiError) -> Self {
        Self::Pki(e)
    }
}

impl From<rubix_datastore::DatastoreError> for RuntimeError {
    fn from(e: rubix_datastore::DatastoreError) -> Self {
        Self::Datastore(e)
    }
}

impl From<rubix_apiserver::ApiserverError> for RuntimeError {
    fn from(e: rubix_apiserver::ApiserverError) -> Self {
        Self::Apiserver(e)
    }
}

impl From<GraphError> for RuntimeError {
    fn from(e: GraphError) -> Self {
        Self::Supervisor(e)
    }
}

impl From<io::Error> for RuntimeError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Builder for extensible node runtime assembly satisfying the C02 runtime contract.
#[derive(Debug)]
pub struct RuntimeBuilder {
    config: ValidatedConfig,
    policy: LifecyclePolicy,
    log_level: LogLevel,
    apiserver: Option<Arc<ApiserverService>>,
    client: Option<KubernetesApiClient>,
    registrations: Vec<Registration>,
}

impl RuntimeBuilder {
    /// Creates a new builder initialized with default policy and log level from configuration.
    #[must_use]
    pub fn new(config: ValidatedConfig) -> Self {
        let policy = LifecyclePolicy::from_config(&config);
        let log_level = configured_log_level(&config);
        Self {
            config,
            policy,
            log_level,
            apiserver: None,
            client: None,
            registrations: Vec::new(),
        }
    }

    #[must_use]
    pub fn config(&self) -> &ValidatedConfig {
        &self.config
    }

    #[must_use]
    pub fn policy(&self) -> &LifecyclePolicy {
        &self.policy
    }

    #[must_use]
    pub fn log_level(&self) -> LogLevel {
        self.log_level
    }

    /// Exposes the production client if an apiserver has been associated with the builder.
    #[must_use]
    pub fn client(&self) -> Option<&KubernetesApiClient> {
        self.client.as_ref()
    }

    #[must_use]
    pub fn apiserver(&self) -> Option<&Arc<ApiserverService>> {
        self.apiserver.as_ref()
    }

    #[must_use]
    pub fn with_apiserver(mut self, apiserver: Arc<ApiserverService>) -> Self {
        self.client = Some(apiserver.admin_client());
        self.apiserver = Some(apiserver);
        self
    }

    #[must_use]
    pub fn with_client(mut self, client: KubernetesApiClient) -> Self {
        self.client = Some(client);
        self
    }

    /// Registers a component directly with its existing specification.
    #[must_use]
    pub fn register_component(mut self, registration: Registration) -> Self {
        self.registrations.push(registration);
        self
    }

    /// Registers a fatal core component. Failure causes global supervisor shutdown.
    #[must_use]
    pub fn register_core<A: Adapter + 'static>(
        mut self,
        id: impl Into<String>,
        prerequisites: Vec<String>,
        adapter: A,
    ) -> Self {
        let spec = self.policy.apply(ComponentSpec {
            id: id.into(),
            prerequisites,
            kind: ComponentKind::LongRunning,
            failure_policy: FailurePolicy::Fatal,
            startup_timeout: self.policy.startup_timeout(),
        });
        self.registrations.push(Registration::new(spec, adapter));
        self
    }

    /// Registers an optional component. Failure degrades the node while core API remains operational.
    #[must_use]
    pub fn register_optional<A: Adapter + 'static>(
        mut self,
        id: impl Into<String>,
        prerequisites: Vec<String>,
        adapter: A,
    ) -> Self {
        let spec = self.policy.apply(ComponentSpec {
            id: id.into(),
            prerequisites,
            kind: ComponentKind::LongRunning,
            failure_policy: FailurePolicy::Degrade,
            startup_timeout: self.policy.startup_timeout(),
        });
        self.registrations.push(Registration::new(spec, adapter));
        self
    }

    /// Registers the optional Portainer Edge Agent component under supervision.
    #[must_use]
    pub fn register_portainer(self, service: PortainerService, prerequisites: Vec<String>) -> Self {
        let timeout = self.policy.startup_timeout();
        let reg =
            PortainerAdapter::registration(COMPONENT_PORTAINER, service, prerequisites, timeout);
        self.register_component(reg)
    }

    /// Builds the supervised node runtime.
    pub fn build(self) -> Result<NodeRuntime, RuntimeError> {
        let (supervisor, observer) = Supervisor::new(self.registrations)?.with_observer();
        Ok(NodeRuntime {
            config: self.config,
            policy: self.policy,
            log_level: self.log_level,
            apiserver: self.apiserver,
            client: self.client,
            supervisor,
            observer,
        })
    }
}

/// Operational node runtime coordinating components under supervision.
#[derive(Debug)]
pub struct NodeRuntime {
    config: ValidatedConfig,
    policy: LifecyclePolicy,
    log_level: LogLevel,
    apiserver: Option<Arc<ApiserverService>>,
    client: Option<KubernetesApiClient>,
    supervisor: Supervisor,
    observer: LifecycleObserver,
}

impl NodeRuntime {
    /// Constructs a builder for customized runtime assembly.
    #[must_use]
    pub fn builder(config: ValidatedConfig) -> RuntimeBuilder {
        RuntimeBuilder::new(config)
    }

    /// Assembles the default production node runtime from validated configuration.
    pub fn from_config(config: ValidatedConfig) -> Result<Self, RuntimeError> {
        let state_dir = PathBuf::from(&config.config().path);
        std::fs::create_dir_all(&state_dir)?;

        let node_ip: IpAddr = if config.config().network.node_ip.is_empty() {
            "127.0.0.1".parse().unwrap()
        } else {
            config.config().network.node_ip.parse().map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidInput, format!("invalid node IP: {e}"))
            })?
        };

        let node_name = if config.config().kubernetes.node_name.is_empty() {
            "rubix-node".to_string()
        } else {
            config.config().kubernetes.node_name.clone()
        };

        let pki_dir = state_dir.join("pki");
        std::fs::create_dir_all(&pki_dir)?;
        let pki_config = ClusterPkiConfig::new(pki_dir.clone(), node_name, node_ip);
        let pki = ClusterPki::new(pki_config);
        pki.reconcile()?;

        let datastore_dir = state_dir.join("datastore");
        std::fs::create_dir_all(&datastore_dir)?;
        let mut datastore_cfg = DatastoreConfig::new(datastore_dir);
        if config.config().storage.db_wal_repair {
            datastore_cfg = datastore_cfg.with_wal_repair(true);
        }

        let (engine, _) = DatastoreEngine::open(datastore_cfg.clone())?;
        let storage = KubernetesStorage::new(engine.client(), "/registry");

        let apiserver_cfg = ApiserverConfig::default_for_pki(&pki_dir, node_ip);
        let apiserver_service = Arc::new(ApiserverService::new(apiserver_cfg, storage));
        let client = apiserver_service.admin_client();

        let controller_cfg = ControllerManagerConfig::default_for_pki(&pki_dir, node_ip);
        let controller_service =
            ControllerManagerService::new(controller_cfg, apiserver_service.clone());

        let dns_config = CoreDnsConfig::new();
        let dns_service = CoreDnsService::new(dns_config, Arc::new(client.clone()));

        let policy = LifecyclePolicy::from_config(&config);
        let timeout = policy.startup_timeout();

        let mut builder = RuntimeBuilder::new(config)
            .with_apiserver(apiserver_service.clone())
            .with_client(client.clone());

        let datastore_reg =
            DatastoreAdapter::registration_for_engine(COMPONENT_DATASTORE, engine, timeout);
        let apiserver_reg = ApiserverAdapter::registration(
            COMPONENT_APISERVER,
            (*apiserver_service).clone(),
            vec![COMPONENT_DATASTORE.to_string()],
            timeout,
        );
        let controller_reg = ControllerManagerAdapter::registration(
            COMPONENT_CONTROLLER_MANAGER,
            controller_service,
            vec![COMPONENT_APISERVER.to_string()],
            timeout,
        );
        builder = builder
            .register_component(datastore_reg)
            .register_component(apiserver_reg)
            .register_component(controller_reg)
            .register_optional(
                COMPONENT_COREDNS,
                vec![COMPONENT_APISERVER.to_string()],
                CoreDnsAdapter::new(dns_service),
            );

        if builder.config().config().storage.local_path.enabled {
            let storage_config = LocalPathConfig::new().with_enabled(true);
            let storage_service = LocalPathService::new(storage_config, Arc::new(client.clone()));
            let storage_reg = LocalPathAdapter::registration(
                COMPONENT_LOCAL_PATH,
                storage_service,
                vec![COMPONENT_APISERVER.to_string()],
                timeout,
            );
            builder = builder.register_component(storage_reg);
        }

        let arch = match std::env::consts::ARCH {
            "aarch64" => rubix_platform::Architecture::Arm64,
            "arm" => rubix_platform::Architecture::ArmV7,
            "riscv64" => rubix_platform::Architecture::Riscv64,
            _ => rubix_platform::Architecture::Amd64,
        };
        let portainer_cfg =
            PortainerAgentConfig::from_rubix_config(&builder.config().config().portainer, arch);
        if portainer_cfg.is_enabled() {
            let portainer_service = PortainerService::new(portainer_cfg, Arc::new(client));
            let portainer_reg = PortainerAdapter::registration(
                COMPONENT_PORTAINER,
                portainer_service,
                vec![COMPONENT_APISERVER.to_string()],
                timeout,
            );
            builder = builder.register_component(portainer_reg);
        }

        builder.build()
    }

    #[must_use]
    pub fn config(&self) -> &ValidatedConfig {
        &self.config
    }

    #[must_use]
    pub fn lifecycle_policy(&self) -> LifecyclePolicy {
        self.policy
    }

    #[must_use]
    pub fn log_level(&self) -> LogLevel {
        self.log_level
    }

    /// Exposes the authenticated production API client satisfying the C02 runtime contract.
    #[must_use]
    pub fn client(&self) -> Option<&KubernetesApiClient> {
        self.client.as_ref()
    }

    #[must_use]
    pub fn apiserver(&self) -> Option<&Arc<ApiserverService>> {
        self.apiserver.as_ref()
    }

    #[must_use]
    pub fn observer(&self) -> LifecycleObserver {
        self.observer.clone()
    }

    /// Runs the supervised runtime until the stop receiver indicates shutdown.
    pub async fn run(self, control: StopReceiver) -> SupervisorReport {
        self.supervisor.run(control).await
    }

    /// Runs the runtime while streaming structured JSONL logs to a caller-owned sink.
    pub async fn run_with_sink<W>(
        self,
        control: StopReceiver,
        sink: W,
        flush_policy: FlushPolicy,
    ) -> (SupervisorReport, DeliveryReport, SinkReport)
    where
        W: Write + Send + 'static,
    {
        let Ok((sender, receiver)) = log_channel(64) else {
            unreachable!("64 is within valid queue capacity");
        };
        let logging_task = tokio::spawn(consume_lifecycle(
            self.observer,
            LifecycleRenderer::new(self.log_level),
            sender,
        ));
        let delivery_task = tokio::spawn(deliver_logs(receiver, sink, flush_policy));

        let report = self.supervisor.run(control).await;
        let delivery_report: DeliveryReport = logging_task.await.unwrap_or_default();
        let sink_report = match delivery_task.await {
            Ok(r) => r,
            Err(_) => SinkReport {
                written: 0,
                flushed_frames: 0,
                outcome: crate::lifecycle_sink::SinkOutcome::WorkerLost,
            },
        };

        (report, delivery_report, sink_report)
    }

    /// Runs the production node to completion, wiring Unix signals and delivering structured logs.
    pub async fn run_to_completion(self) -> io::Result<u8> {
        let (stop_handle, stop_receiver) = stop_channel();

        #[cfg(unix)]
        {
            let handle = stop_handle.clone();
            tokio::spawn(async move {
                use tokio::signal::unix::{SignalKind, signal};
                let mut sigterm = signal(SignalKind::terminate()).ok();
                let mut sigint = signal(SignalKind::interrupt()).ok();
                tokio::select! {
                    () = async {
                        if let Some(s) = sigterm.as_mut() {
                            s.recv().await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    } => {},
                    () = async {
                        if let Some(s) = sigint.as_mut() {
                            s.recv().await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    } => {},
                }
                handle.stop();
            });
        }

        let (report, _delivery, _sink) = self
            .run_with_sink(stop_receiver, io::stderr(), FlushPolicy::EachFrame)
            .await;

        Ok(runtime_exit_code(&report.cause))
    }
}

fn runtime_exit_code(cause: &StopCause) -> u8 {
    u8::from(!matches!(cause, StopCause::Requested))
}

#[cfg(test)]
mod exit_code_tests {
    use super::*;

    #[test]
    fn only_requested_shutdown_is_successful() {
        assert_eq!(runtime_exit_code(&StopCause::Requested), 0);
        assert_eq!(runtime_exit_code(&StopCause::ControlClosed), 1);
        assert_eq!(runtime_exit_code(&StopCause::Finished), 1);
        assert_eq!(
            runtime_exit_code(&StopCause::Fatal(rubix_supervisor::ComponentFailure {
                component: "test-core".into(),
                kind: rubix_supervisor::FailureKind::UnexpectedExit,
            })),
            1
        );
    }
}
