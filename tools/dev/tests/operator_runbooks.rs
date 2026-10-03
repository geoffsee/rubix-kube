//! Integration tests for Rubix operator runbooks and handoff documentation (Issue #126 / Gate C16).
//!
//! Verifies that all required operator documentation files exist under `docs/operator/`,
//! cross-reference authoritative architecture contracts, satisfy structural checklists,
//! and accommodate dual-format (YAML and JSON) client kubeconfig configurations.

use std::fs;
use std::path::{Path, PathBuf};

use rubix_dev::repository_root;
use rubix_dev::state_transition::pki::{KubeconfigFormat, parse_kubeconfig};

fn operator_docs_dir() -> PathBuf {
    let root = repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("repository root");
    root.join("docs").join("operator")
}

#[test]
fn all_required_operator_runbooks_exist_and_are_non_empty() {
    let dir = operator_docs_dir();
    assert!(dir.is_dir(), "docs/operator directory must exist");

    let required_files = [
        "README.md",
        "fresh-installs.md",
        "air-gap-deployment.md",
        "external-container-runtime.md",
        "networking-and-storage.md",
        "metrics-and-cpu-management.md",
        "migration-and-recovery.md",
    ];

    for file in &required_files {
        let path = dir.join(file);
        assert!(path.is_file(), "required runbook missing: {file}");
        let content =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {file}: {e}"));
        assert!(
            content.len() > 500,
            "runbook {file} is too short ({} bytes)",
            content.len()
        );
    }
}

#[test]
fn fresh_installs_covers_universal_minimal_and_container_modes() {
    let content = fs::read_to_string(operator_docs_dir().join("fresh-installs.md")).unwrap();

    // Universal install & preflight
    assert!(content.contains("rubixctl check"));
    assert!(content.contains("rubixctl install"));
    assert!(content.contains("--install-prereqs"));

    // Init systems
    assert!(content.contains("systemd"));
    assert!(content.contains("OpenRC"));
    assert!(content.contains("SysVinit"));
    assert!(content.contains("Upstart"));
    assert!(content.contains("runit"));
    assert!(content.contains("s6"));

    // Minimal manual install & execution modes
    assert!(content.contains("kubesolo.io/v1alpha1"));
    assert!(content.contains("/etc/kubesolo/config.yaml"));
    assert!(content.contains("Foreground"));
    assert!(content.contains("Daemon"));

    // Container mode lifecycle
    assert!(content.contains("rubixctl container create"));
    assert!(content.contains("rubixctl container restart"));
    assert!(content.contains("rubixctl container stop"));
    assert!(content.contains("rubixctl container remove"));
    assert!(content.contains("127.0.0.1"));
}

#[test]
fn air_gap_deployment_covers_16_cells_and_bundle_verification() {
    let content = fs::read_to_string(operator_docs_dir().join("air-gap-deployment.md")).unwrap();

    // 16 cells matrix
    for cell_num in 1..=16 {
        assert!(
            content.contains(&format!("Cell {cell_num:02}")),
            "missing Cell {cell_num:02} in air gap documentation"
        );
    }

    // Required image assets
    assert!(content.contains("coredns/coredns"));
    assert!(content.contains("portainer/pause"));
    assert!(content.contains("rancher/local-path-provisioner"));
    assert!(content.contains("library/busybox"));
    assert!(content.contains("portainer/agent"));
    assert!(content.contains("portainer/d2k"));

    // Security & integrity enforcements
    assert!(content.contains("bundle.manifest"));
    assert!(content.contains("--pull=never"));
    assert!(content.contains("Zero Egress"));
}

#[test]
fn external_runtime_covers_socket_and_cgroup_negotiation() {
    let content =
        fs::read_to_string(operator_docs_dir().join("external-container-runtime.md")).unwrap();

    // Socket syntax & validation
    assert!(content.contains("unix://"));
    assert!(content.contains("/run/containerd/containerd.sock"));
    assert!(content.contains("/run/crio/crio.sock"));

    // Cgroup driver negotiation
    assert!(content.contains("systemd"));
    assert!(content.contains("cgroupfs"));
    assert!(content.contains("RuntimeConfig"));
    assert!(content.contains("cgroup v2"));

    // Ownership boundary
    assert!(content.contains("External Runtime Ownership Rule"));
}

#[test]
fn networking_and_storage_covers_cni_egress_and_localpath() {
    let content =
        fs::read_to_string(operator_docs_dir().join("networking-and-storage.md")).unwrap();

    // Network topology & CNI
    assert!(content.contains("10.42.0.0/16"));
    assert!(content.contains("10.43.0.0/16"));
    assert!(content.contains("10-bridge.conflist"));
    assert!(content.contains("bridge"));
    assert!(content.contains("host-local"));
    assert!(content.contains("portmap"));
    assert!(content.contains("loopback"));

    // Egress rules & zero blanket flush policy
    assert!(content.contains("kubesolo-masq"));
    assert!(content.contains("kubesolo: pod masquerade"));
    assert!(content.contains("nftables"));
    assert!(content.contains("iptables"));

    // LocalPath storage
    assert!(content.contains("rancher.io/local-path"));
    assert!(content.contains("WaitForFirstConsumer"));
    assert!(content.contains("/var/lib/kubesolo/local-path-storage"));
}

