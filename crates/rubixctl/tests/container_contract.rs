use rubixctl::container::{
    ContainerConfig, ContainerEngineClient, ContainerInspect, ContainerInstallParams,
    CreateNetworkRequest, CreateVolumeRequest, container_name, install_container, network_name,
    volume_name,
};
use rubixctl::container_fixtures::{
    serialize_container_create_request, serialize_network_create_request,
    serialize_volume_create_request,
};
use rubixctl::container_ports::{PortMapping, PortParseError, parse_container_ports};
use std::collections::HashMap;
use std::io;

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct MockContainerEngine {
    networks: HashMap<String, CreateNetworkRequest>,
    volumes: HashMap<String, CreateVolumeRequest>,
    images: HashMap<String, bool>,
    containers: HashMap<String, (ContainerConfig, bool, HashMap<String, u16>)>, // (config, running, allocated_ports)

    // Injected failure flags
    fail_create_network: bool,
    fail_create_volume: bool,
    fail_pull_image: bool,
    fail_create_container: bool,
    fail_start_container: bool,
    fail_post_start_inspect: bool,

    // Audit logs of operations
    created_networks: Vec<String>,
    removed_networks: Vec<String>,
    created_volumes: Vec<String>,
    removed_volumes: Vec<String>,
    created_containers: Vec<String>,
    started_containers: Vec<String>,
    stopped_containers: Vec<String>,
    removed_containers: Vec<String>,
}

impl ContainerEngineClient for MockContainerEngine {
    fn inspect_network(&mut self, name: &str) -> io::Result<Option<()>> {
        Ok(self.networks.get(name).map(|_| ()))
    }

    fn create_network(&mut self, req: &CreateNetworkRequest) -> io::Result<()> {
        if self.fail_create_network {
            return Err(io::Error::other("mock fail create network"));
        }
        self.networks.insert(req.name.clone(), req.clone());
        self.created_networks.push(req.name.clone());
        Ok(())
    }

    fn remove_network(&mut self, name: &str) -> io::Result<()> {
        self.networks.remove(name);
        self.removed_networks.push(name.to_string());
        Ok(())
    }

    fn inspect_volume(&mut self, name: &str) -> io::Result<Option<()>> {
        Ok(self.volumes.get(name).map(|_| ()))
    }

    fn create_volume(&mut self, req: &CreateVolumeRequest) -> io::Result<()> {
        if self.fail_create_volume {
            return Err(io::Error::other("mock fail create volume"));
        }
        self.volumes.insert(req.name.clone(), req.clone());
        self.created_volumes.push(req.name.clone());
        Ok(())
    }

    fn remove_volume(&mut self, name: &str) -> io::Result<()> {
        self.volumes.remove(name);
        self.removed_volumes.push(name.to_string());
        Ok(())
    }

    fn inspect_image(&mut self, image: &str) -> io::Result<Option<()>> {
        Ok(self.images.get(image).map(|_| ()))
    }

    fn pull_image(&mut self, image: &str) -> io::Result<()> {
        if self.fail_pull_image {
            return Err(io::Error::other("mock fail pull image"));
        }
        self.images.insert(image.to_string(), true);
        Ok(())
    }

