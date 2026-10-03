use std::collections::BTreeMap;
use std::io::{self, Write};
use std::net::IpAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rubix_apiserver::ApiserverConfig;
use rubix_apiserver::service::ApiserverService;
use rubix_apiserver::storage::KubernetesStorage;
use rubix_apiserver::supervisor::ApiserverAdapter;
use rubix_config::{
    EnvironmentMode, ExplicitFlags, HostContext, ValidatedConfig, decode, resolve_layers,
    write_document,
};
use rubix_datastore::config::DatastoreConfig;
use rubix_datastore::engine::DatastoreEngine;
use rubix_datastore::supervisor::DatastoreAdapter;
use rubix_kube::config_api::ConfigApiServer;
use rubix_kube::lifecycle_logs::LogLevel;
use rubix_kube::lifecycle_sink::FlushPolicy;
use rubix_kube::runtime::{
    COMPONENT_APISERVER, COMPONENT_CONFIG_API, COMPONENT_CONTROLLER_MANAGER, COMPONENT_COREDNS,
    COMPONENT_DATASTORE, COMPONENT_KUBELET, COMPONENT_LOCAL_PATH, COMPONENT_PORTAINER,
    COMPONENT_PROXY, NodeRuntime, RuntimeBuilder,
};
use rubix_pki::cluster::{ClusterPki, ClusterPkiConfig};
use rubix_platform::Architecture;
use rubix_portainer::config::PortainerAgentConfig;
use rubix_portainer::service::PortainerService;
use rubix_supervisor::{
    Adapter, AdapterContext, AdapterError, AdapterFuture, CleanupKind, ComponentKind,
    ComponentSpec, FailureKind, FailurePolicy, Registration, StopCause, StopPhase, stop_channel,
};
use tempfile::TempDir;

#[derive(Clone, Default)]
struct LogProbe(Arc<Mutex<Vec<u8>>>);

impl LogProbe {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().expect("lock").clone()).expect("utf8")
    }
}

impl Write for LogProbe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn test_config(dir: &Path, debug: bool, local_path: bool) -> ValidatedConfig {
    let yaml = format!(
        r#"
path: "{}"
network:
  nodeIP: "127.0.0.1"
kubernetes:
  nodeName: "test-node"
logging:
  debug: {}
storage:
  localPath:
    enabled: {}
"#,
        dir.display(),
        debug,
        local_path
    );
    let config = resolve_layers(
        Some(decode(&yaml).expect("decode")),
        &BTreeMap::new(),
        &ExplicitFlags::default(),
        EnvironmentMode::Include,
        &HostContext {
            cpu_count: 4,
            architecture: "arm64".into(),
            detected_container_mode: false,
        },
    )
    .expect("resolve")
    .validated;
    assert_eq!(config.config().network.node_ip, "127.0.0.1");
    assert_eq!(config.config().kubernetes.node_name, "test-node");
    assert_eq!(config.config().storage.local_path.enabled, local_path);
    config
}

fn setup_cluster_infra(
    dir: &Path,
) -> (
    DatastoreEngine,
    Arc<ApiserverService>,
    ClusterPki,
    ApiserverConfig,
) {
    let node_ip: IpAddr = "127.0.0.1".parse().unwrap();
    let pki_dir = dir.join("pki");
    std::fs::create_dir_all(&pki_dir).unwrap();
    let pki_config = ClusterPkiConfig::new(pki_dir.clone(), "test-node".to_string(), node_ip);
    let pki = ClusterPki::new(pki_config);
    pki.reconcile().expect("PKI reconcile");

    let datastore_dir = dir.join("datastore");
    std::fs::create_dir_all(&datastore_dir).unwrap();
    let ds_config = DatastoreConfig::new(datastore_dir);
    let (engine, _) = DatastoreEngine::open(ds_config).expect("datastore open");
    let storage = KubernetesStorage::new(engine.client(), "/registry");

    let apiserver_cfg = ApiserverConfig::default_for_pki(&pki_dir, node_ip);
    let apiserver_service = Arc::new(ApiserverService::new(apiserver_cfg.clone(), storage));

    (engine, apiserver_service, pki, apiserver_cfg)
}

