use std::fs;
use std::path::{Path, PathBuf};

use rubix_dev::conformance::{
    CERTIFICATION_DISCLAIMER, CONFORMANCE_FOCUS_REGEX, CONFORMANCE_SKIP_REGEX,
    ConformanceInventory, ConformanceResult, ConformanceSummary, Kubeconfig, QualificationRunner,
    generate_suite_selection_markdown,
};
use rubix_dev::release_qualification::receipt::{
    load_candidate_inventory, validate_candidate_receipt,
};
use rubix_dev::repository_root;

fn executed_summary() -> ConformanceSummary {
    let results: Vec<_> = ConformanceInventory::selected_tests()
        .into_iter()
        .map(|t| ConformanceResult {
            test_id: t.id,
            name: t.name,
            passed: true,
            duration_ms: 1,
            error: None,
        })
        .collect();
    ConformanceSummary {
        total_selected: results.len(),
        passed: results.len(),
        failed: 0,
        excluded_count: ConformanceInventory::explicit_exclusions().len(),
        focus_filter: CONFORMANCE_FOCUS_REGEX.into(),
        skip_filter: CONFORMANCE_SKIP_REGEX.into(),
        certification_disclaimer: CERTIFICATION_DISCLAIMER.into(),
        exclusions: ConformanceInventory::explicit_exclusions(),
        results,
    }
}

#[test]
fn summary_requires_exact_unique_successful_per_case_results() {
    // Report-consistency fixture only, not execution evidence.
    let summary = executed_summary();
    summary.verify_qualification().unwrap();
    let mut missing = summary.clone();
    missing.results.clear();
    assert!(missing.verify_qualification().is_err());
    let mut duplicate = summary.clone();
    duplicate.results[1] = duplicate.results[0].clone();
    assert!(duplicate.verify_qualification().is_err());
    let mut failed = summary.clone();
    failed.results[0].passed = false;
    failed.results[0].error = Some("workload failed".into());
    assert!(failed.verify_qualification().is_err());
    let mut contradictory = summary.clone();
    contradictory.results[0].error = Some("not executed".into());
    assert!(contradictory.verify_qualification().is_err());
    let mut wrong = summary.clone();
    wrong.results[0].name = "unrelated test".into();
    assert!(wrong.verify_qualification().is_err());
    let mut counts = summary.clone();
    counts.passed -= 1;
    assert!(counts.verify_qualification().is_err());
    let mut exclusions = summary;
    exclusions.exclusions[0].rationale.clear();
    assert!(exclusions.verify_qualification().is_err());
}

#[test]
fn yaml_roundtrip_preserves_escaped_scalars_empty_users_and_preferences() {
    for context in [
        "admin: prod",
        "true",
        "# comment",
        "line\nbreak",
        "quote\"slash\\",
    ] {
        let input = serde_json::json!({ "apiVersion":"v1", "kind":"Config", "current-context":context, "preferences":{"colors":true,"extension":"quoted: value"}, "clusters":[{"name":"cluster", "cluster":{"server":"https://127.0.0.1:6443", "certificate-authority":"/a: b/#ca"}}], "contexts":[{"name":context,"context":{"cluster":"cluster","user":"user"}}], "users":[{"name":"user","user":{"token":"true # token\nnext"}},{"name":"empty","user":{}}] });
        let parsed = Kubeconfig::parse(&input.to_string()).unwrap();
        assert_eq!(Kubeconfig::parse(&parsed.to_yaml()).unwrap(), parsed);
    }
}

#[test]
fn cli_cannot_claim_node_qualification() {
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_rubix-conformance"))
        .arg("run")
        .output()
        .unwrap();
    assert!(!run.status.success());
    assert!(String::from_utf8_lossy(&run.stderr).contains("not implemented"));
}

fn repo_root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("repo root")
}

#[test]
fn suite_selection_markdown_documentation() {
    let md = generate_suite_selection_markdown();

    assert!(md.contains("# Criterion 6 — Conformance & Recovery Qualification Suite Selection"));
    assert!(md.contains(CERTIFICATION_DISCLAIMER));
    assert!(md.contains("Smoke Checks (3 checks)"));
    assert!(md.contains("Manifest Domains (6 tiers)"));
    assert!(md.contains("Selected Single-Node Conformance Tests (24 tests)"));
    assert!(md.contains("Explicit Exclusions & Technical Rationale (7 categories)"));
    assert!(md.contains("k8s-conf-pod-01"));
    assert!(md.contains("k8s-conf-sec-01"));
    assert!(md.contains("Pod Egress Masquerade / SNAT Routing"));
    assert!(md.contains("LoadBalancer UPDATE path [KS-75]"));
}

