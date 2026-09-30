use std::collections::BTreeMap;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::KubeletError;

/// Default upstream Kubelet ports and endpoints.
pub const DEFAULT_KUBELET_PORT: u16 = 10250;
pub const DEFAULT_KUBELET_READ_ONLY_PORT: u16 = 0;
pub const DEFAULT_HEALTHZ_PORT: u16 = 10248;
pub const DEFAULT_HEALTHZ_BIND_ADDRESS: &str = "127.0.0.1";
pub const DEFAULT_CLUSTER_DOMAIN: &str = "cluster.local";
pub const DEFAULT_CLUSTER_DNS: &str = "10.43.0.10";
pub const DEFAULT_KUBELET_ROOT_DIR: &str = "/var/lib/kubelet";
pub const DEFAULT_CONTAINERD_RUNTIME_ENDPOINT: &str = "unix:///run/containerd/containerd.sock";

/// Comprehensive Kubelet configuration options.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KubeletConfigOptions {
    pub node_name: String,
    pub node_ip: String,
    pub disable_ipv6: bool,
    pub root_dir: PathBuf,
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub kubeconfig: PathBuf,
    pub ca_file: PathBuf,
    pub cert_file: PathBuf,
    pub key_file: PathBuf,
    pub runtime_endpoint: String,
    pub image_endpoint: Option<String>,
    pub cgroup_driver: String,
    pub runtime_cgroup_driver: Option<String>,
    pub resolv_conf: Option<PathBuf>,
    pub cluster_domain: String,
    pub cluster_dns: Vec<String>,
    pub container_mode: bool,
    pub cpu_manager_policy: String,
    pub cpu_manager_policy_options: BTreeMap<String, String>,
    pub reserved_cpus: String,
    pub system_reserved: BTreeMap<String, String>,
    pub kube_reserved: BTreeMap<String, String>,
    pub healthz_port: u16,
    pub healthz_bind_address: String,
    pub port: u16,
    pub read_only_port: u16,
}

impl Default for KubeletConfigOptions {
    fn default() -> Self {
        let root = PathBuf::from(DEFAULT_KUBELET_ROOT_DIR);
        Self {
            node_name: "rubix-node".to_string(),
            node_ip: String::new(),
            disable_ipv6: false,
            config_dir: root.clone(),
            config_file: root.join("kubelet.yaml"),
            root_dir: root.clone(),
            kubeconfig: PathBuf::from("/etc/kubernetes/kubelet.kubeconfig"),
            ca_file: PathBuf::from("/etc/kubernetes/pki/ca.crt"),
            cert_file: PathBuf::from("/etc/kubernetes/pki/kubelet.crt"),
            key_file: PathBuf::from("/etc/kubernetes/pki/kubelet.key"),
            runtime_endpoint: DEFAULT_CONTAINERD_RUNTIME_ENDPOINT.to_string(),
            image_endpoint: None,
            cgroup_driver: "cgroupfs".to_string(),
            runtime_cgroup_driver: None,
            resolv_conf: None,
            cluster_domain: DEFAULT_CLUSTER_DOMAIN.to_string(),
            cluster_dns: vec![DEFAULT_CLUSTER_DNS.to_string()],
            container_mode: false,
            cpu_manager_policy: "none".to_string(),
            cpu_manager_policy_options: BTreeMap::new(),
            reserved_cpus: String::new(),
            system_reserved: BTreeMap::new(),
            kube_reserved: BTreeMap::new(),
            healthz_port: DEFAULT_HEALTHZ_PORT,
            healthz_bind_address: DEFAULT_HEALTHZ_BIND_ADDRESS.to_string(),
            port: DEFAULT_KUBELET_PORT,
            read_only_port: DEFAULT_KUBELET_READ_ONLY_PORT,
        }
    }
}

impl KubeletConfigOptions {
    #[must_use]
    pub fn default_for_pki(
        pki_dir: &Path,
        node_name: impl Into<String>,
        node_ip: impl Into<String>,
        root_dir: &Path,
    ) -> Self {
        let name = node_name.into();
        Self {
            node_name: name,
            node_ip: node_ip.into(),
            disable_ipv6: false,
            root_dir: root_dir.to_path_buf(),
            config_dir: root_dir.to_path_buf(),
            config_file: root_dir.join("kubelet.yaml"),
            kubeconfig: pki_dir.join("kubelet.kubeconfig"),
            ca_file: pki_dir.join("ca.crt"),
            cert_file: pki_dir.join("kubelet.crt"),
            key_file: pki_dir.join("kubelet.key"),
            runtime_endpoint: DEFAULT_CONTAINERD_RUNTIME_ENDPOINT.to_string(),
            image_endpoint: None,
            cgroup_driver: "cgroupfs".to_string(),
            runtime_cgroup_driver: None,
            resolv_conf: None,
            cluster_domain: DEFAULT_CLUSTER_DOMAIN.to_string(),
            cluster_dns: vec![DEFAULT_CLUSTER_DNS.to_string()],
            container_mode: false,
            cpu_manager_policy: "none".to_string(),
            cpu_manager_policy_options: BTreeMap::new(),
            reserved_cpus: String::new(),
            system_reserved: BTreeMap::new(),
            kube_reserved: BTreeMap::new(),
            healthz_port: DEFAULT_HEALTHZ_PORT,
            healthz_bind_address: DEFAULT_HEALTHZ_BIND_ADDRESS.to_string(),
            port: DEFAULT_KUBELET_PORT,
            read_only_port: DEFAULT_KUBELET_READ_ONLY_PORT,
        }
    }