struct ClosureAdapter<F>(F);

impl<F, Fut> Adapter for ClosureAdapter<F>
where
    F: FnOnce(AdapterContext) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), AdapterError>> + Send + 'static,
{
    fn run(self: Box<Self>, context: AdapterContext) -> AdapterFuture {
        Box::pin((self.0)(context))
    }
}

#[tokio::test]
async fn test_optional_failure_against_healthy_kubernetes_api() {
    let temp = TempDir::new().unwrap();
    let config = test_config(temp.path(), false, false);
    let (engine, apiserver, _pki, _apicfg) = setup_cluster_infra(temp.path());
    apiserver.check_prerequisites().await.unwrap();

    let secret_token = "secret-token-must-not-leak-998877".to_string();
    let secret_clone = secret_token.clone();

    let timeout = Duration::from_secs(5);
    let datastore_reg =
        DatastoreAdapter::registration_for_engine(COMPONENT_DATASTORE, engine, timeout);
    let apiserver_reg = ApiserverAdapter::registration(
        COMPONENT_APISERVER,
        (*apiserver).clone(),
        vec![COMPONENT_DATASTORE.to_string()],
        timeout,
    );

    let runtime = RuntimeBuilder::new(config)
        .with_apiserver(apiserver)
        .register_component(datastore_reg)
        .register_component(apiserver_reg)
        .register_optional(
            "flaky-addon",
            vec![COMPONENT_APISERVER.to_string()],
            ClosureAdapter(move |mut context: AdapterContext| async move {
                assert!(!secret_clone.is_empty());
                // Mark ready briefly then fail to simulate an optional degradation
                context.ready();
                tokio::time::sleep(Duration::from_millis(50)).await;
                Err(AdapterError {
                    code: "addon_internal_failure",
                })
            }),
        )
        .build()
        .expect("build runtime");

    let client = runtime.client().expect("client must be present").clone();
    let probe = LogProbe::default();
    let (stop_handle, stop_receiver) = stop_channel();

    let run_handle =
        tokio::spawn(runtime.run_with_sink(stop_receiver, probe.clone(), FlushPolicy::EachFrame));

    // Wait until degradation is recorded in the sink
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let text = probe.text();
            if text.contains("\"event\":\"component_failure\"")
                && text.contains("\"event\":\"supervisor_degraded\"")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .expect("degradation logged to sink");

    // Assert that the sink log does NOT disclose the private secret token
    let text_before_stop = probe.text();
    assert!(
        !text_before_stop.contains(&secret_token),
        "Secret token leaked in lifecycle logs!"
    );
    assert!(
        text_before_stop.contains("\"detail_code\":\"addon_internal_failure\""),
        "Missing diagnostic code in logs: {text_before_stop}"
    );

    // Assert that the KubernetesApiClient remains fully healthy and functional
    client
        .create_namespace("degraded-test-ns")
        .await
        .expect("namespace creation succeeds on degraded node");

    let namespaces = client.list_namespaces().await.expect("list namespaces");
    let ns_list = namespaces["items"].as_array().expect("items array");
    assert!(
        ns_list
            .iter()
            .any(|n| n["metadata"]["name"] == "degraded-test-ns"),
        "Created namespace not found"
    );

    let mut data = BTreeMap::new();
    data.insert("config_key".to_string(), "sample_data".to_string());
    client
        .create_configmap("degraded-test-ns", "test-cm", data)
        .await
        .expect("configmap creation succeeds on degraded node");

    let cm = client
        .get_configmap("degraded-test-ns", "test-cm")
        .await
        .expect("get configmap succeeds on degraded node");
    assert_eq!(cm["data"]["config_key"], "sample_data");

    // Clean shutdown
    stop_handle.stop();
    let (sup_report, delivery_report, sink_report) = run_handle.await.expect("run task");
    assert_eq!(sup_report.cause, StopCause::Requested);
    assert_eq!(delivery_report.dropped_full, 0);
    assert_eq!(
        sink_report.outcome,
        rubix_kube::lifecycle_sink::SinkOutcome::Closed
    );

    let final_text = probe.text();
    assert!(!final_text.contains(&secret_token));
}

