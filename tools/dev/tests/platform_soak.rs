use rubix_dev::platform_soak::{
    DimensionCategory, EnvironmentMapping, MatrixCompleteness, PlatformSoakReport,
    PlatformSoakRunner, RegressionSuite, RestartSummary, SupportStatus, SustainedSoakSummary,
};

#[test]
fn canonical_matrix_contains_all_promised_environments_without_omissions() {
    let records = EnvironmentMapping::canonical_matrix();

    // 1. Verify validation passes on canonical matrix
    MatrixCompleteness::validate(&records).expect("canonical matrix must be complete and valid");

    // 2. Verify all 16 node variant cells are uniquely present
    let node_cells: Vec<_> = records
        .iter()
        .filter(|r| r.dimension == DimensionCategory::NodeVariantCell)
        .collect();
    assert_eq!(node_cells.len(), 16);
    for cell in 1..=16 {
        let id = format!("node-cell-{cell:02}");
        assert!(node_cells.iter().any(|r| r.id == id), "missing cell {id}");
    }

    // 3. Verify all 4 management targets + Windows exclusion
    let mgmt_targets: Vec<_> = records
        .iter()
        .filter(|r| r.dimension == DimensionCategory::ManagementTarget)
        .collect();
    assert_eq!(mgmt_targets.len(), 5);
    let win = mgmt_targets
        .iter()
        .find(|r| r.id == "mgmt-windows")
        .unwrap();
    assert!(matches!(
        win.support_status,
        SupportStatus::Unsupported { .. }
    ));

    // 4. Verify 28 OCI image records (7 images x 4 platforms)
    let oci_images: Vec<_> = records
        .iter()
        .filter(|r| r.dimension == DimensionCategory::OciContainerImage)
        .collect();
    assert_eq!(oci_images.len(), 28);

    // D2K unsupported on armv7 and riscv64
    let d2k_armv7 = oci_images
        .iter()
        .find(|r| r.id == "image-d2k-armv7")
        .unwrap();
    assert!(matches!(
        d2k_armv7.support_status,
        SupportStatus::Unsupported { .. }
    ));
    let d2k_riscv64 = oci_images
        .iter()
        .find(|r| r.id == "image-d2k-riscv64")
        .unwrap();
    assert!(matches!(
        d2k_riscv64.support_status,
        SupportStatus::Unsupported { .. }
    ));

    // Portainer unsupported on riscv64
    let portainer_riscv = oci_images
        .iter()
        .find(|r| r.id == "image-portainer-agent-riscv64")
        .unwrap();
    assert!(matches!(
        portainer_riscv.support_status,
        SupportStatus::Unsupported { .. }
    ));

    // CoreDNS supported on all 4
    for arch in ["amd64", "arm64", "armv7", "riscv64"] {
        let coredns = oci_images
            .iter()
            .find(|r| r.id == format!("image-coredns-{arch}"))
            .unwrap();
        assert_eq!(coredns.support_status, SupportStatus::Supported);
    }

    // 5. Container mode static CPU manager is unsupported
    let static_cpu = records
        .iter()
        .find(|r| r.id == "container-static-cpu-manager")
        .unwrap();
    assert!(matches!(
        static_cpu.support_status,
        SupportStatus::Unsupported { .. }
    ));
}

#[test]
fn matrix_completeness_fails_closed_on_omission() {
    let mut records = EnvironmentMapping::canonical_matrix();
    // Omit cell 05
    records.retain(|r| r.id != "node-cell-05");

    let err = MatrixCompleteness::validate(&records).unwrap_err();
    assert!(err.contains("missing node variant cell 05"));
}

#[test]
fn matrix_completeness_fails_closed_on_unsupported_without_rationale() {
    let mut records = EnvironmentMapping::canonical_matrix();
    for r in &mut records {
        if r.id == "mgmt-windows" {
            r.support_status = SupportStatus::Unsupported {
                rationale: "   ".into(),
            };
        }
    }

    let err = MatrixCompleteness::validate(&records).unwrap_err();
    assert!(err.contains("unsupported environment 'mgmt-windows' lacks technical rationale"));
}

#[test]
fn matrix_completeness_fails_closed_on_illegal_supported_claim() {
    let mut records = EnvironmentMapping::canonical_matrix();
    for r in &mut records {
        if r.id == "image-d2k-riscv64" {
            r.support_status = SupportStatus::Supported;
        }
    }

    let err = MatrixCompleteness::validate(&records).unwrap_err();
    assert!(err.contains("image-d2k-riscv64 cannot be marked supported"));
}

