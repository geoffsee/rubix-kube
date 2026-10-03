//! Engine request fixtures for Linux, macOS, and WSL2 container-mode workflows.
//!
//! Provides JSON representations and structured request comparisons matching Docker Engine API v1.41+.

use crate::container::{ContainerConfig, CreateNetworkRequest, CreateVolumeRequest};

/// Serializes a `CreateNetworkRequest` to Docker Engine API JSON format.
///
/// Matches `POST /networks/create`:
/// ```json
/// {
///   "Name": "kubesolo-net",
///   "Driver": "bridge",
///   "Options": {
///     "com.docker.network.driver.mtu": "1500"
///   }
/// }
/// ```
pub fn serialize_network_create_request(req: &CreateNetworkRequest) -> String {
    let mut options_json = Vec::new();
    for (k, v) in &req.options {
        options_json.push(format!("\"{k}\":\"{v}\""));
    }
    format!(
        "{{\"Name\":\"{}\",\"Driver\":\"{}\",\"Options\":{{{}}}}}",
        req.name,
        req.driver,
        options_json.join(",")
    )
}

/// Serializes a `CreateVolumeRequest` to Docker Engine API JSON format.
///
/// Matches `POST /volumes/create`:
/// ```json
/// {
///   "Name": "kubesolo-data",
///   "Driver": "local"
/// }
/// ```
pub fn serialize_volume_create_request(req: &CreateVolumeRequest) -> String {
    format!(
        "{{\"Name\":\"{}\",\"Driver\":\"{}\"}}",
        req.name, req.driver
    )
}

/// Serializes a `ContainerConfig` to Docker Engine API JSON format.
///
/// Matches `POST /containers/create?name=<name>`:
/// Includes `HostConfig` with `Privileged`, `CgroupnsMode`, `Binds`, `PortBindings`, `NetworkMode`, `RestartPolicy`.
pub fn serialize_container_create_request(config: &ContainerConfig) -> String {
    // 1. ExposedPorts: {"6443/tcp": {}}
    let exposed_str = config
        .exposed_ports
        .iter()
        .map(|p| format!("\"{p}\":{{}}"))
        .collect::<Vec<_>>()
        .join(",");

    // 2. Env: ["VAR=val", ...]
    let env_str = config
        .env
        .iter()
        .map(|e| format!("\"{e}\""))
        .collect::<Vec<_>>()
        .join(",");

    // 3. Binds: ["kubesolo-data:/var/lib/kubesolo"]
    let binds_str = config
        .binds
        .iter()
        .map(|b| format!("\"{b}\""))
        .collect::<Vec<_>>()
        .join(",");

    // 4. PortBindings: {"6443/tcp": [{"HostIp": "127.0.0.1", "HostPort": ""}]}
    let port_bindings_str = config
        .port_bindings
        .iter()
        .map(|(port_key, bindings)| {
            let bindings_str = bindings
                .iter()
                .map(|b| {
                    format!(
                        "{{\"HostIp\":\"{}\",\"HostPort\":\"{}\"}}",
                        b.host_ip, b.host_port
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("\"{port_key}\":[{bindings_str}]")
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"Image\":\"{}\",\"Hostname\":\"{}\",\"Env\":[{}],\"ExposedPorts\":{{{}}},\"HostConfig\":{{\"Binds\":[{}],\"NetworkMode\":\"{}\",\"PortBindings\":{{{}}},\"Privileged\":{},\"CgroupnsMode\":\"{}\",\"RestartPolicy\":{{\"Name\":\"{}\"}}}}}}",
        config.image,
        config.hostname,
        env_str,
        exposed_str,
        binds_str,
        config.network_mode,
        port_bindings_str,
        config.privileged,
        config.cgroupns_mode,
        config.restart_policy
    )
}