#[tokio::test]
async fn test_fatal_core_failure_terminates_supervisor_and_cancels_dependents() {
    let temp = TempDir::new().unwrap();
    let config = test_config(temp.path(), false, false);

    let secret = "fatal-core-secret-xyz".to_string();
    let runtime = RuntimeBuilder::new(config)
        .register_core(
            "broken-core-db",
            vec![],
            ClosureAdapter(move |_context: AdapterContext| async move {
                assert!(!secret.is_empty());
                Err(AdapterError {
                    code: "database_corruption_detected",
                })
            }),
        )
        .register_core(
            "dependent-service",
            vec!["broken-core-db".to_string()],
            ClosureAdapter(move |mut context: AdapterContext| async move {
                context.ready();
                loop {
                    if context.changed().await == rubix_supervisor::StopPhase::Graceful {
                        return Ok(());
                    }
                }
            }),
        )
        .build()
        .expect("build runtime");

    let probe = LogProbe::default();
    let (_stop_handle, stop_receiver) = stop_channel();

    let (sup_report, _delivery, _sink) = runtime
        .run_with_sink(stop_receiver, probe.clone(), FlushPolicy::EachFrame)
        .await;

    match &sup_report.cause {
        StopCause::Fatal(failure) => {
            assert_eq!(failure.component, "broken-core-db");
            assert_eq!(
                failure.kind,
                FailureKind::Adapter("database_corruption_detected")
            );
        },
        other => panic!("Expected fatal failure, got {other:?}"),
    }

    let text = probe.text();
    assert!(!text.contains("fatal-core-secret-xyz"));
    assert!(text.contains("\"detail_code\":\"database_corruption_detected\""));
    assert!(text.contains("\"event\":\"component_failure\""));
    assert!(text.contains("\"event\":\"supervisor_stop\""));
}

#[tokio::test]
async fn test_slow_startup_deadline_timeout_produces_secret_safe_diagnostics() {
    let temp = TempDir::new().unwrap();
    let config = test_config(temp.path(), false, false);

    let secret = "startup-timeout-secret-token".to_string();
    let short_timeout = Duration::from_millis(60);

    let spec = ComponentSpec {
        id: "slow-starting-service".to_string(),
        prerequisites: vec![],
        kind: ComponentKind::LongRunning,
        failure_policy: FailurePolicy::Fatal,
        startup_timeout: short_timeout,
    };

    let reg = Registration::new(
        spec,
        ClosureAdapter(move |_context: AdapterContext| async move {
            assert!(!secret.is_empty());
            // Intentionally sleep past the startup deadline without calling context.ready()
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok(())
        }),
    );

    let runtime = RuntimeBuilder::new(config)
        .register_component(reg)
        .build()
        .expect("build runtime");

    let probe = LogProbe::default();
    let (_stop_handle, stop_receiver) = stop_channel();

    let (sup_report, _delivery, _sink) = runtime
        .run_with_sink(stop_receiver, probe.clone(), FlushPolicy::EachFrame)
        .await;

    match &sup_report.cause {
        StopCause::Fatal(failure) => {
            assert_eq!(failure.component, "slow-starting-service");
            assert_eq!(failure.kind, FailureKind::StartupTimeout);
        },
        other => panic!("Expected startup timeout fatal failure, got {other:?}"),
    }

    let text = probe.text();
    assert!(!text.contains("startup-timeout-secret-token"));
    assert!(text.contains("\"event\":\"component_failure\""));
    assert!(text.contains("\"code\":\"startup_timeout\""));
}

