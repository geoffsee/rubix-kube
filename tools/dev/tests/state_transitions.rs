//! Integration tests for Go-to-Rust state transition validation (Issue #124 / Epic E30).

use std::fs;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose};
use rubix_config::HostContext;
use rubix_dev::state_transition::config::{assert_flags_match_yaml, validate_config_transition};
use rubix_dev::state_transition::datastore::{KineRecord, validate_datastore_transition};
use rubix_dev::state_transition::pki::{KubeconfigFormat, assert_pki_transition};
use rubix_dev::state_transition::report::StateTransitionReport;
use rubix_dev::state_transition::storage::assert_pv_storage_preserved;
use rubix_dev::state_transition::versions::{
    MINIMUM_SUPPORTED_VERSION, SupportedStartingVersion, UnsupportedVersionError,
    classify_starting_version,
};
use rubix_dev::state_transition::workloads::{
    StatePortabilityCategory, WorkloadIdentity, assert_static_manifests_preserved,
    assert_workload_identities_preserved, state_classification_inventory,
};
use tempfile::tempdir;

#[test]
fn test_supported_starting_versions_selection_and_rejection() {
    // 1. All supported versions must classify correctly
    for ver in SupportedStartingVersion::ALL {
        let classified = classify_starting_version(ver.as_str()).expect("should classify");
        assert_eq!(classified, ver);
    }

    // 2. Pre-config file versions require flag migration
    assert!(SupportedStartingVersion::V1_1_8.requires_flag_migration());
    assert!(SupportedStartingVersion::V1_2_0.requires_flag_migration());
    assert!(!SupportedStartingVersion::V1_1_8.supports_config_file());
    assert!(!SupportedStartingVersion::V1_2_0.supports_config_file());

    // 3. Post-config file versions (v1.3.0+) support YAML natively
    assert!(SupportedStartingVersion::V1_3_0.supports_config_file());
    assert!(SupportedStartingVersion::V1_3_1.supports_config_file());
    assert!(SupportedStartingVersion::V1_3_2.supports_config_file());
    assert!(SupportedStartingVersion::V1_3_3.supports_config_file());
    assert!(!SupportedStartingVersion::V1_3_0.requires_flag_migration());

    // 4. Versions below v1.1.8 must be rejected
    match classify_starting_version("v1.1.7") {
        Err(UnsupportedVersionError::BelowMinimumSupported { version, minimum }) => {
            assert_eq!(version, "v1.1.7");
            assert_eq!(minimum, MINIMUM_SUPPORTED_VERSION);
        },
        other => panic!("expected BelowMinimumSupported, got {other:?}"),
    }

    match classify_starting_version("v1.0.0") {
        Err(UnsupportedVersionError::BelowMinimumSupported { .. }) => {},
        other => panic!("expected BelowMinimumSupported, got {other:?}"),
    }

    // 5. Unparseable or empty versions rejected
    assert!(matches!(
        classify_starting_version(""),
        Err(UnsupportedVersionError::Empty)
    ));
    assert!(matches!(
        classify_starting_version("develop"),
        Err(UnsupportedVersionError::Malformed(_))
    ));
}

#[test]
fn test_configuration_transition_from_legacy_flags() {
    let tmp = tempdir().unwrap();
    let service_file = tmp.path().join("kubesolo.service");
    let dest_config = tmp.path().join("config.yaml");

    // Realistic systemd unit with legacy CLI flags from Go KubeSolo v1.1.8
    let service_content = "[Unit]
Description=KubeSolo
After=network.target

[Service]
ExecStart=/usr/local/bin/kubesolo \\
  --node-ip=192.168.1.55 \\
  --cluster-cidr=10.42.0.0/16 \\
  --service-cidr=10.43.0.0/16 \\
  --dns-ip=10.43.0.10 \\
  --disable-ipv6 \\
  --portainer-edge-id=edge-node-alpha \\
  --portainer-edge-key=secret-agent-key-999
Restart=always

[Install]
WantedBy=multi-user.target
";
    fs::write(&service_file, service_content).unwrap();

    let host = HostContext {
        cpu_count: 4,
        architecture: "x86_64".into(),
        detected_container_mode: false,
    };

    let assertion = validate_config_transition(
        SupportedStartingVersion::V1_1_8,
        &service_file,
        &dest_config,
        Some(&host),
    )
    .unwrap();

    assert!(assertion.service_migrated);
    assert!(assertion.permissions_valid_0600);
    assert_eq!(&assertion.node_ip, "192.168.1.55");
    assert!(assertion.disable_ipv6);
    assert_eq!(&assertion.edge_id, "edge-node-alpha");

    // Verify backup file was created
    let backup_path = assertion.backup_path.expect("backup created");
    assert!(backup_path.exists());
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), service_content);

    // Verify modified service unit points to --config
    let updated_service = fs::read_to_string(&service_file).unwrap();
    assert!(updated_service.contains(&format!("--config={}", dest_config.display())));

    // Verify YAML content matches original flags
    let yaml_content = fs::read_to_string(&dest_config).unwrap();
    assert_flags_match_yaml(service_content, &yaml_content).unwrap();
}