#[test]
fn candidate_digest_verification_passes_when_matched() {
    let manifest = PlatformSoakRunner::synthetic_candidate_manifest("0.1.0");
    let observed: Vec<(&str, &str, u64)> = manifest
        .node_archives
        .iter()
        .map(|a| (a.filename.as_str(), a.sha256.as_str(), a.size_bytes))
        .collect();

    let summary =
        rubix_dev::platform_soak::CandidateVerificationSummary::verify(&manifest, &observed)
            .expect("candidate verification should pass");

    assert!(summary.all_matched);
    assert_eq!(summary.mismatched_artifacts, 0);
    assert!(summary.total_artifacts >= 20);
}

#[test]
fn candidate_digest_verification_fails_closed_on_mismatch() {
    let manifest = PlatformSoakRunner::synthetic_candidate_manifest("0.1.0");
    let bad_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let observed = vec![(
        manifest.node_archives[0].filename.as_str(),
        bad_hash,
        manifest.node_archives[0].size_bytes,
    )];

    let err = rubix_dev::platform_soak::CandidateVerificationSummary::verify(&manifest, &observed)
        .unwrap_err();
    assert!(err.contains("candidate digest mismatch"));
}

#[test]
fn sustained_soak_evaluation_evaluates_contractual_bounds() {
    // 1. Passing 24h soak: growth <= 1.10x, 0 OOMs, 0 crashes, 0 probe failures
    let pass_res = SustainedSoakSummary::evaluate_record(
        "amd64",
        86400,
        48,
        500 * 1024 * 1024,
        525 * 1024 * 1024, // 1.05x growth
        0,
        0,
        0,
    );
    assert!(pass_res.is_ok());
    let rec = pass_res.unwrap();
    assert!(rec.passed);
    assert!((rec.derived_growth_ratio - 1.05).abs() < 1e-6);

    // 2. Failing: growth > 1.10x
    let fail_growth = SustainedSoakSummary::evaluate_record(
        "amd64",
        86400,
        48,
        500 * 1024 * 1024,
        600 * 1024 * 1024, // 1.20x growth > 1.10x
        0,
        0,
        0,
    );
    assert!(fail_growth.is_err());
    assert!(fail_growth.unwrap_err().contains("exceeds bound"));

    // 3. Failing: OOM kill event
    let fail_oom = SustainedSoakSummary::evaluate_record(
        "amd64",
        86400,
        48,
        500 * 1024 * 1024,
        520 * 1024 * 1024,
        1, // 1 OOM
        0,
        0,
    );
    assert!(fail_oom.is_err());
    assert!(fail_oom.unwrap_err().contains("OOM kill events"));

    // 4. Failing: duration < 24h
    let fail_duration = SustainedSoakSummary::evaluate_record(
        "amd64",
        3600, // 1h instead of 24h
        2,
        500 * 1024 * 1024,
        510 * 1024 * 1024,
        0,
        0,
        0,
    );
    assert!(fail_duration.is_err());
    assert!(
        fail_duration
            .unwrap_err()
            .contains("less than required 24h")
    );
}

#[test]
fn restart_summary_evaluates_bounds_and_state_preservation() {
    let cases = RestartSummary::canonical_cases();
    assert_eq!(cases.len(), 5);
    for case in &cases {
        assert!(case.passed);
        assert!(case.state_preserved);
        assert!(case.observed_duration_ms <= case.bound_duration_ms);
    }

    // Failing: duration exceeded
    let fail_time = RestartSummary::evaluate_case(
        "RST-CLEAN",
        "Clean restart",
        "node readiness <= 10s",
        10_000,
        15_000, // 15s > 10s
        true,
        "detail",
    );
    assert!(fail_time.is_err());

    // Failing: state not preserved
    let fail_state = RestartSummary::evaluate_case(
        "RST-CLEAN",
        "Clean restart",
        "node readiness <= 10s",
        10_000,
        5_000,
        false, // state not preserved
        "detail",
    );
    assert!(fail_state.is_err());
}