#[tokio::test]
async fn test_debug_log_level_controls() {
    let temp1 = TempDir::new().unwrap();
    let config_info = test_config(temp1.path(), false, false);
    let probe_info = LogProbe::default();
    let (stop1, stop_rx1) = stop_channel();

    let runtime1 = RuntimeBuilder::new(config_info)
        .register_core(
            "dummy-worker",
            vec![],
            ClosureAdapter(|mut ctx: AdapterContext| async move {
                ctx.ready();
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok(())
            }),
        )
        .build()
        .expect("build runtime1");

    assert_eq!(runtime1.log_level(), LogLevel::Info);
    let h1 =
        tokio::spawn(runtime1.run_with_sink(stop_rx1, probe_info.clone(), FlushPolicy::EachFrame));
    tokio::time::sleep(Duration::from_millis(40)).await;
    stop1.stop();
    h1.await.unwrap();

    let info_text = probe_info.text();
    assert!(!info_text.contains("\"level\":\"debug\""));

    let temp2 = TempDir::new().unwrap();
    let config_debug = test_config(temp2.path(), true, false);
    let probe_debug = LogProbe::default();
    let (stop2, stop_rx2) = stop_channel();

    let runtime2 = RuntimeBuilder::new(config_debug)
        .register_core(
            "dummy-worker",
            vec![],
            ClosureAdapter(|mut ctx: AdapterContext| async move {
                ctx.ready();
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok(())
            }),
        )
        .build()
        .expect("build runtime2");

    assert_eq!(runtime2.log_level(), LogLevel::Debug);
    let h2 =
        tokio::spawn(runtime2.run_with_sink(stop_rx2, probe_debug.clone(), FlushPolicy::EachFrame));
    tokio::time::sleep(Duration::from_millis(40)).await;
    stop2.stop();
    h2.await.unwrap();

    let debug_text = probe_debug.text();
    assert!(debug_text.contains("\"level\":\"debug\""));
}

#[tokio::test]
async fn test_node_runtime_from_config_and_production_client() {
    let temp = TempDir::new().unwrap();
    let config = test_config(temp.path(), false, true);

    let runtime = NodeRuntime::from_config(config).expect("assemble from config");
    assert!(runtime.client().is_some());
    assert!(runtime.apiserver().is_some());
    assert_eq!(runtime.log_level(), LogLevel::Info);

    let client = runtime.client().unwrap().clone();
    let probe = LogProbe::default();
    let (stop_handle, stop_receiver) = stop_channel();

    let run_handle =
        tokio::spawn(runtime.run_with_sink(stop_receiver, probe.clone(), FlushPolicy::EachFrame));

    // Wait until apiserver is ready and serving
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if client.list_namespaces().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("apiserver becomes reachable");

    // Validate Kubernetes API interaction
    client
        .create_namespace("production-ns")
        .await
        .expect("create ns");
    let ns = client.get_namespace("production-ns").await.expect("get ns");
    assert_eq!(ns["metadata"]["name"], "production-ns");

    stop_handle.stop();
    let (report, delivery, sink) = run_handle.await.expect("join runtime task");
    assert_eq!(report.cause, StopCause::Requested);
    assert_eq!(delivery.dropped_full, 0);
    assert_eq!(
        sink.outcome,
        rubix_kube::lifecycle_sink::SinkOutcome::Closed
    );
}