#[tokio::test]
async fn candidate_qualification_execution_and_receipt_validation() {
    let root = repo_root();
    let cell_inventory = load_candidate_inventory(&root).expect("candidate inventory");
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let output_path = temp_dir.path();

    let runner = QualificationRunner::default();
    let receipt = runner
        .run_candidate_qualification(output_path, Some(&root))
        .await
        .expect("run candidate qualification");

    assert!(output_path.join("receipt.json").is_file());
    assert!(
        output_path
            .join("criterion-06-conformance-and-soak.json")
            .is_file()
    );
    assert!(output_path.join("suite-selection.md").is_file());

    let logs_dir = output_path.join("logs");
    assert!(logs_dir.is_dir());

    let entries: Vec<_> = fs::read_dir(&logs_dir)
        .expect("read logs dir")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(
        entries.len(),
        70,
        "expected 70 log files (35 stdout, 35 stderr)"
    );

    let stdout_count = entries
        .iter()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".stdout"))
        .count();
    let stderr_count = entries
        .iter()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".stderr"))
        .count();
    assert_eq!(stdout_count, 35);
    assert_eq!(stderr_count, 35);

    validate_candidate_receipt(&receipt, &cell_inventory, 6).expect("validate candidate receipt");

    assert_eq!(receipt.schema_version, 1);
    assert_eq!(receipt.criterion, 6);
    assert_eq!(receipt.commands.len(), 35);
    assert_eq!(receipt.assertions.len(), 33);
    assert_eq!(receipt.skips.len(), 7);
    assert_eq!(receipt.cleanup.status, "complete");
    assert!(receipt.cleanup.remaining_containers.is_empty());
    assert!(receipt.cleanup.remaining_images.is_empty());

    for cmd in &receipt.commands {
        assert_eq!(
            cmd.exit_code, 0,
            "command '{:?}' exited non-zero",
            cmd.command
        );
    }
    for assertion in &receipt.assertions {
        assert!(assertion.passed, "assertion '{}' failed", assertion.name);
    }
}

#[test]
fn receipt_tamper_detection() {
    let root = repo_root();
    let cell_inventory = load_candidate_inventory(&root).expect("candidate inventory");
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let output_path = temp_dir.path();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let receipt = rt
        .block_on(
            QualificationRunner::default().run_candidate_qualification(output_path, Some(&root)),
        )
        .expect("run candidate qualification");

    // Tampering 1: modified command exit code breaks integrity hash
    let mut tampered = receipt.clone();
    tampered.commands[0].exit_code = 1;
    let err = validate_candidate_receipt(&tampered, &cell_inventory, 6).unwrap_err();
    assert!(err.to_string().contains("integrity hash mismatch"));

    // Tampering 2: invalid criterion
    let mut tampered = receipt.clone();
    tampered.criterion = 5;
    let err = validate_candidate_receipt(&tampered, &cell_inventory, 6).unwrap_err();
    assert!(err.to_string().contains("criterion mismatch"));

    // Tampering 3: modified assertion passed breaks integrity hash
    let mut tampered = receipt.clone();
    tampered.assertions[0].passed = false;
    let err = validate_candidate_receipt(&tampered, &cell_inventory, 6).unwrap_err();
    assert!(err.to_string().contains("integrity hash mismatch"));
}

#[test]
fn cli_candidate_qualification_run_and_verify() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let output_path = temp_dir.path();

    let run_cmd = std::process::Command::new(env!("CARGO_BIN_EXE_rubix-conformance"))
        .arg("run")
        .arg("--output")
        .arg(output_path)
        .arg("--root")
        .arg(&root)
        .output()
        .expect("run rubix-conformance run");

    assert!(
        run_cmd.status.success(),
        "run failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&run_cmd.stdout),
        String::from_utf8_lossy(&run_cmd.stderr)
    );

    let receipt_path = output_path.join("receipt.json");
    assert!(receipt_path.is_file());

    let verify_cmd = std::process::Command::new(env!("CARGO_BIN_EXE_rubix-conformance"))
        .arg("verify-receipt")
        .arg(&receipt_path)
        .arg("--root")
        .arg(&root)
        .output()
        .expect("run rubix-conformance verify-receipt");

    assert!(
        verify_cmd.status.success(),
        "verify-receipt failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&verify_cmd.stdout),
        String::from_utf8_lossy(&verify_cmd.stderr)
    );
    assert!(String::from_utf8_lossy(&verify_cmd.stdout).contains("Receipt verification passed"));

    let receipt_content = fs::read_to_string(&receipt_path).expect("read receipt");
    let tampered_content =
        receipt_content.replace("\"status\": \"complete\"", "\"status\": \"incomplete\"");
    fs::write(&receipt_path, tampered_content).expect("write tampered receipt");

    let verify_tampered = std::process::Command::new(env!("CARGO_BIN_EXE_rubix-conformance"))
        .arg("verify-receipt")
        .arg(&receipt_path)
        .arg("--root")
        .arg(&root)
        .output()
        .expect("run rubix-conformance verify-receipt tampered");

    assert!(!verify_tampered.status.success());
    assert!(
        String::from_utf8_lossy(&verify_tampered.stderr)
            .contains("error: receipt verification failed")
    );
}