#[test]
fn historical_regressions_inventory_is_complete_and_valid() {
    let regs = RegressionSuite::historical_regressions();
    assert_eq!(regs.len(), 10);
    RegressionSuite::validate(&regs).expect("all 10 historical regressions must pass validation");

    let epics = RegressionSuite::epic_regressions();
    assert_eq!(epics.len(), 30);
    for epic in &epics {
        assert!(epic.passed);
        assert!(!epic.qualification_gate.is_empty());
    }
}

#[test]
fn full_platform_soak_qualification_run_and_report_serialization() {
    let runner = PlatformSoakRunner::new();
    let version = "0.1.0";
    let report = runner
        .run_qualification(version, None)
        .expect("qualification run must succeed");

    assert_eq!(report.product, "rubix-kube");
    assert_eq!(report.version, version);
    assert!(report.overall_qualified);

    // Validate report directly
    report
        .validate(Some(version))
        .expect("report validation must succeed");

    // Test Markdown formatting
    let md = report.to_markdown();
    assert!(md.contains("# Platform Coverage and Sustained-Soak Qualification Report"));
    assert!(md.contains("## 1. Executive Summary"));
    assert!(md.contains("## 2. Platform & Environment Matrix"));
    assert!(md.contains("## 3. Candidate Digest Verification"));
    assert!(md.contains("## 4. Sustained 24-Hour Soak & Memory Stability"));
    assert!(md.contains("## 5. Restart Bounds & Recovery Lifecycle"));
    assert!(md.contains("## 6. Historical Regressions"));
    assert!(md.contains("## 7. Per-Epic Regression Gates"));

    // Test JSON round-trip
    let json_bytes = serde_json::to_vec_pretty(&report).unwrap();
    let deserialized: PlatformSoakReport = serde_json::from_slice(&json_bytes).unwrap();
    assert_eq!(deserialized, report);
    deserialized
        .validate(Some(version))
        .expect("deserialized report validation must succeed");
}

#[test]
fn report_validation_rejects_tampered_records_and_recomputes_all_bounds() {
    let runner = PlatformSoakRunner::new();
    let version = "0.1.0";
    let valid_report = runner
        .run_qualification(version, None)
        .expect("qualification run must succeed");

    // 1. Tamper: short soak duration with passed=true
    let mut tampered = valid_report.clone();
    tampered.soak_results[0].actual_duration_seconds = 1; // 1 second
    assert!(tampered.validate(Some(version)).is_err());

    // 2. Tamper: final memory doubled (2x) but serialized ratio and pass flag unchanged
    let mut tampered = valid_report.clone();
    tampered.soak_results[0].final_settled_idle_rss_bytes =
        tampered.soak_results[0].initial_settled_idle_rss_bytes * 2;
    assert!(tampered.validate(Some(version)).is_err());

    // 3. Tamper: zero initial RSS
    let mut tampered = valid_report.clone();
    tampered.soak_results[0].initial_settled_idle_rss_bytes = 0;
    assert!(tampered.validate(Some(version)).is_err());

    // 4. Tamper: missing primary architecture (remove riscv64)
    let mut tampered = valid_report.clone();
    tampered
        .soak_results
        .retain(|s| s.target_architecture != "riscv64");
    let err = tampered.validate(Some(version)).unwrap_err();
    assert!(
        err.to_string()
            .contains("missing required primary architecture soak record: 'riscv64'")
    );

    // 5. Tamper: restart duration exceeds bound but passed=true
    let mut tampered = valid_report.clone();
    tampered.restart_results[0].observed_duration_ms = 999_999;
    assert!(tampered.validate(Some(version)).is_err());

    // 6. Tamper: restart case missing
    let mut tampered = valid_report.clone();
    tampered
        .restart_results
        .retain(|r| r.id != "RST-RESET-CLEANUP");
    assert!(tampered.validate(Some(version)).is_err());

    // 7. Tamper: historical regression failed
    let mut tampered = valid_report.clone();
    tampered.historical_regressions[0].passed = false;
    assert!(tampered.validate(Some(version)).is_err());

    // 8. Tamper: epic regression failed
    let mut tampered = valid_report.clone();
    tampered.epic_regressions[0].passed = false;
    assert!(tampered.validate(Some(version)).is_err());

    // 9. Tamper: overall_qualified false
    let mut tampered = valid_report.clone();
    tampered.overall_qualified = false;
    assert!(tampered.validate(Some(version)).is_err());
}