#[test]
fn metrics_and_cpu_covers_routes_negotiation_and_policies() {
    let content =
        fs::read_to_string(operator_docs_dir().join("metrics-and-cpu-management.md")).unwrap();

    // HTTP Routes
    assert!(content.contains("/metrics"));
    assert!(content.contains("/healthz"));
    assert!(content.contains("/livez"));
    assert!(content.contains("/readyz"));

    // Content negotiation
    assert!(content.contains("Prometheus"));
    assert!(content.contains("OpenMetrics"));
    assert!(content.contains("406 Not Acceptable"));

    // Metric series
    assert!(content.contains("kubesolo_build_info"));
    assert!(content.contains("kubesolo_kine_db_size_bytes"));
    assert!(content.contains("kubesolo_certificate_valid"));
    assert!(content.contains("kubesolo_component_up"));

    // CPU Manager
    assert!(content.contains("static"));
    assert!(content.contains("full-pcpus-only"));
    assert!(content.contains("distribute-cpus-across-numa"));
    assert!(content.contains("cpu_manager_state"));
}

#[test]
fn migration_and_recovery_covers_steps_downtime_and_limits() {
    let content =
        fs::read_to_string(operator_docs_dir().join("migration-and-recovery.md")).unwrap();

    // Versions
    assert!(content.contains("v1.1.8"));
    assert!(content.contains("v1.2.0"));
    assert!(content.contains("v1.3.0"));
    assert!(content.contains("v1.3.1"));
    assert!(content.contains("v1.3.2"));
    assert!(content.contains("v1.3.3"));

    // Datastore WAL & PKI
    assert!(content.contains("wal_checkpoint"));
    assert!(content.contains("state.db"));
    assert!(content.contains("pki"));

    // Recovery
    assert!(content.contains("rubixctl upgrade --recover"));
    assert!(content.contains(".upgrade-pending"));
    assert!(content.contains(".upgrade-committing"));

    // Measured SLA & downtime bounds
    assert!(content.contains("10 seconds"));
    assert!(content.contains("5 seconds"));
    assert!(content.contains("30 seconds"));
    assert!(content.contains("1.10x"));
}

#[test]
fn dual_format_kubeconfig_accommodation_verifies_yaml_and_json() {
    // Standard YAML format kubeconfig
    let yaml_kubeconfig = b"apiVersion: v1
clusters:
- cluster:
    certificate-authority-data: Y2EtZGF0YQ==
    server: https://127.0.0.1:6443
  name: kubesolo
contexts:
- context:
    cluster: kubesolo
    user: admin
  name: admin@kubesolo
current-context: admin@kubesolo
kind: Config
preferences: {}
users:
- name: admin
  user:
    client-certificate-data: Y2VydC1kYXRh
    client-key-data: a2V5LWRhdGE=
";

    // Strict JSON format kubeconfig
    let json_kubeconfig = br#"{
  "apiVersion": "v1",
  "kind": "Config",
  "clusters": [
    {
      "name": "kubesolo",
      "cluster": {
        "server": "https://127.0.0.1:6443",
        "certificate-authority-data": "Y2EtZGF0YQ=="
      }
    }
  ],
  "contexts": [
    {
      "name": "admin@kubesolo",
      "context": {
        "cluster": "kubesolo",
        "user": "admin"
      }
    }
  ],
  "current-context": "admin@kubesolo",
  "preferences": {},
  "users": [
    {
      "name": "admin",
      "user": {
        "client-certificate-data": "Y2VydC1kYXRh",
        "client-key-data": "a2V5LWRhdGE="
      }
    }
  ]
}"#;

    let parsed_yaml = parse_kubeconfig(yaml_kubeconfig).expect("YAML kubeconfig must parse");
    assert_eq!(parsed_yaml.format, KubeconfigFormat::Yaml);
    assert_eq!(parsed_yaml.server, "https://127.0.0.1:6443");
    assert_eq!(parsed_yaml.cluster_name, "kubesolo");
    assert_eq!(parsed_yaml.user_name, "admin");
    assert_eq!(parsed_yaml.current_context, "admin@kubesolo");
    assert_eq!(parsed_yaml.ca_cert_bytes, b"ca-data");
    assert_eq!(parsed_yaml.client_cert_bytes, b"cert-data");
    assert_eq!(parsed_yaml.client_key_bytes, b"key-data");

    let parsed_json = parse_kubeconfig(json_kubeconfig).expect("JSON kubeconfig must parse");
    assert_eq!(parsed_json.format, KubeconfigFormat::Json);
    assert_eq!(parsed_json.server, "https://127.0.0.1:6443");
    assert_eq!(parsed_json.cluster_name, "kubesolo");
    assert_eq!(parsed_json.user_name, "admin");
    assert_eq!(parsed_json.current_context, "admin@kubesolo");
    assert_eq!(parsed_json.ca_cert_bytes, b"ca-data");
    assert_eq!(parsed_json.client_cert_bytes, b"cert-data");
    assert_eq!(parsed_json.client_key_bytes, b"key-data");
}
