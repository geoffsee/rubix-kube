#![allow(clippy::pedantic)]

use std::fs;
use tempfile::TempDir;

use rubix_dev::conformance::{
    CERTIFICATION_DISCLAIMER, ConformanceInventory, ExclusionCategory, Kubeconfig,
    QualificationReport, QualificationRunner,
};

#[tokio::test]
async fn synthetic_fixture_evidence_cannot_qualify_a_node() {
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let runner = QualificationRunner::new();
    let report = runner
        .run_fixture()
        .await
        .expect("Synthetic fixture suite should pass");

    // 1. Verify smoke results
    assert_eq!(report.smoke_results.len(), 3);
    for smoke in &report.smoke_results {
        assert_eq!(
            smoke.passed,
            smoke.check != rubix_dev::conformance::SmokeCheck::PodEgress
        );
    }

    // 2. Verify all 6 manifest domains
    assert_eq!(report.manifest_domain_results.len(), 6);
    for domain in &report.manifest_domain_results {
        assert!(domain.passed, "Manifest domain {} should pass", domain.name);
        assert!(
            domain.assertions_count > 0,
            "Domain {} should have executed assertions",
            domain.name
        );
    }

    let timestamp: u64 = report
        .timestamp
        .strip_prefix("unix:")
        .unwrap()
        .parse()
        .unwrap();
    let ended = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!((started..=ended).contains(&timestamp));

    // Check specific domains
    let d1 = &report.manifest_domain_results[0];
    assert_eq!(d1.name, "Tier 1 — Workloads & Networking");

    let d2 = &report.manifest_domain_results[1];
    assert_eq!(d2.name, "Tier 2 — Storage Persistence");
    assert!(
        d2.details
            .iter()
            .any(|d| d.contains("no reader workload executed"))
    );

    let d3 = &report.manifest_domain_results[2];
    assert_eq!(d3.name, "Tier 3 — Config & Identity");
    assert!(
        d3.details
            .iter()
            .any(|d| d.contains("no container consumed configuration"))
    );

    let d4 = &report.manifest_domain_results[3];
    assert_eq!(d4.name, "Tier 4 — Controllers");

    let d5 = &report.manifest_domain_results[4];
    assert_eq!(d5.name, "Tier 5 — DNS & LoadBalancer");

    let d6 = &report.manifest_domain_results[5];
    assert_eq!(d6.name, "Tier 6 — LoadBalancer UPDATE path [KS-75]");
    assert_eq!(d6.assertions_count, 10);
    assert!(d6.details.iter().all(|detail| !detail.contains("dry-run")));

    // 3. Verify selected conformance results
    let conf = &report.conformance_summary;
    assert_eq!(conf.total_selected, 24);
    assert_eq!(conf.passed, 0);
    assert!(conf.results.is_empty());
    assert_eq!(conf.failed, 0);
    assert_eq!(conf.excluded_count, 7);

    // Verify exclusions have technical rationale
    for ex in &conf.exclusions {
        assert!(!ex.pattern.is_empty());
        assert!(!ex.rationale.is_empty());
    }

    // Verify non-certification disclaimer
    assert!(
        report
            .certification_disclaimer
            .contains(CERTIFICATION_DISCLAIMER)
    );

    // 4. Verify no hidden skips rule
    report
        .verify_fixture()
        .expect("Exact fixture coverage must pass");
    assert!(report.verify_qualification().is_err());
    assert!(runner.run_qualification().await.is_err());
    let mut duplicate = report.clone();
    duplicate.smoke_results[1] = duplicate.smoke_results[0].clone();
    assert!(duplicate.verify_fixture().is_err());
    let mut duplicate = report.clone();
    duplicate.manifest_domain_results[1] = duplicate.manifest_domain_results[0].clone();
    assert!(duplicate.verify_fixture().is_err());
    let mut incorrect = report.clone();
    incorrect.total_assertions_checked += 1;
    assert!(incorrect.verify_fixture().is_err());
    let mut invented = report.clone();
    invented.conformance_summary.passed = 24;
    assert!(invented.verify_fixture().is_err());

    // 5. Verify Markdown report output
    let md = report.to_markdown();
    assert!(md.contains("# Rubix Synthetic In-Process Fixture Report"));
    assert!(md.contains("C13/E28 remains unqualified"));
    assert!(md.contains("## 1. Baseline Smoke Verification"));
    assert!(md.contains("## 2. Six Baseline Manifest Domains"));
    assert!(md.contains("## 3. Selected Single-Node Conformance Summary"));
    assert!(md.contains(CERTIFICATION_DISCLAIMER));
    assert!(md.contains("YAML and JSON"));
    assert!(md.contains("no CoreDNS server or pod query executed"));
    assert!(!md.contains("Reader pod verified"));
    assert!(!md.contains("Consumer pod verified"));

    // 6. Verify JSON serialization round-trip
    let json_str = serde_json::to_string_pretty(&report).expect("Serialize to JSON");
    let deser: QualificationReport =
        serde_json::from_str(&json_str).expect("Deserialize from JSON");
    assert_eq!(deser.conformance_summary.total_selected, 24);
    assert_eq!(deser.conformance_summary.passed, 0);
}