    /// Resolves the cgroup driver to use.
    ///
    /// Precedence matches upstream `KubeSolo`:
    /// 1. Runtime-reported cgroup driver (from CRI runtime), if present.
    /// 2. If in container mode, defaults to "cgroupfs".
    /// 3. Explicit cgroup driver if configured (and not "auto").
    /// 4. Detected from host init system: "systemd" if `/run/systemd/private` exists, else "cgroupfs".
    #[must_use]
    pub fn resolve_cgroup_driver(&self) -> String {
        if let Some(ref driver) = self.runtime_cgroup_driver
            && !driver.is_empty()
        {
            return driver.clone();
        }
        if self.container_mode {
            return "cgroupfs".to_string();
        }
        if !self.cgroup_driver.is_empty() && self.cgroup_driver != "auto" {
            return self.cgroup_driver.clone();
        }
        if Path::new("/run/systemd/private").exists() {
            "systemd".to_string()
        } else {
            "cgroupfs".to_string()
        }
    }

    /// Constructs Kubelet configuration JSON document matching upstream `kubelet.config.k8s.io/v1beta1`.
    #[must_use]
    pub fn generate_kubelet_config(&self) -> Value {
        let mut map = serde_json::Map::new();

        map.insert(
            "apiVersion".to_string(),
            json!("kubelet.config.k8s.io/v1beta1"),
        );
        map.insert(
            "authentication".to_string(),
            json!({
                "anonymous": {
                    "enabled": false
                },
                "webhook": {
                    "cacheTTL": "5m0s",
                    "enabled": true
                },
                "x509": {
                    "clientCAFile": self.ca_file.to_string_lossy()
                }
            }),
        );
        map.insert(
            "authorization".to_string(),
            json!({
                "mode": "Webhook",
                "webhook": {
                    "cacheAuthorizedTTL": "10m0s",
                    "cacheUnauthorizedTTL": "1m0s"
                }
            }),
        );
        map.insert(
            "cgroupDriver".to_string(),
            json!(self.resolve_cgroup_driver()),
        );
        map.insert("clusterDNS".to_string(), json!(self.cluster_dns));
        map.insert("clusterDomain".to_string(), json!(self.cluster_domain));
        map.insert(
            "containerRuntimeEndpoint".to_string(),
            json!(self.runtime_endpoint),
        );
        map.insert("failSwapOn".to_string(), json!(false));
        map.insert("kind".to_string(), json!("KubeletConfiguration"));
        map.insert("readOnlyPort".to_string(), json!(self.read_only_port));

        if self.cpu_manager_policy == "static" {
            map.insert("cpuManagerPolicy".to_string(), json!("static"));
            if !self.reserved_cpus.is_empty() {
                map.insert("reservedSystemCPUs".to_string(), json!(self.reserved_cpus));
            }
            if !self.cpu_manager_policy_options.is_empty() {
                map.insert(
                    "cpuManagerPolicyOptions".to_string(),
                    json!(self.cpu_manager_policy_options),
                );
            }
        }

        if !self.system_reserved.is_empty() {
            map.insert("systemReserved".to_string(), json!(self.system_reserved));
        }

        if self.container_mode {
            map.insert("cgroupsPerQOS".to_string(), json!(false));
            map.insert("enforceNodeAllocatable".to_string(), json!([]));
            map.insert(
                "evictionHard".to_string(),
                json!({
                    "imagefs.available": "0%",
                    "memory.available": "50Mi",
                    "nodefs.available": "0%",
                    "nodefs.inodesFree": "0%"
                }),
            );
            map.insert("imageGCHighThresholdPercent".to_string(), json!(100));
            if !map.contains_key("systemReserved") {
                map.insert("systemReserved".to_string(), json!({}));
            }
            map.insert("kubeReserved".to_string(), json!(self.kube_reserved));
            map.insert("resolvConf".to_string(), json!("/dev/null"));
            map.insert("rotateCertificates".to_string(), json!(true));
        } else {
            let resolv = self.resolv_conf.as_ref().map_or_else(
                || "/etc/resolv.conf".to_string(),
                |p| p.to_string_lossy().to_string(),
            );
            map.insert("resolvConf".to_string(), json!(resolv));
            map.insert("rotateCertificates".to_string(), json!(true));
            if !self.kube_reserved.is_empty() {
                map.insert("kubeReserved".to_string(), json!(self.kube_reserved));
            }
        }

        map.insert(
            "tlsCertFile".to_string(),
            json!(self.cert_file.to_string_lossy()),
        );
        map.insert(
            "tlsPrivateKeyFile".to_string(),
            json!(self.key_file.to_string_lossy()),
        );

        Value::Object(map)
    }