#[test]
fn test_configuration_preservation_for_existing_config() {
    let tmp = tempdir().unwrap();
    let service_file = tmp.path().join("kubesolo.service");
    let dest_config = tmp.path().join("config.yaml");

    // Existing config file in v1.3.0
    let config_content = "apiVersion: kubesolo.io/v1alpha1
network:
  nodeIP: 10.10.10.10
";
    fs::write(&dest_config, config_content).unwrap();

    let service_content = format!(
        "[Service]
ExecStart=/usr/local/bin/kubesolo --config={}
",
        dest_config.display()
    );
    fs::write(&service_file, &service_content).unwrap();

    let assertion = validate_config_transition(
        SupportedStartingVersion::V1_3_0,
        &service_file,
        &dest_config,
        None,
    )
    .unwrap();

    // Already migrated, should not mutate or overwrite
    assert!(!assertion.service_migrated);
    assert_eq!(&assertion.node_ip, "10.10.10.10");
    assert_eq!(fs::read_to_string(&dest_config).unwrap(), config_content);
}

fn generate_test_pki() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    // Generate CA
    let mut ca_params = CertificateParams::default();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "kubesolo-ca");
    ca_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    ca_params.key_usages.push(KeyUsagePurpose::CrlSign);
    let ca_key = KeyPair::generate().unwrap();
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();
    let ca_key_pem = ca_key.serialize_pem();

    // Generate Client (admin) signed by CA
    let mut client_params = CertificateParams::default();
    client_params.is_ca = IsCa::NoCa;
    client_params
        .distinguished_name
        .push(DnType::CommonName, "admin");
    client_params
        .key_usages
        .push(KeyUsagePurpose::DigitalSignature);
    let client_key = KeyPair::generate().unwrap();
    let issuer = rcgen::Issuer::from_ca_cert_pem(&ca_cert.pem(), ca_key).unwrap();
    let client_cert = client_params.signed_by(&client_key, &issuer).unwrap();

    (
        ca_cert.pem().into_bytes(),
        ca_key_pem.into_bytes(),
        client_cert.pem().into_bytes(),
        client_key.serialize_pem().into_bytes(),
    )
}

#[test]
fn test_pki_trust_roots_and_kubeconfig_both_formats() {
    let tmp = tempdir().unwrap();
    let pki_before = tmp.path().join("pki-before");
    let pki_after = tmp.path().join("pki-after");
    fs::create_dir_all(&pki_before).unwrap();
    fs::create_dir_all(&pki_after).unwrap();

    let (ca_pem, ca_key_pem, client_pem, client_key_pem) = generate_test_pki();

    // Setup before and after PKI trees
    fs::write(pki_before.join("ca.crt"), &ca_pem).unwrap();
    fs::write(pki_before.join("ca.key"), &ca_key_pem).unwrap();
    fs::write(pki_before.join("service-account.key"), b"mock-sa-key").unwrap();

    fs::write(pki_after.join("ca.crt"), &ca_pem).unwrap();
    fs::write(pki_after.join("ca.key"), &ca_key_pem).unwrap();
    fs::write(pki_after.join("service-account.key"), b"mock-sa-key").unwrap();

    let ca_b64 = BASE64_STANDARD.encode(&ca_pem);
    let cert_b64 = BASE64_STANDARD.encode(&client_pem);
    let key_b64 = BASE64_STANDARD.encode(&client_key_pem);

    // Case 1: YAML kubeconfig
    let yaml_kubeconfig = tmp.path().join("admin-yaml.kubeconfig");
    let yaml_content = format!(
        "apiVersion: v1
clusters:
- cluster:
    certificate-authority-data: {ca_b64}
    server: https://127.0.0.1:6443
  name: kubesolo
contexts:
- context:
    cluster: kubesolo
    user: admin
  name: admin@kubesolo
current-context: admin@kubesolo
kind: Config
preferences: {{}}
users:
- name: admin
  user:
    client-certificate-data: {cert_b64}
    client-key-data: {key_b64}
"
    );
    fs::write(&yaml_kubeconfig, &yaml_content).unwrap();

    let assertion_yaml = assert_pki_transition(&pki_before, &pki_after, &yaml_kubeconfig).unwrap();
    assert!(assertion_yaml.ca_preserved);
    assert!(assertion_yaml.sa_key_preserved);
    assert!(assertion_yaml.client_cert_verified);
    assert_eq!(assertion_yaml.kubeconfig_format, KubeconfigFormat::Yaml);

    // Case 2: JSON kubeconfig
    let json_kubeconfig = tmp.path().join("admin-json.kubeconfig");
    let json_content = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Config",
        "current-context": "admin@kubesolo",
        "clusters": [{
            "name": "kubesolo",
            "cluster": {
                "server": "https://127.0.0.1:6443",
                "certificate-authority-data": ca_b64
            }
        }],
        "users": [{
            "name": "admin",
            "user": {
                "client-certificate-data": cert_b64,
                "client-key-data": key_b64
            }
        }]
    });
    fs::write(
        &json_kubeconfig,
        serde_json::to_string_pretty(&json_content).unwrap(),
    )
    .unwrap();

    let assertion_json = assert_pki_transition(&pki_before, &pki_after, &json_kubeconfig).unwrap();
    assert!(assertion_json.ca_preserved);
    assert!(assertion_json.sa_key_preserved);
    assert!(assertion_json.client_cert_verified);
    assert_eq!(assertion_json.kubeconfig_format, KubeconfigFormat::Json);
}