#[tokio::test]
async fn test_c02_runtime_contract_downstream_wiring_interface() {
    let temp = TempDir::new().unwrap();
    let config = test_config(temp.path(), false, false);

    // Verify component constants exist and match expectations
    assert_eq!(COMPONENT_DATASTORE, "datastore");
    assert_eq!(COMPONENT_APISERVER, "apiserver");
    assert_eq!(COMPONENT_CONTROLLER_MANAGER, "controller-manager");
    assert_eq!(COMPONENT_COREDNS, "coredns");
    assert_eq!(COMPONENT_LOCAL_PATH, "local-path-provisioner");
    assert_eq!(COMPONENT_PORTAINER, "portainer-agent");
    assert_eq!(COMPONENT_PROXY, "kube-proxy");
    assert_eq!(COMPONENT_KUBELET, "kubelet");

    let (engine, apiserver, _pki, _apicfg) = setup_cluster_infra(temp.path());
    let client = apiserver.admin_client();

    let builder = RuntimeBuilder::new(config)
        .with_apiserver(apiserver)
        .with_client(client.clone());

    // C02 interface checks: builder exposes client and apiserver
    assert!(builder.client().is_some());
    assert!(builder.apiserver().is_some());

    // Component registration interface checks
    let timeout = Duration::from_secs(5);
    let datastore_reg =
        DatastoreAdapter::registration_for_engine(COMPONENT_DATASTORE, engine, timeout);

    let runtime = builder
        .register_component(datastore_reg)
        .register_core(
            "test-core-dep",
            vec![COMPONENT_DATASTORE.to_string()],
            ClosureAdapter(|mut ctx: AdapterContext| async move {
                ctx.ready();
                Ok(())
            }),
        )
        .register_optional(
            "test-opt-dep",
            vec![COMPONENT_DATASTORE.to_string()],
            ClosureAdapter(|mut ctx: AdapterContext| async move {
                ctx.ready();
                Ok(())
            }),
        )
        .build()
        .expect("build runtime");

    assert!(runtime.client().is_some());
    assert!(runtime.apiserver().is_some());
}

async fn assert_all_portainer_resources_exist(
    client: &rubix_apiserver::client::KubernetesApiClient,
) {
    assert!(client.get_namespace("portainer").await.is_ok());
    assert!(
        client
            .get_service_account("portainer", "portainer-sa-clusteradmin")
            .await
            .is_ok()
    );
    assert!(
        client
            .get_cluster_role_binding("portainer-crb-clusteradmin")
            .await
            .is_ok()
    );
    assert!(
        client
            .get_configmap("portainer", "portainer-agent-edge")
            .await
            .is_ok()
    );
    assert!(
        client
            .get_secret("portainer", "portainer-agent-edge-key")
            .await
            .is_ok()
    );
    assert!(
        client
            .get_service("portainer", "portainer-agent")
            .await
            .is_ok()
    );
    assert!(
        client
            .get_deployment("portainer", "portainer-agent")
            .await
            .is_ok()
    );
}

async fn mutate_portainer_configmap(client: &rubix_apiserver::client::KubernetesApiClient) {
    let mut data = BTreeMap::new();
    data.insert("EDGE_ID".to_string(), "edge-cluster-99".to_string());
    data.insert(
        "USER_CUSTOM_SETTING".to_string(),
        "persisted-value-12345".to_string(),
    );
    client
        .update_configmap("portainer", "portainer-agent-edge", data, None)
        .await
        .unwrap();

    let updated_cm = client
        .get_configmap("portainer", "portainer-agent-edge")
        .await
        .unwrap();
    assert_eq!(
        updated_cm["data"]["USER_CUSTOM_SETTING"],
        "persisted-value-12345"
    );
}

fn reopen_cluster_infra(dir: &Path) -> (DatastoreEngine, Arc<ApiserverService>, ApiserverConfig) {
    let datastore_dir = dir.join("datastore");
    let ds_config = DatastoreConfig::new(datastore_dir);
    let (engine, _) = DatastoreEngine::open(ds_config).expect("datastore reopen");
    let storage = KubernetesStorage::new(engine.client(), "/registry");
    let pki_dir = dir.join("pki");
    let node_ip: IpAddr = "127.0.0.1".parse().unwrap();
    let apiserver_cfg = ApiserverConfig::default_for_pki(&pki_dir, node_ip);
    let apiserver = Arc::new(ApiserverService::new(apiserver_cfg.clone(), storage));
    (engine, apiserver, apiserver_cfg)
}

