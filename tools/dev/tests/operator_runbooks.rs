//! Runbook examples must use implemented commands and the distribution schema.
//! These parser/file checks establish no live installation or migration qualification.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rubix_dev::repository_root;
use rubix_dev::state_transition::pki::{KubeconfigFormat, parse_kubeconfig};

const RUNBOOKS: [&str; 7] = [
    "README.md",
    "fresh-installs.md",
    "air-gap-deployment.md",
    "external-container-runtime.md",
    "networking-and-storage.md",
    "metrics-and-cpu-management.md",
    "migration-and-recovery.md",
];

fn operator_docs_dir() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("repository root")
        .join("docs/operator")
}

fn fenced_blocks(content: &str, language: &str) -> Vec<String> {
    let mut blocks = vec![];
    let mut active = false;
    let mut current = String::new();
    for line in content.lines() {
        if line == format!("```{language}") {
            assert!(!active, "nested code fence");
            active = true;
        } else if line == "```" && active {
            blocks.push(std::mem::take(&mut current));
            active = false;
        } else if active {
            current.push_str(line);
            current.push('\n');
        }
    }
    assert!(!active, "unterminated code fence");
    blocks
}

#[test]
fn runbook_links_resolve_to_actual_contracts_and_implementation() {
    let directory = operator_docs_dir();
    let links = regex::Regex::new(r"\]\(([^)]+)\)").unwrap();
    for name in RUNBOOKS {
        let path = directory.join(name);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.len() > 500, "empty runbook {name}");
        for matched in links.captures_iter(&text) {
            let target = &matched[1];
            if !target.starts_with("http") && !target.starts_with('#') {
                assert!(
                    directory.join(target).is_file(),
                    "broken {name} link: {target}"
                );
            }
        }
    }
}

#[test]
fn documented_management_examples_use_actual_parser() {
    let mut count = 0;
    for name in RUNBOOKS {
        let text = fs::read_to_string(operator_docs_dir().join(name)).unwrap();
        for block in fenced_blocks(&text, "sh") {
            for line in block
                .lines()
                .filter_map(|line| line.strip_prefix("rubixctl "))
            {
                // The recovery option is separately checked against the integrated parent.
                if line.contains("--recover") {
                    continue;
                }
                let args = line
                    .split_whitespace()
                    .map(String::from)
                    .collect::<Vec<_>>();
                assert!(
                    rubixctl::parse_command(&args, &BTreeMap::new()).is_ok(),
                    "unsupported documented command in {name}: {line}"
                );
                count += 1;
            }
        }
    }
    assert!(count >= 20, "no useful management examples inspected");
}

#[test]
fn documented_recovery_option_uses_actual_parser() {
    for name in ["README.md", "migration-and-recovery.md"] {
        let text = fs::read_to_string(operator_docs_dir().join(name)).unwrap();
        let commands = fenced_blocks(&text, "sh")
            .into_iter()
            .flat_map(|block| block.lines().map(String::from).collect::<Vec<_>>())
            .filter(|line| line.starts_with("rubixctl ") && line.contains("--recover"))
            .collect::<Vec<_>>();
        assert_eq!(commands.len(), 1);
        let args = commands[0]
            .split_whitespace()
            .skip(1)
            .map(String::from)
            .collect::<Vec<_>>();
        assert!(
            matches!(
                rubixctl::parse_command(&args, &BTreeMap::new()),
                Ok(rubixctl::Command::Upgrade(options)) if options.recover
            ),
            "recovery requires the explicit reviewed recovery dispatch"
        );
    }
}

#[test]
fn documented_configuration_examples_have_no_ignored_fields() {
    let mut count = 0;
    for name in RUNBOOKS {
        let text = fs::read_to_string(operator_docs_dir().join(name)).unwrap();
        for yaml in fenced_blocks(&text, "yaml") {
            let decoded = rubix_config::decode(&yaml).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                decoded.warnings.is_empty(),
                "ignored/misleading {name} settings: {:?}",
                decoded.warnings
            );
            count += 1;
        }
    }
    assert!(count >= 5);
}

#[test]
fn airgap_cells_match_exact_release_matrix() {
    use rubix_platform::{Architecture, Libc};
    let text = fs::read_to_string(operator_docs_dir().join("air-gap-deployment.md")).unwrap();
    let rows = text
        .lines()
        .filter(|line| {
            line.strip_prefix("| Cell ")
                .and_then(|value| value.as_bytes().first())
                .is_some_and(u8::is_ascii_digit)
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), rubix_assets::Matrix::NODE_VARIANTS.len());
    for (row, variant) in rows.iter().zip(rubix_assets::Matrix::NODE_VARIANTS) {
        let architecture = match variant.architecture {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
            Architecture::ArmV7 => "arm (ARMv7 hard-float)",
            Architecture::Riscv64 => "riscv64",
        };
        let libc = match variant.libc {
            Libc::Glibc => "glibc",
            Libc::Musl => "musl",
        };
        let delivery = match variant.variant {
            rubix_assets::Variant::Online => "online",
            rubix_assets::Variant::Offline => "offline",
        };
        assert_eq!(
            *row,
            format!(
                "| Cell {:02} | {architecture} | {libc} | {delivery} |",
                variant.cell
            )
        );
    }
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
