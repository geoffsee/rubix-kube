#![allow(clippy::too_many_lines)]

use std::fs;
use std::sync::Arc;

use rubix_network::MockCommandExecutor;
use rubix_proxy::{
    DataplaneProber, EndpointItem, EndpointPort, EndpointSliceDefinition, IptablesDataplane,
    KubeProxyOptions, NftablesDataplane, Protocol, ProxyError, ProxyMode, ProxyService,
    ServiceDefinition, ServicePort, ServiceRoutingTable, ServiceType,
};
use tempfile::TempDir;

#[test]
fn test_tcp_and_udp_workload_probes_reach_correct_backends() {
    let mut table = ServiceRoutingTable::new();

    // 1. Setup a TCP Service (web: ClusterIP + NodePort)
    let web_svc = ServiceDefinition::new(
        "tier1-workload",
        "web",
        ServiceType::NodePort,
        Some("10.43.0.100".to_string()),
        vec![ServicePort {
            name: Some("http".to_string()),
            protocol: Protocol::Tcp,
            port: 80,
            target_port: 8080,
            node_port: Some(30080),
        }],
    );
    table.apply_service(web_svc);

    let web_slice = EndpointSliceDefinition::new(
        "tier1-workload",
        "web-slice-1",
        "web",
        vec![EndpointPort {
            name: Some("http".to_string()),
            port: Some(8080),
            protocol: Some(Protocol::Tcp),
        }],
        vec![
            EndpointItem::new(
                vec!["10.42.0.10".to_string()],
                true,
                Some("node-1".to_string()),
            ),
            EndpointItem::new(
                vec!["10.42.0.11".to_string()],
                true,
                Some("node-1".to_string()),
            ),
        ],
    );
    table.apply_endpoint_slice(web_slice);

    // 2. Setup a UDP Service (dns: ClusterIP + NodePort)
    let dns_svc = ServiceDefinition::new(
        "kube-system",
        "kube-dns",
        ServiceType::NodePort,
        Some("10.43.0.10".to_string()),
        vec![ServicePort {
            name: Some("dns".to_string()),
            protocol: Protocol::Udp,
            port: 53,
            target_port: 53,
            node_port: Some(30053),
        }],
    );
    table.apply_service(dns_svc);

    let dns_slice = EndpointSliceDefinition::new(
        "kube-system",
        "kube-dns-slice-1",
        "kube-dns",
        vec![EndpointPort {
            name: Some("dns".to_string()),
            port: Some(53),
            protocol: Some(Protocol::Udp),
        }],
        vec![
            EndpointItem::new(
                vec!["10.42.0.20".to_string()],
                true,
                Some("node-1".to_string()),
            ),
            EndpointItem::new(
                vec!["10.42.0.21".to_string()],
                true,
                Some("node-1".to_string()),
            ),
        ],
    );
    table.apply_endpoint_slice(dns_slice);

    let prober = DataplaneProber::new();

    // Verify TCP ClusterIP probe
    let tcp_res = prober
        .probe_service_cluster_ip(&table, "tier1-workload", "web", Protocol::Tcp)
        .unwrap();
    assert!(tcp_res.success);
    let target = tcp_res.reached_endpoint.unwrap();
    assert_eq!(target.port, 8080);
    assert_eq!(target.protocol, Protocol::Tcp);
    assert!(target.ip == "10.42.0.10" || target.ip == "10.42.0.11");

    // Verify TCP NodePort probe
    let tcp_np_res = prober
        .probe_service_node_port(&table, "tier1-workload", "web", Protocol::Tcp)
        .unwrap();
    assert!(tcp_np_res.success);
    let target_np = tcp_np_res.reached_endpoint.unwrap();
    assert_eq!(target_np.port, 8080);
    assert_eq!(target_np.protocol, Protocol::Tcp);

    // Verify UDP ClusterIP probe
    let udp_res = prober
        .probe_service_cluster_ip(&table, "kube-system", "kube-dns", Protocol::Udp)
        .unwrap();
    assert!(udp_res.success);
    let target_udp = udp_res.reached_endpoint.unwrap();
    assert_eq!(target_udp.port, 53);
    assert_eq!(target_udp.protocol, Protocol::Udp);
    assert!(target_udp.ip == "10.42.0.20" || target_udp.ip == "10.42.0.21");

    // Verify UDP NodePort probe
    let udp_np_res = prober
        .probe_service_node_port(&table, "kube-system", "kube-dns", Protocol::Udp)
        .unwrap();
    assert!(udp_np_res.success);
    let target_udp_np = udp_np_res.reached_endpoint.unwrap();
    assert_eq!(target_udp_np.port, 53);
    assert_eq!(target_udp_np.protocol, Protocol::Udp);

    // Verify protocol isolation: TCP probe against UDP service fails
    let mismatch_res =
        prober.probe_service_cluster_ip(&table, "kube-system", "kube-dns", Protocol::Tcp);
    assert!(mismatch_res.is_err());

    // Verify protocol isolation: UDP probe against TCP service fails
    let mismatch_res2 =
        prober.probe_service_cluster_ip(&table, "tier1-workload", "web", Protocol::Udp);
    assert!(mismatch_res2.is_err());
}

