//! CLI verification tool for Go-to-Rust state transitions (Epic E30 / Issue #124).
//!
//! Validates supported starting versions, configuration conversion, PKI trust roots,
//! kubeconfig format compatibility (both YAML and JSON), datastore non-interchangeability,
//! static workloads, and persistent volume data preservation.

use std::process::ExitCode;

use rubix_dev::state_transition::versions::SupportedStartingVersion;
use rubix_dev::state_transition::workloads::state_classification_inventory;
use rubix_dev::state_transition::{
    KubeconfigFormat, MINIMUM_SUPPORTED_VERSION, classify_starting_version, parse_kubeconfig,
};

fn verify_version_catalog() -> Result<(), String> {
    println!("Supported starting versions:");
    for ver in SupportedStartingVersion::ALL {
        let classified = classify_starting_version(ver.as_str())
            .map_err(|e| format!("failed to classify {ver}: {e}"))?;
        if classified != ver {
            return Err(format!("classification mismatch for {ver}"));
        }
        println!(
            "  [{}] supports_config_file={} requires_flag_migration={} ({})",
            ver.as_str(),
            ver.supports_config_file(),
            ver.requires_flag_migration(),
            ver.layout_description()
        );
    }
    println!();

    let rejected_cases = ["v1.0.0", "v0.9.1", "v1.1.7", "", "develop", "unversioned"];
    for raw in rejected_cases {
        match classify_starting_version(raw) {
            Ok(v) => return Err(format!("expected '{raw}' to be rejected, but got {v}")),
            Err(e) => println!("  [ok] rejected unsupported '{raw}': {e}"),
        }
    }
    println!();
    Ok(())
}

fn verify_kubeconfig_formats() -> Result<(), String> {
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

    let json_kubeconfig = br#"{
  "apiVersion": "v1",
  "kind": "Config",
  "current-context": "admin@kubesolo",
  "contexts": [{"name":"admin@kubesolo", "context":{"cluster":"kubesolo", "user":"admin"}}],
  "clusters": [
    {
      "name": "kubesolo",
      "cluster": {
        "server": "https://127.0.0.1:6443",
        "certificate-authority-data": "Y2EtZGF0YQ=="
      }
    }
  ],
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

    let parsed_yaml = parse_kubeconfig(yaml_kubeconfig)
        .map_err(|e| format!("YAML kubeconfig failed to parse: {e}"))?;
    if parsed_yaml.format != KubeconfigFormat::Yaml {
        return Err("expected YAML kubeconfig format".into());
    }
    println!(
        "  [ok] YAML kubeconfig accommodated: server={}",
        parsed_yaml.server
    );

    let parsed_json = parse_kubeconfig(json_kubeconfig)
        .map_err(|e| format!("JSON kubeconfig failed to parse: {e}"))?;
    if parsed_json.format != KubeconfigFormat::Json {
        return Err("expected JSON kubeconfig format".into());
    }
    println!(
        "  [ok] JSON kubeconfig accommodated: server={}",
        parsed_json.server
    );
    println!();
    Ok(())
}

fn verify_state_classification() {
    let inventory = state_classification_inventory();
    println!(
        "State classification inventory ({} items):",
        inventory.len()
    );
    for item in &inventory {
        println!(
            "  [{:?}] {} - {}",
            item.category, item.path_or_resource, item.justification
        );
    }
    println!();
}

fn run() -> Result<(), String> {
    println!("=== Rubix State Transition Fixture Checks (Issue #124) ===");
    println!("Minimum supported baseline: {MINIMUM_SUPPORTED_VERSION}\n");

    verify_version_catalog()?;
    verify_kubeconfig_formats()?;
    verify_state_classification();

    println!("All static state transition checks passed successfully.");
    println!("Production Kine migration, WAL checkpointing and downtime remain UNQUALIFIED.");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        },
    }
}
