use std::fs;
use std::sync::Arc;
use std::time::Duration;

use rubix_network::MockCommandExecutor;
use rubix_proxy::{
    COMPONENT_PROXY, DEFAULT_STARTUP_TIMEOUT, KubeProxyOptions, ProxyAdapter, ProxyMode,
    ProxyService,
};
use rubix_supervisor::{StopCause, Supervisor, stop_channel};
use tempfile::TempDir;

#[tokio::test]
async fn test_supervisor_adapter_lifecycle() {
    let temp = TempDir::new().unwrap();
    let kubeconfig = temp.path().join("admin.kubeconfig");
    fs::write(&kubeconfig, "apiVersion: v1\nkind: Config\n").unwrap();

    let mock = MockCommandExecutor::new_nftables_host();
    let options = KubeProxyOptions::new(
        kubeconfig,
        true, // container_mode = true
        ProxyMode::Nftables,
    );

    let service = ProxyService::new(options)
        .with_executor(Arc::new(mock))
        .with_sys_root(temp.path().to_path_buf());

    let reg = ProxyAdapter::registration(
        COMPONENT_PROXY,
        service.clone(),
        vec![],
        DEFAULT_STARTUP_TIMEOUT,
    );

    let supervisor = Supervisor::new(vec![reg]).expect("supervisor creation succeeds");
    let (stop_handle, stop_receiver) = stop_channel();
    let sup_handle = tokio::spawn(async move { supervisor.run(stop_receiver).await });

    // Allow supervisor to start proxy adapter and confirm readiness
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !service.is_running() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        service.is_running(),
        "Proxy service must be active after startup"
    );

    let report = service.check_readiness().unwrap();
    assert!(report.is_healthy);
    assert_eq!(report.proxy_mode, ProxyMode::Nftables);
    assert!(report.container_mode);
    assert!(report.all_conntrack_zero);

    stop_handle.stop();
    let report = sup_handle.await.expect("supervisor task joins");
    assert!(matches!(report.cause, StopCause::Requested));
    assert!(!service.is_running());
}