#[test]
fn test_endpoint_additions_and_removals_dynamically_update_traffic_routing() {
    let mut table = ServiceRoutingTable::new();

    let svc = ServiceDefinition::new(
        "default",
        "api-svc",
        ServiceType::ClusterIP,
        Some("10.43.10.50".to_string()),
        vec![ServicePort {
            name: Some("http".to_string()),
            protocol: Protocol::Tcp,
            port: 80,
            target_port: 80,
            node_port: None,
        }],
    );
    table.apply_service(svc);

    // 1. Initial EndpointSlice with single endpoint
    let initial_slice = EndpointSliceDefinition::new(
        "default",
        "api-svc-slice",
        "api-svc",
        vec![EndpointPort {
            name: Some("http".to_string()),
            port: Some(80),
            protocol: Some(Protocol::Tcp),
        }],
        vec![EndpointItem::new(
            vec!["10.42.1.10".to_string()],
            true,
            None,
        )],
    );
    table.apply_endpoint_slice(initial_slice);

    let prober = DataplaneProber::new();

    // Must route strictly to 10.42.1.10
    for _ in 0..5 {
        let res = prober
            .probe_route(&table, "10.43.10.50", 80, Protocol::Tcp)
            .unwrap();
        assert_eq!(res.reached_endpoint.unwrap().ip, "10.42.1.10");
    }

    // 2. Endpoint Addition: scale out to 3 endpoints
    let updated_slice = EndpointSliceDefinition::new(
        "default",
        "api-svc-slice",
        "api-svc",
        vec![EndpointPort {
            name: Some("http".to_string()),
            port: Some(80),
            protocol: Some(Protocol::Tcp),
        }],
        vec![
            EndpointItem::new(vec!["10.42.1.10".to_string()], true, None),
            EndpointItem::new(vec!["10.42.1.11".to_string()], true, None),
            EndpointItem::new(vec!["10.42.1.12".to_string()], true, None),
        ],
    );
    table.apply_endpoint_slice(updated_slice);

    // Verify that traffic now distributes across all 3 endpoints
    let mut seen_ips = std::collections::BTreeSet::new();
    for _ in 0..12 {
        let res = prober
            .probe_route(&table, "10.43.10.50", 80, Protocol::Tcp)
            .unwrap();
        seen_ips.insert(res.reached_endpoint.unwrap().ip);
    }
    assert_eq!(seen_ips.len(), 3);
    assert!(seen_ips.contains("10.42.1.10"));
    assert!(seen_ips.contains("10.42.1.11"));
    assert!(seen_ips.contains("10.42.1.12"));

    // 3. Endpoint Removal: scale down, remove 10.42.1.10
    let scaled_down_slice = EndpointSliceDefinition::new(
        "default",
        "api-svc-slice",
        "api-svc",
        vec![EndpointPort {
            name: Some("http".to_string()),
            port: Some(80),
            protocol: Some(Protocol::Tcp),
        }],
        vec![
            EndpointItem::new(vec!["10.42.1.11".to_string()], true, None),
            EndpointItem::new(vec!["10.42.1.12".to_string()], true, None),
        ],
    );
    table.apply_endpoint_slice(scaled_down_slice);

    seen_ips.clear();
    for _ in 0..10 {
        let res = prober
            .probe_route(&table, "10.43.10.50", 80, Protocol::Tcp)
            .unwrap();
        let ip = res.reached_endpoint.unwrap().ip;
        assert_ne!(
            ip, "10.42.1.10",
            "removed endpoint must not receive traffic"
        );
        seen_ips.insert(ip);
    }
    assert_eq!(seen_ips.len(), 2);
    assert!(seen_ips.contains("10.42.1.11"));
    assert!(seen_ips.contains("10.42.1.12"));

    // 4. Endpoint Unreadiness: mark 10.42.1.12 as not ready
    let unready_slice = EndpointSliceDefinition::new(
        "default",
        "api-svc-slice",
        "api-svc",
        vec![EndpointPort {
            name: Some("http".to_string()),
            port: Some(80),
            protocol: Some(Protocol::Tcp),
        }],
        vec![
            EndpointItem::new(vec!["10.42.1.11".to_string()], true, None),
            EndpointItem::new(vec!["10.42.1.12".to_string()], false, None), // ready = false
        ],
    );
    table.apply_endpoint_slice(unready_slice);

    for _ in 0..5 {
        let res = prober
            .probe_route(&table, "10.43.10.50", 80, Protocol::Tcp)
            .unwrap();
        assert_eq!(
            res.reached_endpoint.unwrap().ip,
            "10.42.1.11",
            "unready endpoint must be excluded from traffic routing"
        );
    }

    // 5. Total endpoint outage: all endpoints unready
    let all_unready_slice = EndpointSliceDefinition::new(
        "default",
        "api-svc-slice",
        "api-svc",
        vec![EndpointPort {
            name: Some("http".to_string()),
            port: Some(80),
            protocol: Some(Protocol::Tcp),
        }],
        vec![
            EndpointItem::new(vec!["10.42.1.11".to_string()], false, None),
            EndpointItem::new(vec!["10.42.1.12".to_string()], false, None),
        ],
    );
    table.apply_endpoint_slice(all_unready_slice);

    let err = prober
        .probe_route(&table, "10.43.10.50", 80, Protocol::Tcp)
        .unwrap_err();
    match err {
        ProxyError::DataplaneProbeFailed { service, reason } => {
            assert!(service.contains("10.43.10.50:80/TCP"));
            assert!(reason.contains("no ready endpoints"));
        },
        other => panic!("expected DataplaneProbeFailed error, got: {other:?}"),
    }
}