    /// Renders Kubelet configuration into canonical YAML matching the official golden fixture.
    #[must_use]
    pub fn render_yaml(&self) -> String {
        let val = self.generate_kubelet_config();
        render_canonical_yaml(&val)
    }

    /// Writes the generated YAML document to `self.config_file`.
    pub fn write_kubelet_config_file(&self) -> Result<(), KubeletError> {
        let parent =
            self.config_file
                .parent()
                .ok_or_else(|| KubeletError::InvalidConfiguration {
                    field: "config_file".to_string(),
                    reason: "config_file must have a parent directory".to_string(),
                })?;

        if parent.exists() && !parent.is_dir() {
            return Err(KubeletError::InvalidConfiguration {
                field: "config_file".to_string(),
                reason: format!(
                    "parent path {} is a file, not a directory",
                    parent.display()
                ),
            });
        }

        fs::create_dir_all(parent)?;
        let yaml = self.render_yaml();
        fs::write(&self.config_file, yaml)?;
        Ok(())
    }

    /// Generates CLI arguments for Kubelet process execution.
    ///
    /// Evaluates IP validity and loopback:
    /// - Loopback addresses (`127.0.0.1`, `::1`) or invalid IP strings are omitted.
    /// - Valid non-loopback addresses are passed via `--node-ip`.
    /// - If `disable_ipv6` is set, IPv6 addresses are omitted.
    #[must_use]
    pub fn configure_kubelet_args(&self) -> Vec<String> {
        let mut args = vec![
            "--config".to_string(),
            self.config_file.to_string_lossy().to_string(),
            "--hostname-override".to_string(),
            self.node_name.clone(),
            "--root-dir".to_string(),
            self.root_dir.to_string_lossy().to_string(),
            "--kubeconfig".to_string(),
            self.kubeconfig.to_string_lossy().to_string(),
        ];

        if let Ok(ip) = self.node_ip.parse::<IpAddr>() {
            let is_loopback = ip.is_loopback();
            let is_ipv6_disabled = self.disable_ipv6 && ip.is_ipv6();
            if !is_loopback && !is_ipv6_disabled {
                args.push("--node-ip".to_string());
                args.push(self.node_ip.clone());
            }
        }

        args
    }

    /// Validates that upstream resource defaults are preserved without reviving removed KS-68 edge overrides.
    pub fn validate_upstream_resource_defaults(&self) -> Result<(), KubeletError> {
        if self.container_mode {
            if self.cpu_manager_policy == "static" {
                return Err(KubeletError::InvalidConfiguration {
                    field: "cpu_manager_policy".to_string(),
                    reason: "static CPU manager policy is unsupported in container mode"
                        .to_string(),
                });
            }
        } else {
            // In host mode, edge memory overrides must not be revived
            if self.read_only_port != 0 {
                return Err(KubeletError::InvalidConfiguration {
                    field: "read_only_port".to_string(),
                    reason: "read_only_port must default to 0 for security".to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Renders a JSON Value into deterministic, alphabetically sorted YAML.
pub fn render_canonical_yaml(value: &Value) -> String {
    let mut out = String::new();
    render_value(value, 0, &mut out);
    out
}

fn render_scalar(val: &Value, out: &mut String) {
    match val {
        Value::String(s) => {
            if s.is_empty()
                || s == "true"
                || s == "false"
                || s == "null"
                || s == "yes"
                || s == "no"
                || s.starts_with('*')
                || s.starts_with('&')
                || s.starts_with('!')
                || s.starts_with('@')
                || s.starts_with('`')
                || s.starts_with('-')
                || s.starts_with('?')
                || s.starts_with('{')
                || s.starts_with('[')
                || s.contains(": ")
                || s.contains('#')
            {
                out.push_str(&serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\"")));
            } else {
                out.push_str(s);
            }
        },
        _ => {
            out.push_str(&val.to_string());
        },
    }
}

fn render_array_elements(arr: &[Value], indent: usize, out: &mut String) {
    out.push('\n');
    for elem in arr {
        out.push_str(&" ".repeat(indent));
        out.push_str("- ");
        render_scalar(elem, out);
        out.push('\n');
    }
}

fn render_value(value: &Value, indent: usize, out: &mut String) {
    if let Value::Object(map) = value {
        // Sort keys alphabetically
        let mut keys: Vec<&String> = map.keys().collect();
        keys.sort();

        for key in keys {
            let v = &map[key];
            out.push_str(&" ".repeat(indent));
            out.push_str(key);
            out.push(':');

            match v {
                Value::Object(sub_map) if sub_map.is_empty() => {
                    out.push_str(" {}\n");
                },
                Value::Object(_) => {
                    out.push('\n');
                    render_value(v, indent + 2, out);
                },
                Value::Array(arr) if arr.is_empty() => {
                    out.push_str(" []\n");
                },
                Value::Array(arr) => {
                    render_array_elements(arr, indent + 2, out);
                },
                Value::String(_) | Value::Bool(_) | Value::Number(_) => {
                    out.push(' ');
                    render_scalar(v, out);
                    out.push('\n');
                },
                Value::Null => {
                    out.push_str(" null\n");
                },
            }
        }
    }
}