#[tokio::test]
async fn test_datastore_non_interchangeability_and_explicit_export_import() {
    let tmp = tempdir().unwrap();

    // Mock realistic Kine records from Go KubeSolo
    let records = vec![
        KineRecord {
            id: 1,
            name: "/registry/namespaces/default".into(),
            created: 1,
            deleted: 0,
            create_revision: 1,
            prev_revision: 0,
            total_keys: 1,
            lease: 0,
            value: b"namespace-default-data".to_vec(),
            old_value: Vec::new(),
        },
        KineRecord {
            id: 2,
            name: "/registry/services/specs/default/kubernetes".into(),
            created: 1,
            deleted: 0,
            create_revision: 2,
            prev_revision: 0,
            total_keys: 2,
            lease: 0,
            value: b"service-kubernetes-spec".to_vec(),
            old_value: Vec::new(),
        },
        KineRecord {
            id: 3,
            name: "/registry/pods/default/nginx".into(),
            created: 1,
            deleted: 0,
            create_revision: 3,
            prev_revision: 0,
            total_keys: 3,
            lease: 0,
            value: b"pod-nginx-v1".to_vec(),
            old_value: Vec::new(),
        },
        KineRecord {
            id: 4,
            name: "/registry/pods/default/temporary".into(),
            created: 1,
            deleted: 0,
            create_revision: 4,
            prev_revision: 0,
            total_keys: 4,
            lease: 0,
            value: b"temporary-pod".to_vec(),
            old_value: Vec::new(),
        },
        // Delete record 4 (tombstone)
        KineRecord {
            id: 5,
            name: "/registry/pods/default/temporary".into(),
            created: 0,
            deleted: 1,
            create_revision: 4,
            prev_revision: 4,
            total_keys: 3,
            lease: 0,
            value: Vec::new(),
            old_value: b"temporary-pod".to_vec(),
        },
    ];

    let assertion = validate_datastore_transition(&records, tmp.path())
        .await
        .unwrap();

    // 1. Proof of non-interchangeability: raw SQLite format was rejected
    assert!(assertion.raw_sqlite_rejected_by_rubix_datastore);

    // 2. Export / import preserved all active keys
    assert_eq!(assertion.total_records_before, 5);
    assert_eq!(assertion.active_keys_before, 3);
    assert_eq!(assertion.restored_keys, 3);
    assert!(assertion.keys_identical);

    // 3. Monotonic revision preserved
    assert!(assertion.revisions_monotonic);
    assert_eq!(assertion.restored_revision, 5);
}