#[tokio::test]
async fn test_portainer_wiring_and_object_preservation_across_runs() {
    let temp = TempDir::new().unwrap();
    let config = test_config(temp.path(), false, false);
    let (engine, apiserver, _pki, _apicfg) = setup_cluster_infra(temp.path());
    let client = Arc::new(apiserver.admin_client());

    let portainer_cfg = PortainerAgentConfig::new(
        "edge-cluster-99",
        "edge-key-cluster-99",
        Architecture::Arm64,
    )
    .with_readiness_timeout(Duration::ZERO);

    let portainer_svc = PortainerService::new(portainer_cfg.clone(), client.clone());

    let timeout = Duration::from_secs(5);
    let datastore_reg =
        DatastoreAdapter::registration_for_engine(COMPONENT_DATASTORE, engine, timeout);
    let apiserver_reg = ApiserverAdapter::registration(
        COMPONENT_APISERVER,
        (*apiserver).clone(),
        vec![COMPONENT_DATASTORE.to_string()],
        timeout,
    );

    let runtime1 = RuntimeBuilder::new(config.clone())
        .with_apiserver(apiserver.clone())
        .with_client((*client).clone())
        .register_component(datastore_reg)
        .register_component(apiserver_reg)
        .register_portainer(portainer_svc, vec![COMPONENT_APISERVER.to_string()])
        .build()
        .expect("build runtime1");

    let probe1 = LogProbe::default();
    let (stop1, stop_rx1) = stop_channel();
    let run1 =
        tokio::spawn(runtime1.run_with_sink(stop_rx1, probe1.clone(), FlushPolicy::EachFrame));

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if client
                .get_configmap("portainer", "portainer-agent-edge")
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("portainer configmap is reconciled");

    assert_all_portainer_resources_exist(&client).await;
    mutate_portainer_configmap(&client).await;

    stop1.stop();
    let (report1, _, _) = run1.await.unwrap();
    assert_eq!(report1.cause, StopCause::Requested);

    // Drop all handles referencing runtime 1's datastore to release .lock
    drop(client);
    drop(apiserver);

    // Re-open datastore and apiserver from same path to simulate restart
    let (engine2, apiserver2, _) = reopen_cluster_infra(temp.path());
    let client2 = Arc::new(apiserver2.admin_client());
    let portainer_svc2 = PortainerService::new(portainer_cfg, client2.clone());

    let datastore_reg2 =
        DatastoreAdapter::registration_for_engine(COMPONENT_DATASTORE, engine2, timeout);
    let apiserver_reg2 = ApiserverAdapter::registration(
        COMPONENT_APISERVER,
        (*apiserver2).clone(),
        vec![COMPONENT_DATASTORE.to_string()],
        timeout,
    );

    let runtime2 = RuntimeBuilder::new(config)
        .with_apiserver(apiserver2)
        .with_client((*client2).clone())
        .register_component(datastore_reg2)
        .register_component(apiserver_reg2)
        .register_portainer(portainer_svc2, vec![COMPONENT_APISERVER.to_string()])
        .build()
        .expect("build runtime2");

    let probe2 = LogProbe::default();
    let (stop2, stop_rx2) = stop_channel();
    let run2 =
        tokio::spawn(runtime2.run_with_sink(stop_rx2, probe2.clone(), FlushPolicy::EachFrame));

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify user change was preserved across restart
    let cm_after_restart = client2
        .get_configmap("portainer", "portainer-agent-edge")
        .await
        .unwrap();
    assert_eq!(
        cm_after_restart["data"]["USER_CUSTOM_SETTING"], "persisted-value-12345",
        "User modified configmap fields must be preserved across restarts"
    );

    stop2.stop();
    let (report2, _, _) = run2.await.unwrap();
    assert_eq!(report2.cause, StopCause::Requested);
}