    fn inspect_container(&mut self, name: &str) -> io::Result<Option<ContainerInspect>> {
        if self.fail_post_start_inspect && self.started_containers.contains(&name.to_string()) {
            return Err(io::Error::other("mock fail post start inspect"));
        }
        if let Some((_, running, ports)) = self.containers.get(name) {
            Ok(Some(ContainerInspect {
                id: format!("id-{name}"),
                name: name.to_string(),
                running: *running,
                exit_code: 0,
                allocated_ports: ports.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    fn create_container(&mut self, name: &str, config: &ContainerConfig) -> io::Result<String> {
        if self.fail_create_container {
            return Err(io::Error::other("mock fail create container"));
        }
        let mut allocated = HashMap::new();
        // Allocate ports matching bindings
        for (port_key, bindings) in &config.port_bindings {
            for b in bindings {
                let host_p = resolve_mock_port(port_key, &b.host_port);
                allocated.insert(port_key.clone(), host_p);
            }
        }
        self.containers
            .insert(name.to_string(), (config.clone(), false, allocated));
        self.created_containers.push(name.to_string());
        Ok(name.to_string())
    }

    fn start_container(&mut self, id_or_name: &str) -> io::Result<()> {
        if self.fail_start_container {
            return Err(io::Error::other("mock fail start container"));
        }
        if let Some((_, running, _)) = self.containers.get_mut(id_or_name) {
            *running = true;
        }
        self.started_containers.push(id_or_name.to_string());
        Ok(())
    }

    fn stop_container(&mut self, id_or_name: &str, _timeout: u32) -> io::Result<()> {
        if let Some((_, running, _)) = self.containers.get_mut(id_or_name) {
            *running = false;
        }
        self.stopped_containers.push(id_or_name.to_string());
        Ok(())
    }

    fn remove_container(&mut self, id_or_name: &str, _force: bool) -> io::Result<()> {
        self.containers.remove(id_or_name);
        self.removed_containers.push(id_or_name.to_string());
        Ok(())
    }
}

fn resolve_mock_port(port_key: &str, host_port: &str) -> u16 {
    if host_port.is_empty() {
        if port_key == "6443/tcp" { 32768 } else { 32769 }
    } else {
        host_port.parse().unwrap_or(0)
    }
}

// -----------------------------------------------------------------------------
// 1. Port Parsing and Validation Tests
// -----------------------------------------------------------------------------

#[test]
fn test_port_parsing_comprehensive() {
    // Single bare port defaults to 1:1 tcp
    assert_eq!(
        parse_container_ports("80").unwrap(),
        vec![PortMapping {
            host_ip: String::new(),
            host_port: 80,
            container_port: 80,
            protocol: "tcp".into(),
        }]
    );

    // Protocol uppercase converted to lowercase
    assert_eq!(
        parse_container_ports("53/UDP").unwrap(),
        vec![PortMapping {
            host_ip: String::new(),
            host_port: 53,
            container_port: 53,
            protocol: "udp".into(),
        }]
    );

    // Host IP binding with explicit ports
    assert_eq!(
        parse_container_ports("127.0.0.1:8443:443/tcp").unwrap(),
        vec![PortMapping {
            host_ip: "127.0.0.1".into(),
            host_port: 8443,
            container_port: 443,
            protocol: "tcp".into(),
        }]
    );

    // Range 8000-8002
    let range_res = parse_container_ports("8000-8002").unwrap();
    assert_eq!(range_res.len(), 3);
    assert_eq!(range_res[0].host_port, 8000);
    assert_eq!(range_res[1].host_port, 8001);
    assert_eq!(range_res[2].host_port, 8002);

    // Duplicate detection
    assert!(matches!(
        parse_container_ports("80, 80").unwrap_err(),
        PortParseError::DuplicateHostPort(80, _)
    ));

    // Invalid protocols rejected
    assert!(matches!(
        parse_container_ports("80/sctp").unwrap_err(),
        PortParseError::UnsupportedProtocol(_)
    ));
}

// -----------------------------------------------------------------------------
// 2. Resource Naming Tests
// -----------------------------------------------------------------------------

#[test]
fn test_resource_naming_conventions() {
    // Default / "rubix" / "kubesolo" -> "kubesolo"
    assert_eq!(container_name(""), "kubesolo");
    assert_eq!(container_name("rubix"), "kubesolo");
    assert_eq!(container_name("kubesolo"), "kubesolo");
    assert_eq!(network_name("rubix"), "kubesolo-net");
    assert_eq!(volume_name("rubix"), "kubesolo-data");

    // Named cluster "dev" -> "kubesolo-dev"
    assert_eq!(container_name("dev"), "kubesolo-dev");
    assert_eq!(network_name("dev"), "kubesolo-dev-net");
    assert_eq!(volume_name("dev"), "kubesolo-dev-data");

    // Named cluster "prod-cluster" -> "kubesolo-prod-cluster"
    assert_eq!(container_name("prod-cluster"), "kubesolo-prod-cluster");
    assert_eq!(network_name("prod-cluster"), "kubesolo-prod-cluster-net");
    assert_eq!(volume_name("prod-cluster"), "kubesolo-prod-cluster-data");
}

// -----------------------------------------------------------------------------
// 3. Multi-Cluster Coexistence Tests
// -----------------------------------------------------------------------------

#[test]
fn test_multi_cluster_coexistence() {
    let mut engine = MockContainerEngine::default();

    // Cluster A
    let params_a = ContainerInstallParams {
        instance_name: "alpha".to_string(),
        image: "portainer/kubesolo:latest".to_string(),
        mtu: Some(1450),
        d2k: false,
        container_ports: Some("8080:80".to_string()),
        extra_env: vec![],
        apiserver_host_port: Some(6443),
        d2k_host_port: None,
    };
    let res_a = install_container(&mut engine, &params_a).unwrap();

    // Cluster B
    let params_b = ContainerInstallParams {
        instance_name: "beta".to_string(),
        image: "portainer/kubesolo:latest".to_string(),
        mtu: Some(1500),
        d2k: true,
        container_ports: Some("8081:80".to_string()),
        extra_env: vec![],
        apiserver_host_port: Some(7443),
        d2k_host_port: Some(2377),
    };
    let res_b = install_container(&mut engine, &params_b).unwrap();

    // Verify distinct container names
    assert_eq!(res_a.container_name, "kubesolo-alpha");
    assert_eq!(res_b.container_name, "kubesolo-beta");

    // Verify distinct networks and MTUs
    assert_eq!(res_a.network_name, "kubesolo-alpha-net");
    assert_eq!(res_b.network_name, "kubesolo-beta-net");
    assert_eq!(
        engine
            .networks
            .get("kubesolo-alpha-net")
            .unwrap()
            .options
            .get("com.docker.network.driver.mtu")
            .unwrap(),
        "1450"
    );
    assert_eq!(
        engine
            .networks
            .get("kubesolo-beta-net")
            .unwrap()
            .options
            .get("com.docker.network.driver.mtu")
            .unwrap(),
        "1500"
    );

    // Verify distinct volumes
    assert_eq!(res_a.volume_name, "kubesolo-alpha-data");
    assert_eq!(res_b.volume_name, "kubesolo-beta-data");
    assert!(engine.volumes.contains_key("kubesolo-alpha-data"));
    assert!(engine.volumes.contains_key("kubesolo-beta-data"));

    // Verify distinct API and D2K ports
    assert_eq!(res_a.apiserver_port, 6443);
    assert_eq!(res_b.apiserver_port, 7443);
    assert_eq!(res_a.d2k_port, None);
    assert_eq!(res_b.d2k_port, Some(2377));

    // Both containers are running simultaneously
    assert_eq!(engine.containers.len(), 2);
    assert!(engine.containers.get("kubesolo-alpha").unwrap().1);
    assert!(engine.containers.get("kubesolo-beta").unwrap().1);
}

// -----------------------------------------------------------------------------
// 4. Rollback and Ownership Cleanup Tests
// -----------------------------------------------------------------------------

#[test]
fn test_rollback_cleans_only_newly_owned_resources() {
    let mut engine = MockContainerEngine::default();

    // Pre-create an existing volume (e.g. from previous run or shared)
    let pre_existing_vol = CreateVolumeRequest::new("kubesolo-data");
    engine.create_volume(&pre_existing_vol).unwrap();
    assert_eq!(engine.created_volumes, vec!["kubesolo-data"]);

    // Injected failure at container start
    engine.fail_start_container = true;

    let params = ContainerInstallParams {
        instance_name: "rubix".to_string(),
        image: "portainer/kubesolo:latest".to_string(),
        mtu: None,
        d2k: false,
        container_ports: None,
        extra_env: vec![],
        apiserver_host_port: None,
        d2k_host_port: None,
    };

    let result = install_container(&mut engine, &params);
    assert!(result.is_err());

    // Newly created network and container must be rolled back
    assert!(engine.removed_containers.contains(&"kubesolo".to_string()));
    assert!(
        engine
            .removed_networks
            .contains(&"kubesolo-net".to_string())
    );

    // BUT pre-existing volume must NOT be rolled back
    assert!(
        !engine
            .removed_volumes
            .contains(&"kubesolo-data".to_string())
    );
    assert!(engine.volumes.contains_key("kubesolo-data"));
}

#[test]
fn test_rollback_on_network_creation_failure() {
    let mut engine = MockContainerEngine {
        fail_create_network: true,
        ..Default::default()
    };

    let params = ContainerInstallParams {
        instance_name: "test".to_string(),
        image: "img:tag".to_string(),
        mtu: None,
        d2k: false,
        container_ports: None,
        extra_env: vec![],
        apiserver_host_port: None,
        d2k_host_port: None,
    };

    let result = install_container(&mut engine, &params);
    assert!(result.is_err());

    // Nothing was created, nothing to remove
    assert!(engine.removed_containers.is_empty());
    assert!(engine.removed_networks.is_empty());
    assert!(engine.removed_volumes.is_empty());
}

// -----------------------------------------------------------------------------
// 5. Engine Request Fixtures (Linux/macOS/WSL2)
// -----------------------------------------------------------------------------

#[test]
fn test_engine_request_fixtures_matching_api() {
    // Network create serialization
    let net_req = CreateNetworkRequest::new("kubesolo-net", Some(1450));
    let net_json = serialize_network_create_request(&net_req);
    assert!(net_json.contains("\"Name\":\"kubesolo-net\""));
    assert!(net_json.contains("\"Driver\":\"bridge\""));
    assert!(net_json.contains("\"com.docker.network.driver.mtu\":\"1450\""));

    // Volume create serialization
    let vol_req = CreateVolumeRequest::new("kubesolo-data");
    let vol_json = serialize_volume_create_request(&vol_req);
    assert_eq!(
        vol_json,
        "{\"Name\":\"kubesolo-data\",\"Driver\":\"local\"}"
    );

    // Container create serialization
    let mut engine = MockContainerEngine::default();
    let params = ContainerInstallParams {
        instance_name: "test".to_string(),
        image: "portainer/kubesolo:v1.1.8".to_string(),
        mtu: Some(1500),
        d2k: true,
        container_ports: Some("8080:80".to_string()),
        extra_env: vec![("CUSTOM_VAR".into(), "val".into())],
        apiserver_host_port: Some(6443),
        d2k_host_port: Some(2376),
    };
    install_container(&mut engine, &params).unwrap();

    let (cfg, _, _) = engine.containers.get("kubesolo-test").unwrap();
    let container_json = serialize_container_create_request(cfg);

    // Verify key fields required by KubeSolo container contract
    assert!(container_json.contains("\"Privileged\":true"));
    assert!(container_json.contains("\"CgroupnsMode\":\"host\""));
    assert!(container_json.contains("\"RestartPolicy\":{\"Name\":\"unless-stopped\"}"));
    assert!(container_json.contains("\"Binds\":[\"kubesolo-test-data:/var/lib/kubesolo\"]"));
    assert!(container_json.contains("\"NetworkMode\":\"kubesolo-test-net\""));
    assert!(
        container_json.contains("\"6443/tcp\":[{\"HostIp\":\"127.0.0.1\",\"HostPort\":\"6443\"}]")
    );
    assert!(
        container_json.contains("\"2376/tcp\":[{\"HostIp\":\"127.0.0.1\",\"HostPort\":\"2376\"}]")
    );
    assert!(container_json.contains("\"80/tcp\":[{\"HostIp\":\"\",\"HostPort\":\"8080\"}]"));
    assert!(container_json.contains("\"KUBESOLO_CONTAINER_MODE=true\""));
    assert!(container_json.contains("\"KUBESOLO_NAME=test\""));
    assert!(container_json.contains("\"CUSTOM_VAR=val\""));
}

// -----------------------------------------------------------------------------
// 6. CPU Manager Policy Rejection in Container Mode
// -----------------------------------------------------------------------------

#[test]
fn test_cpu_manager_policy_rejected_in_container_mode() {
    let mut engine = MockContainerEngine::default();

    let params = ContainerInstallParams {
        instance_name: "test".to_string(),
        image: "img:tag".to_string(),
        mtu: None,
        d2k: false,
        container_ports: None,
        extra_env: vec![("KUBESOLO_CPU_MANAGER_POLICY".into(), "static".into())],
        apiserver_host_port: None,
        d2k_host_port: None,
    };

    let result = install_container(&mut engine, &params);
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("cpu-manager-policy \"static\" is not supported in container mode"));
}

#[test]
fn test_api_and_d2k_bind_loopback_and_publish_discovered_ports() {
    let cfg =
        rubixctl::container::build_port_configuration(None, true, Some(2400), None).expect("ports");
    for bindings in cfg.port_bindings.values() {
        assert!(bindings.iter().all(|b| b.host_ip == "127.0.0.1"));
    }
    assert_eq!(cfg.port_bindings["6443/tcp"][0].host_port, "");
    assert_eq!(cfg.port_bindings["2376/tcp"][0].host_port, "2400");

    let mut ports = HashMap::new();
    ports.insert("6443/tcp".to_string(), 32768);
    ports.insert("2376/tcp".to_string(), 2400);
    let inspect = ContainerInspect {
        id: "x".into(),
        name: "x".into(),
        running: true,
        exit_code: 0,
        allocated_ports: ports,
    };
    let eps = rubixctl::container::published_endpoints(&inspect).expect("endpoints");
    assert_eq!(eps.apiserver.to_string(), "127.0.0.1:32768");
    assert_eq!(
        eps.d2k.map(|a| a.to_string()).as_deref(),
        Some("127.0.0.1:2400")
    );
}

#[test]
fn test_no_endpoint_without_published_api_port() {
    let inspect = ContainerInspect {
        id: "x".into(),
        name: "x".into(),
        running: true,
        exit_code: 0,
        allocated_ports: HashMap::new(),
    };
    assert!(rubixctl::container::published_endpoints(&inspect).is_none());
}