#[test]
fn test_static_workloads_and_kubernetes_resource_identities() {
    let before = vec![
        WorkloadIdentity {
            api_version: "v1".into(),
            kind: "Namespace".into(),
            namespace: String::new(),
            name: "default".into(),
            uid: "ns-uid-100".into(),
            resource_version: "10".into(),
            spec_hash: "hash-ns-spec".into(),
        },
        WorkloadIdentity {
            api_version: "v1".into(),
            kind: "Pod".into(),
            namespace: "default".into(),
            name: "web-server".into(),
            uid: "pod-uid-200".into(),
            resource_version: "25".into(),
            spec_hash: "hash-pod-spec-nginx".into(),
        },
        WorkloadIdentity {
            api_version: "v1".into(),
            kind: "PersistentVolumeClaim".into(),
            namespace: "default".into(),
            name: "data-claim".into(),
            uid: "pvc-uid-300".into(),
            resource_version: "30".into(),
            spec_hash: "hash-pvc-spec-10Gi".into(),
        },
    ];

    let after_identical = before.clone();
    assert_workload_identities_preserved(&before, &after_identical).unwrap();

    // Drift in UID must be rejected
    let mut after_drift = before.clone();
    after_drift[1].uid = "mutated-uid-999".into();
    assert!(assert_workload_identities_preserved(&before, &after_drift).is_err());

    // Static manifests verification
    let tmp = tempdir().unwrap();
    let manifests_before = tmp.path().join("manifests-before");
    let manifests_after = tmp.path().join("manifests-after");
    fs::create_dir_all(&manifests_before).unwrap();
    fs::create_dir_all(&manifests_after).unwrap();

    fs::write(
        manifests_before.join("kube-vip.yaml"),
        b"apiVersion: v1\nkind: Pod\nmetadata:\n  name: kube-vip\n",
    )
    .unwrap();
    fs::write(
        manifests_after.join("kube-vip.yaml"),
        b"apiVersion: v1\nkind: Pod\nmetadata:\n  name: kube-vip\n",
    )
    .unwrap();

    assert_static_manifests_preserved(&manifests_before, &manifests_after).unwrap();

    // State classification inventory must classify all 10 entries
    let inventory = state_classification_inventory();
    assert_eq!(inventory.len(), 10);
    let persistent_count = inventory
        .iter()
        .filter(|i| i.category == StatePortabilityCategory::PortablePersistent)
        .count();
    let ephemeral_count = inventory
        .iter()
        .filter(|i| i.category == StatePortabilityCategory::NonportableEphemeral)
        .count();
    assert_eq!(persistent_count, 5);
    assert_eq!(ephemeral_count, 5);
}

#[test]
fn test_pv_storage_preservation_and_checksum_verification() {
    let tmp = tempdir().unwrap();
    let storage_before = tmp.path().join("local-path-storage-before");
    let storage_after = tmp.path().join("local-path-storage-after");

    // Create realistic PV directories
    let pvc_1 = storage_before.join("pvc-aaa_default_database-volume");
    let pvc_2 = storage_before.join("pvc-bbb_default_media-volume");
    fs::create_dir_all(&pvc_1).unwrap();
    fs::create_dir_all(pvc_2.join("subfolder")).unwrap();

    fs::write(pvc_1.join("db.sqlite"), b"binary-database-content-12345").unwrap();
    fs::write(
        pvc_2.join("subfolder").join("file.bin"),
        b"media-data-bytes",
    )
    .unwrap();

    // Duplicate to after directory
    let pvc_1_after = storage_after.join("pvc-aaa_default_database-volume");
    let pvc_2_after = storage_after.join("pvc-bbb_default_media-volume");
    fs::create_dir_all(&pvc_1_after).unwrap();
    fs::create_dir_all(pvc_2_after.join("subfolder")).unwrap();

    fs::write(
        pvc_1_after.join("db.sqlite"),
        b"binary-database-content-12345",
    )
    .unwrap();
    fs::write(
        pvc_2_after.join("subfolder").join("file.bin"),
        b"media-data-bytes",
    )
    .unwrap();

    let assertion = assert_pv_storage_preserved(&storage_before, &storage_after).unwrap();
    assert_eq!(assertion.total_volumes, 2);
    assert_eq!(assertion.total_files, 2);
    assert!(assertion.all_checksums_match);
    assert!(assertion.all_permissions_match);

    // If file content is modified, checksum assertion must fail
    fs::write(pvc_1_after.join("db.sqlite"), b"corrupted-content").unwrap();
    let assertion_corrupted = assert_pv_storage_preserved(&storage_before, &storage_after).unwrap();
    assert!(!assertion_corrupted.all_checksums_match);
}

#[test]
fn test_state_transition_report_formatting() {
    let report = StateTransitionReport {
        starting_version: SupportedStartingVersion::V1_2_0,
        target_distribution: "Rubix v0.1.0 (Candidate)".into(),
        config_assertion: None,
        pki_assertion: None,
        datastore_assertion: None,
        pv_storage_assertion: None,
        nonportable_state_classified: 10,
        downtime_documented_minutes: 5,
        required_backups_verified: true,
        overall_success: true,
    };

    let md = report.to_markdown();
    assert!(md.contains("# State Transition Report: v1.2.0 -> Rubix v0.1.0 (Candidate)"));
    assert!(md.contains("**Estimated Downtime**: ~5 minutes"));
    assert!(md.contains("**PASS**"));
}