#[tokio::test]
async fn test_node_runtime_from_config_with_portainer_enabled() {
    let temp = TempDir::new().unwrap();
    let yaml = format!(
        r#"
path: "{}"
network:
  node_ip: "127.0.0.1"
kubernetes:
  node_name: "test-node"
logging:
  debug: false
portainer:
  edgeID: "test-edge-id"
  edgeKey: "test-edge-key"
  async: false
"#,
        temp.path().display()
    );
    let config = resolve_layers(
        Some(decode(&yaml).expect("decode")),
        &BTreeMap::new(),
        &ExplicitFlags::default(),
        EnvironmentMode::Include,
        &HostContext {
            cpu_count: 4,
            architecture: "arm64".into(),
            detected_container_mode: false,
        },
    )
    .expect("resolve")
    .validated;

    assert_eq!(config.config().portainer.edge_id, "test-edge-id");
    assert_eq!(config.config().portainer.edge_key, "test-edge-key");

    let runtime = NodeRuntime::from_config(config).expect("assemble from config");
    assert!(runtime.client().is_some());
    assert!(runtime.apiserver().is_some());

    let client = runtime.client().unwrap().clone();
    let probe = LogProbe::default();
    let (stop_handle, stop_receiver) = stop_channel();

    let run_handle =
        tokio::spawn(runtime.run_with_sink(stop_receiver, probe.clone(), FlushPolicy::EachFrame));

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if client
                .get_configmap("portainer", "portainer-agent-edge")
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("portainer configmap is created via from_config");

    let cm = client
        .get_configmap("portainer", "portainer-agent-edge")
        .await
        .unwrap();
    assert_eq!(cm["data"]["EDGE_ID"], "test-edge-id");

    stop_handle.stop();
    let (report, _, _) = run_handle.await.expect("join runtime task");
    assert_eq!(report.cause, StopCause::Requested);
}

#[tokio::test]
async fn test_portainer_optional_failure_degrades_supervisor_gracefully() {
    let temp = TempDir::new().unwrap();
    let config = test_config(temp.path(), false, false);
    let (engine, apiserver, _pki, _apicfg) = setup_cluster_infra(temp.path());
    let client = Arc::new(apiserver.admin_client());

    // Config that fails readiness: readiness_timeout = 50ms
    let portainer_cfg =
        PortainerAgentConfig::new("edge-fail-id", "edge-fail-key", Architecture::Arm64)
            .with_readiness_timeout(Duration::from_millis(50));

    let portainer_svc = PortainerService::new(portainer_cfg, client.clone());

    let timeout = Duration::from_secs(5);
    let datastore_reg =
        DatastoreAdapter::registration_for_engine(COMPONENT_DATASTORE, engine, timeout);
    let apiserver_reg = ApiserverAdapter::registration(
        COMPONENT_APISERVER,
        (*apiserver).clone(),
        vec![COMPONENT_DATASTORE.to_string()],
        timeout,
    );

    let runtime = RuntimeBuilder::new(config)
        .with_apiserver(apiserver)
        .with_client((*client).clone())
        .register_component(datastore_reg)
        .register_component(apiserver_reg)
        .register_portainer(portainer_svc, vec![COMPONENT_APISERVER.to_string()])
        .build()
        .expect("build runtime");

    let probe = LogProbe::default();
    let (stop_handle, stop_receiver) = stop_channel();
    let run_handle =
        tokio::spawn(runtime.run_with_sink(stop_receiver, probe.clone(), FlushPolicy::EachFrame));

    // Wait until degradation is recorded in the sink
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let text = probe.text();
            if text.contains("\"event\":\"component_failure\"")
                && text.contains("\"event\":\"supervisor_degraded\"")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .expect("portainer degradation logged to sink");

    let text_before_stop = probe.text();
    assert!(
        text_before_stop.contains("\"detail_code\":\"portainer_readiness_timeout\""),
        "Missing diagnostic code in logs: {text_before_stop}"
    );

    // Verify that apiserver remains fully operational and healthy!
    let ns_list = client
        .list_namespaces()
        .await
        .expect("apiserver is alive and responding");
    let namespaces = ns_list["items"].as_array().expect("items array");
    assert!(!namespaces.is_empty());

    stop_handle.stop();
    let (report, _, _) = run_handle.await.expect("join runtime");
    assert_eq!(report.cause, StopCause::Requested);

    let portainer_failure = report
        .failures
        .iter()
        .find(|f| f.component == COMPONENT_PORTAINER);
    assert!(portainer_failure.is_some());
    assert_eq!(
        portainer_failure.unwrap().kind,
        FailureKind::Adapter("portainer_readiness_timeout")
    );
}