#[test]
fn test_iptables_and_nftables_dataplane_rule_synthesis() {
    let mut table = ServiceRoutingTable::new();

    let svc = ServiceDefinition::new(
        "tier1-workload",
        "web-clusterip",
        ServiceType::NodePort,
        Some("10.43.0.100".to_string()),
        vec![ServicePort {
            name: Some("http".to_string()),
            protocol: Protocol::Tcp,
            port: 80,
            target_port: 80,
            node_port: Some(30080),
        }],
    );
    table.apply_service(svc);

    let slice = EndpointSliceDefinition::new(
        "tier1-workload",
        "web-slice",
        "web-clusterip",
        vec![EndpointPort {
            name: Some("http".to_string()),
            port: Some(80),
            protocol: Some(Protocol::Tcp),
        }],
        vec![
            EndpointItem::new(vec!["10.42.0.10".to_string()], true, None),
            EndpointItem::new(vec!["10.42.0.11".to_string()], true, None),
        ],
    );
    table.apply_endpoint_slice(slice);

    // 1. Test Iptables rule generation
    let iptables_rules = IptablesDataplane::generate_rules(&table);
    assert!(!iptables_rules.is_empty());

    let has_clusterip_rule = iptables_rules.iter().any(|r| {
        r.chain == "KUBE-SERVICES"
            && r.rule
                .contains("-d 10.43.0.100/32 -p tcp -m tcp --dport 80")
    });
    assert!(has_clusterip_rule, "iptables rules must route ClusterIP");

    let has_nodeport_rule = iptables_rules
        .iter()
        .any(|r| r.chain == "KUBE-NODEPORTS" && r.rule.contains("-p tcp -m tcp --dport 30080"));
    assert!(has_nodeport_rule, "iptables rules must route NodePort");

    let has_dnat_rule = iptables_rules.iter().any(|r| {
        r.rule.contains("DNAT --to-destination 10.42.0.10:80")
            || r.rule.contains("DNAT --to-destination 10.42.0.11:80")
    });
    assert!(has_dnat_rule, "iptables rules must DNAT to endpoints");

    // 2. Test Nftables rule generation
    let nftables_rules = NftablesDataplane::generate_rules(&table);
    assert!(!nftables_rules.is_empty());

    let has_nft_clusterip = nftables_rules.iter().any(|r| {
        r.chain == "services"
            && r.statement
                .contains("ip daddr 10.43.0.100 tcp dport 80 dnat")
    });
    assert!(has_nft_clusterip, "nftables rules must route ClusterIP");

    let has_nft_nodeport = nftables_rules
        .iter()
        .any(|r| r.chain == "nodeports" && r.statement.contains("tcp dport 30080 dnat"));
    assert!(has_nft_nodeport, "nftables rules must route NodePort");
}