#[test]
fn test_kubeconfig_dual_format_accommodation() {
    let temp_dir = TempDir::new().unwrap();

    // 1. Valid YAML kubeconfig
    let yaml_content = r#"
apiVersion: v1
kind: Config
current-context: admin@rubix
clusters:
- name: rubix
  cluster:
    server: https://127.0.0.1:6443
    certificate-authority-data: dGVzdC1jYQ==
users:
- name: admin
  user:
    client-certificate-data: dGVzdC1jZXJ0
    client-key-data: dGVzdC1rZXk=
contexts:
- name: admin@rubix
  context:
    cluster: rubix
    user: admin
"#;

    let yaml_path = temp_dir.path().join("admin.yaml");
    fs::write(&yaml_path, yaml_content).unwrap();

    let from_yaml = Kubeconfig::from_file(&yaml_path).expect("YAML kubeconfig should parse");
    assert_eq!(from_yaml.current_context, "admin@rubix");
    assert_eq!(from_yaml.clusters.len(), 1);
    assert_eq!(
        from_yaml.clusters[0].cluster.server,
        "https://127.0.0.1:6443"
    );
    assert_eq!(from_yaml.users[0].name, "admin");

    // Convert to JSON and verify JSON format parsing
    let json_string = from_yaml
        .to_json()
        .expect("Kubeconfig should convert to JSON");
    let json_path = temp_dir.path().join("admin.json");
    fs::write(&json_path, &json_string).unwrap();

    let from_json = Kubeconfig::from_file(&json_path).expect("JSON kubeconfig should parse");
    assert_eq!(from_json.current_context, "admin@rubix");
    assert_eq!(
        from_json.clusters[0].cluster.server,
        "https://127.0.0.1:6443"
    );

    // Convert JSON back to YAML and verify equivalence
    let yaml_string = from_json.to_yaml();
    let roundtrip_yaml =
        Kubeconfig::parse(&yaml_string).expect("Roundtrip YAML should parse identically");
    assert_eq!(roundtrip_yaml.current_context, from_yaml.current_context);
    assert_eq!(
        roundtrip_yaml.clusters[0].cluster.server,
        from_yaml.clusters[0].cluster.server
    );
    assert_eq!(
        roundtrip_yaml.users[0].user.client_certificate_data,
        from_yaml.users[0].user.client_certificate_data
    );
}

#[test]
fn test_conformance_inventory_and_exclusion_audit() {
    let inventory = ConformanceInventory::selected_tests();
    assert_eq!(inventory.len(), 24);

    // Verify all test IDs match k8s-conf-*
    for test in &inventory {
        assert!(test.id.starts_with("k8s-conf-"));
        assert!(test.single_node_safe);
        assert!(!test.name.is_empty());
        assert!(!test.focus_keyword.is_empty());
    }

    // Verify focus filter matches all selected tests
    for test in &inventory {
        assert!(
            ConformanceInventory::is_selected(&test.name),
            "Test {} should be selected by regex",
            test.id
        );
    }

    // Verify exclusions
    let exclusions = ConformanceInventory::explicit_exclusions();
    assert_eq!(exclusions.len(), 7);

    // Verify every exclusion category is represented
    let categories: Vec<ExclusionCategory> = exclusions.iter().map(|e| e.category).collect();
    assert!(categories.contains(&ExclusionCategory::MultiNode));
    assert!(categories.contains(&ExclusionCategory::Disruptive));
    assert!(categories.contains(&ExclusionCategory::SerialSlow));
    assert!(categories.contains(&ExclusionCategory::Flaky));

    // Verify hidden skips detection
    let invalid_test_name = "Test with hidden skip [Conformance]";
    // Without focus keyword it should not be selected
    assert!(!ConformanceInventory::is_selected(invalid_test_name));

    let valid_name = "[sig-api-machinery] ConfigMap should be created [Conformance]";
    assert!(ConformanceInventory::is_selected(valid_name));

    // But if test name contains [Disruptive], it should NOT be selected
    let disruptive_name = "[sig-api-machinery] ConfigMap [Disruptive] [Conformance]";
    assert!(!ConformanceInventory::is_selected(disruptive_name));
}