#[tokio::test]
async fn test_shutdown_error_preserves_original_cause() {
    let temp = TempDir::new().unwrap();
    let config = test_config(temp.path(), true, false);
    let secret = "shutdown-secret-must-not-leak-445566".to_string();
    let secret_log = secret.clone();
    let (stop_handle, stop_receiver) = stop_channel();
    let probe = LogProbe::default();
    let runtime = RuntimeBuilder::new(config)
        .register_core(
            "core",
            vec![],
            ClosureAdapter(move |mut ctx: AdapterContext| async move {
                assert!(!secret.is_empty());
                ctx.ready();
                loop {
                    match ctx.changed().await {
                        StopPhase::Running => {},
                        StopPhase::Graceful | StopPhase::Force => {
                            return Err(AdapterError {
                                code: "shutdown_flush_failed",
                            });
                        },
                    }
                }
            }),
        )
        .build()
        .expect("build runtime");

    let running =
        tokio::spawn(runtime.run_with_sink(stop_receiver, probe.clone(), FlushPolicy::EachFrame));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if probe.text().contains("\"state\":\"ready\"") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("component ready");
    stop_handle.stop();
    let (report, _, _) = running.await.expect("runtime finished");
    let logs = probe.text();
    assert_eq!(report.cause, StopCause::Requested);
    assert!(report.cleanup_failures.iter().any(|failure| {
        failure.component == "core" && failure.kind == CleanupKind::Adapter("shutdown_flush_failed")
    }));
    assert!(logs.contains("\"event\":\"component_cleanup\""));
    assert!(logs.contains("\"code\":\"adapter\""));
    assert!(logs.contains("\"detail_code\":\"shutdown_flush_failed\""));
    assert!(logs.contains("\"event\":\"supervisor_stop\""));
    assert!(logs.contains("\"code\":\"requested\""));
    assert!(!logs.contains(secret_log.as_str()));
}

#[tokio::test]
async fn test_config_api_registration_and_supervised_lifecycle() {
    assert_eq!(COMPONENT_CONFIG_API, "configapi");

    let temp = TempDir::new().unwrap();
    let config = test_config(temp.path(), false, false);
    let socket_path = temp.path().join("config.sock");
    let config_file = temp.path().join("config.yaml");
    write_document(&config_file, config.config()).unwrap();

    let server = ConfigApiServer::new(socket_path.clone(), config_file, HostContext::default());

    let (stop_handle, stop_receiver) = stop_channel();
    let runtime = RuntimeBuilder::new(config)
        .register_config_api(server)
        .build()
        .expect("build runtime with config api");

    let run_handle = tokio::spawn(async move {
        runtime
            .run_with_sink(stop_receiver, io::sink(), FlushPolicy::EachFrame)
            .await
    });

    let mut ready = false;
    for _ in 0..50 {
        if check_healthz(&socket_path).await {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        ready,
        "config API server should become ready and serve /healthz"
    );

    // Clean shutdown
    stop_handle.stop();
    let (sup_report, _, _) = run_handle.await.expect("run task");
    assert_eq!(sup_report.cause, StopCause::Requested);
    assert!(
        !socket_path.exists(),
        "socket must be removed on supervisor shutdown"
    );
}

async fn check_healthz(socket_path: &Path) -> bool {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    if !socket_path.exists() {
        return false;
    }
    let Ok(mut stream) = tokio::net::UnixStream::connect(socket_path).await else {
        return false;
    };
    let req = b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    if stream.write_all(req).await.is_err() {
        return false;
    }
    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).await.is_ok() && resp.ends_with(b"ok\n")
}