#[tokio::test]
async fn test_logs_and_readiness_distinguish_component_startup_from_dataplane_probes() {
    let temp = TempDir::new().unwrap();
    let kubeconfig = temp.path().join("admin.kubeconfig");
    fs::write(&kubeconfig, "apiVersion: v1\nkind: Config\n").unwrap();

    let proc_net = temp.path().join("proc/net");
    fs::create_dir_all(&proc_net).unwrap();
    fs::write(proc_net.join("ip_tables_names"), "filter\nnat\n").unwrap();

    let mock = MockCommandExecutor::new_iptables_host();
    let options = KubeProxyOptions::new(kubeconfig, true, ProxyMode::IpTables);

    let mut service = ProxyService::new(options)
        .with_executor(Arc::new(mock))
        .with_sys_root(temp.path().to_path_buf());

    assert!(service.check_prerequisites().is_ok());
    assert!(service.start().is_ok());
    assert!(service.is_running());

    // 1. Component startup readiness
    // At this stage, component process is running and SNAT is established,
    // but dataplane probes have not run yet.
    let startup_health = service.check_startup_readiness().unwrap();
    assert!(startup_health.is_healthy);
    assert!(startup_health.startup_ready);
    assert!(!startup_health.dataplane_ready);
    assert!(service.is_ready());
    assert!(!service.is_dataplane_ready());

    let readiness_report = service.check_readiness().unwrap();
    assert!(readiness_report.startup_ready);
    assert!(!readiness_report.dataplane_ready);
    assert_eq!(
        readiness_report.details.get("dataplane_status").unwrap(),
        "dataplane probes not yet verified"
    );

    // 2. Configure Service and EndpointSlice in routing table
    {
        let mut table = service.routing_table().write().await;
        table.apply_service(ServiceDefinition::new(
            "tier1-workload",
            "web-clusterip",
            ServiceType::ClusterIP,
            Some("10.43.0.100".to_string()),
            vec![ServicePort {
                name: Some("http".to_string()),
                protocol: Protocol::Tcp,
                port: 80,
                target_port: 80,
                node_port: None,
            }],
        ));
        table.apply_endpoint_slice(EndpointSliceDefinition::new(
            "tier1-workload",
            "web-slice",
            "web-clusterip",
            vec![EndpointPort {
                name: Some("http".to_string()),
                port: Some(80),
                protocol: Some(Protocol::Tcp),
            }],
            vec![EndpointItem::new(
                vec!["10.42.0.10".to_string()],
                true,
                None,
            )],
        ));
    }

    // 3. Execute dataplane verification
    let prober = DataplaneProber::new();
    let dp_report = service.verify_dataplane(&prober).await.unwrap();
    assert!(dp_report.all_passed());
    assert!(dp_report.tcp_clusterip_probes_passed);
    assert!(service.is_dataplane_ready());

    // Now readiness clearly indicates BOTH component startup and dataplane probe success
    let full_readiness = service.check_readiness().unwrap();
    assert!(full_readiness.startup_ready);
    assert!(full_readiness.dataplane_ready);
    assert_eq!(
        full_readiness.details.get("dataplane_status").unwrap(),
        "dataplane probes verified"
    );

    // 4. Test failure case: If all endpoints become unready, dataplane probes fail
    // while component startup remains intact!
    {
        let mut table = service.routing_table().write().await;
        table.apply_endpoint_slice(EndpointSliceDefinition::new(
            "tier1-workload",
            "web-slice",
            "web-clusterip",
            vec![EndpointPort {
                name: Some("http".to_string()),
                port: Some(80),
                protocol: Some(Protocol::Tcp),
            }],
            vec![EndpointItem::new(
                vec!["10.42.0.10".to_string()],
                false,
                None,
            )], // unready
        ));
    }

    let err = service.verify_dataplane(&prober).await.unwrap_err();
    match err {
        ProxyError::DataplaneProbeFailed { .. } => {},
        other => panic!("expected DataplaneProbeFailed error, got: {other:?}"),
    }

    // Component startup is still ready, but dataplane is not ready!
    assert!(service.is_ready());
    assert!(!service.is_dataplane_ready());

    let degraded_report = service.check_readiness().unwrap();
    assert!(degraded_report.startup_ready);
    assert!(!degraded_report.dataplane_ready);
    assert_eq!(
        degraded_report.details.get("dataplane_status").unwrap(),
        "dataplane probes not yet verified"
    );

    service.stop();
    assert!(!service.is_running());
    assert!(!service.is_ready());
    assert!(!service.is_dataplane_ready());
}
