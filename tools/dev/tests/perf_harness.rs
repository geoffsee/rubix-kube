//! Arithmetic fixtures are useful test inputs, never evidence of live qualification.
use rubix_dev::perf::harness::{build_candidate_fixture, build_reference_fixture};
use rubix_dev::perf::{
    Architecture, GateEvaluationReport, PerformanceReport, VarianceSummary,
    generate_markdown_report, load_report, verify_retained_process_coverage,
};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .into()
}
fn pair() -> (PerformanceReport, PerformanceReport) {
    (
        build_reference_fixture(Architecture::Amd64),
        build_candidate_fixture(Architecture::Amd64),
    )
}
fn assert_invalid(reference: &PerformanceReport, candidate: &PerformanceReport) {
    let evaluation = GateEvaluationReport::evaluate(reference, candidate);
    assert!(!evaluation.all_passed);
    assert!(
        evaluation.results.is_empty(),
        "invalid input must never print passing arithmetic"
    );
    assert!(!evaluation.validation_errors.is_empty());
}

#[test]
#[allow(clippy::float_cmp)]
fn nearest_rank_and_invalid_samples() {
    let summary = VarianceSummary::from_samples((1..=20).map(f64::from).collect()).unwrap();
    assert_eq!(
        (summary.count, summary.p50, summary.p90, summary.p95),
        (20, 10.0, 18.0, 19.0)
    );
    assert!((summary.variance - 35.0).abs() < 1e-9);
    for samples in [
        vec![],
        vec![f64::NAN],
        vec![f64::INFINITY],
        vec![-1.0],
        vec![f64::MAX; 20],
    ] {
        assert!(VarianceSummary::from_samples(samples).is_none());
    }
}

#[test]
fn synthetic_arithmetic_never_qualifies_and_directionality_uses_raw_values() {
    let (reference, mut candidate) = pair();
    let evaluation = GateEvaluationReport::evaluate(&reference, &candidate);
    assert!(!evaluation.all_passed);
    assert_eq!(evaluation.results.len(), 12);
    assert!(evaluation.results.iter().all(|g| g.passed));
    assert!(evaluation.validation_errors[0].contains("Not qualified"));
    candidate.pod_density.max_ready_replicas =
        VarianceSummary::from_samples(vec![90.0; 5]).unwrap();
    let density = GateEvaluationReport::evaluate(&reference, &candidate);
    assert!(
        !density
            .results
            .iter()
            .find(|g| g.name.contains("Pod Density"))
            .unwrap()
            .passed
    );
    candidate.startup_latencies.boot_to_api_seconds =
        VarianceSummary::from_samples(vec![1000.0; 20]).unwrap();
    let boot = GateEvaluationReport::evaluate(&reference, &candidate);
    assert!(!boot.results[0].passed);
}

#[test]
fn stale_statistics_and_incomplete_or_nonfinite_raw_samples_are_rejected() {
    let (reference, candidate) = pair();
    for mutation in 0..6 {
        let mut candidate = candidate.clone();
        let summary = &mut candidate.startup_latencies.boot_to_api_seconds;
        match mutation {
            0 => {
                summary.samples = vec![1000.0];
                summary.count = 1;
            },
            1 => summary.p95 = 0.01,
            2 => summary.mean = 0.01,
            3 => summary.variance = f64::NAN,
            4 => summary.samples[0] = f64::INFINITY,
            _ => summary.samples[0] = -1.0,
        }
        assert_invalid(&reference, &candidate);
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stale.json");
    let mut stale = candidate;
    stale.startup_latencies.boot_to_api_seconds.p95 = 0.01;
    rubix_dev::perf::save_report(&path, &stale).unwrap();
    assert!(load_report(&path).is_err());
}

#[test]
fn unmatched_environment_payload_and_wrong_implementation_are_rejected() {
    let (reference, candidate) = pair();
    for mutation in 0..7 {
        let mut candidate = candidate.clone();
        match mutation {
            0 => {
                candidate.architecture = Architecture::Arm64;
                candidate.hardware.architecture = Architecture::Arm64;
            },
            1 => candidate.hardware.machine_model = "different".into(),
            2 => candidate.hardware.kernel_version = "different".into(),
            3 => {
                candidate.workload.density_node_memory_limit_bytes *= 2;
                candidate.pod_density.node_memory_limit_bytes *= 2;
            },
            4 => candidate.workload.probe_digest = "different".into(),
            5 => candidate.versions.containerd = "different".into(),
            _ => candidate.implementation = rubix_dev::perf::ImplementationKind::Go,
        }
        assert_invalid(&reference, &candidate);
    }
}

#[test]
fn soak_is_derived_duration_bound_and_observed_failures_are_reported() {
    let (reference, candidate) = pair();
    for mutation in 0..5 {
        let mut candidate = candidate.clone();
        match mutation {
            0 => {
                candidate.sustained_growth.initial_settled_idle_median_bytes = 100;
                candidate.sustained_growth.final_settled_idle_median_bytes = 1000;
                candidate.sustained_growth.growth_ratio = 1.0;
            },
            1 => candidate.sustained_growth.duration_hours = 0,
            2 => candidate.sustained_growth.initial_settled_idle_median_bytes = 0,
            3 => candidate.sustained_growth.oom_kill_count = 3,
            _ => candidate.sustained_growth.crash_count = 2,
        }
        let evaluation = GateEvaluationReport::evaluate(&reference, &candidate);
        assert!(
            !evaluation
                .results
                .iter()
                .find(|g| g.name.contains("Sustained Growth"))
                .unwrap()
                .passed
        );
        let text = generate_markdown_report(&reference, &candidate, &evaluation, None);
        assert!(text.contains("NOT QUALIFIED"));
        if mutation == 3 {
            assert!(text.contains("3 OOMs"));
        }
        if mutation == 4 {
            assert!(text.contains("2 crashes"));
        }
    }
}

#[test]
fn retained_daemon_and_shim_are_independent_roles() {
    for removed in ["containerd", "containerd-shim-runc-v2", "kube-apiserver"] {
        let mut candidate = build_candidate_fixture(Architecture::Amd64);
        candidate
            .idle_footprint
            .retained_processes
            .retain(|p| p.process_name != removed);
        assert!(verify_retained_process_coverage(&candidate).is_err());
    }
}

#[test]
fn a_single_deadline_violation_is_not_hidden_by_shutdown_p95() {
    let (reference, mut candidate) = pair();
    let mut samples = vec![1.0; 19];
    samples.push(36.0);
    candidate.shutdown.graceful_duration_seconds = VarianceSummary::from_samples(samples).unwrap();
    let evaluation = GateEvaluationReport::evaluate(&reference, &candidate);
    assert!(
        !evaluation
            .results
            .iter()
            .find(|g| g.name.contains("Shutdown"))
            .unwrap()
            .passed
    );
}

#[test]
fn fixture_integrity_and_all_cli_qualification_paths_fail_closed() {
    let directory = root().join("tools/perf");
    let provenance: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("provenance.json")).unwrap()).unwrap();
    assert_eq!(provenance["evidence_kind"], "synthetic-fixture");
    for (name, digest) in provenance["files"].as_object().unwrap() {
        assert_eq!(
            digest.as_str().unwrap(),
            rubix_dev::sha256(&std::fs::read(directory.join(name)).unwrap())
        );
    }
    for architecture in ["amd64", "arm64"] {
        let reference = directory.join(format!("fixtures/{architecture}-reference-go.json"));
        let candidate = directory.join(format!("fixtures/{architecture}-candidate-rust.json"));
        let evaluation = GateEvaluationReport::evaluate(
            &load_report(&reference).unwrap(),
            &load_report(&candidate).unwrap(),
        );
        assert!(!evaluation.all_passed);
        for command in ["evaluate-gates", "report"] {
            let output = Command::new(env!("CARGO_BIN_EXE_rubix-perf"))
                .arg(command)
                .arg("--reference")
                .arg(&reference)
                .arg("--candidate")
                .arg(&candidate)
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(!String::from_utf8_lossy(&output.stdout).contains("Overall: PASS"));
        }
    }
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-perf"))
        .arg("check-baselines")
        .arg(directory.join("fixtures"))
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn generated_fixtures_and_relabelled_captures_cannot_qualify() {
    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-perf"))
        .arg("generate-fixtures")
        .arg(directory.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let path = directory.path().join("fixtures/amd64-candidate-rust.json");
    let mut candidate = load_report(&path).unwrap();
    candidate.evidence_kind = rubix_dev::perf::metrics::EvidenceKind::UnverifiedCapture;
    let reference = build_reference_fixture(Architecture::Amd64);
    assert!(!GateEvaluationReport::evaluate(&reference, &candidate).all_passed);
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-perf"))
        .arg("check-baselines")
        .arg(directory.path().join("fixtures"))
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn secondary_contract_rejects_tampered_platform_claims() {
    use rubix_dev::perf::SecondaryTargetsRegistry;

    let expected = SecondaryTargetsRegistry::default_contract();
    expected.validate().unwrap();
    for architecture in ["armv7", "riscv64"] {
        for field in [
            "architecture",
            "status",
            "address_space_bits",
            "maximum_pod_density",
            "d2k_supported",
            "portainer_supported",
            "crun_source_build_required",
            "cold_boot_latency_overhead_multiplier",
        ] {
            let mut encoded = serde_json::to_value(&expected).unwrap();
            let value = &mut encoded["targets"][architecture][field];
            *value = match value {
                serde_json::Value::Bool(original) => serde_json::json!(!*original),
                serde_json::Value::Number(_) => serde_json::json!(123),
                _ => serde_json::json!("qualified"),
            };
            let altered: SecondaryTargetsRegistry = serde_json::from_value(encoded).unwrap();
            assert!(
                altered.validate().is_err(),
                "accepted {architecture}.{field}"
            );
        }
    }
}

#[test]
fn report_rejects_invalid_secondary_registry_before_markdown_output() {
    use rubix_dev::perf::SecondaryTargetsRegistry;

    let directory = tempfile::tempdir().unwrap();
    let secondary = directory.path().join("secondary.json");
    let mut registry = SecondaryTargetsRegistry::default_contract();
    registry.targets.get_mut("riscv64").unwrap().d2k_supported = true;
    std::fs::write(&secondary, serde_json::to_vec(&registry).unwrap()).unwrap();
    let fixtures = root().join("tools/perf/fixtures");
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-perf"))
        .arg("report")
        .arg("--reference")
        .arg(fixtures.join("amd64-reference-go.json"))
        .arg("--candidate")
        .arg(fixtures.join("amd64-candidate-rust.json"))
        .arg("--secondary")
        .arg(&secondary)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "invalid claims must never be published"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("secondary targets validation failed")
    );
}

#[test]
fn deliberately_regressed_samples_fail_every_contract_gate() {
    let (reference, base_candidate) = pair();

    // Verify baseline passes all 12 gates
    let baseline_eval = GateEvaluationReport::evaluate(&reference, &base_candidate);
    assert_eq!(baseline_eval.results.len(), 12);
    assert!(baseline_eval.arithmetic_all_passed());

    check_startup_latency_regressions(&reference, &base_candidate);
    check_footprint_and_artifact_regressions(&reference, &base_candidate);
    check_capacity_growth_and_shutdown_regressions(&reference, &base_candidate);
}

fn check_startup_latency_regressions(
    reference: &PerformanceReport,
    base_candidate: &PerformanceReport,
) {
    // 1. Boot-to-API latency regressed (> 1.10x reference)
    let mut cand = base_candidate.clone();
    cand.startup_latencies.boot_to_api_seconds =
        VarianceSummary::from_samples(vec![
            reference.startup_latencies.boot_to_api_seconds.p95
                * 1.25;
            20
        ])
        .unwrap();
    let eval = GateEvaluationReport::evaluate(reference, &cand);
    assert!(!eval.arithmetic_all_passed());
    assert!(
        !eval
            .results
            .iter()
            .find(|g| g.name.contains("Boot-to-API"))
            .unwrap()
            .passed
    );

    // 2. Node Ready latency regressed (> 1.10x reference)
    let mut cand = base_candidate.clone();
    cand.startup_latencies.node_ready_seconds =
        VarianceSummary::from_samples(vec![
            reference.startup_latencies.node_ready_seconds.p95
                * 1.25;
            20
        ])
        .unwrap();
    let eval = GateEvaluationReport::evaluate(reference, &cand);
    assert!(!eval.arithmetic_all_passed());
    assert!(
        !eval
            .results
            .iter()
            .find(|g| g.name.contains("Node Ready"))
            .unwrap()
            .passed
    );

    // 3. First Pod preloaded latency regressed (> 1.10x reference)
    let mut cand = base_candidate.clone();
    cand.startup_latencies.first_pod_preloaded_seconds = VarianceSummary::from_samples(vec![
        reference
            .startup_latencies
            .first_pod_preloaded_seconds
            .p95
            * 1.25;
        20
    ])
    .unwrap();
    let eval = GateEvaluationReport::evaluate(reference, &cand);
    assert!(!eval.arithmetic_all_passed());
    assert!(
        !eval
            .results
            .iter()
            .find(|g| g.name.contains("Preloaded"))
            .unwrap()
            .passed
    );

    // 4. First Pod cold latency regressed (> 1.10x reference)
    let mut cand = base_candidate.clone();
    cand.startup_latencies.first_pod_cold_seconds =
        VarianceSummary::from_samples(vec![
            reference.startup_latencies.first_pod_cold_seconds.p95
                * 1.25;
            20
        ])
        .unwrap();
    let eval = GateEvaluationReport::evaluate(reference, &cand);
    assert!(!eval.arithmetic_all_passed());
    assert!(
        !eval
            .results
            .iter()
            .find(|g| g.name.contains("Cold Image"))
            .unwrap()
            .passed
    );
}

fn check_footprint_and_artifact_regressions(
    reference: &PerformanceReport,
    base_candidate: &PerformanceReport,
) {
    // 5. Idle PSS regressed (> 1.10x reference)
    let mut cand = base_candidate.clone();
    cand.idle_footprint.summed_pss_bytes =
        VarianceSummary::from_samples(vec![
            reference.idle_footprint.summed_pss_bytes.p50 * 1.25;
            5
        ])
        .unwrap();
    let eval = GateEvaluationReport::evaluate(reference, &cand);
    assert!(!eval.arithmetic_all_passed());
    assert!(
        !eval
            .results
            .iter()
            .find(|g| g.name.contains("Summed PSS"))
            .unwrap()
            .passed
    );

    // 6. Idle Cgroup memory regressed (> 1.10x reference)
    let mut cand = base_candidate.clone();
    cand.idle_footprint.cgroup_memory_bytes =
        VarianceSummary::from_samples(vec![
            reference.idle_footprint.cgroup_memory_bytes.p50 * 1.25;
            5
        ])
        .unwrap();
    let eval = GateEvaluationReport::evaluate(reference, &cand);
    assert!(!eval.arithmetic_all_passed());
    assert!(
        !eval
            .results
            .iter()
            .find(|g| g.name.contains("Cgroup Memory"))
            .unwrap()
            .passed
    );

    // 7. Compressed archive size regressed (> 1.10x reference)
    let mut cand = base_candidate.clone();
    cand.artifact_footprint.compressed_archive_bytes =
        reference.artifact_footprint.compressed_archive_bytes * 5 / 4;
    let eval = GateEvaluationReport::evaluate(reference, &cand);
    assert!(!eval.arithmetic_all_passed());
    assert!(
        !eval
            .results
            .iter()
            .find(|g| g.name.contains("Compressed Archive"))
            .unwrap()
            .passed
    );

    // 8. Extracted executables size regressed (> 1.10x reference)
    let mut cand = base_candidate.clone();
    cand.artifact_footprint.extracted_executable_bytes =
        reference.artifact_footprint.extracted_executable_bytes * 5 / 4;
    let eval = GateEvaluationReport::evaluate(reference, &cand);
    assert!(!eval.arithmetic_all_passed());
    assert!(
        !eval
            .results
            .iter()
            .find(|g| g.name.contains("Extracted Executables"))
            .unwrap()
            .passed
    );

    // 9. Default image payload regressed (> 1.10x reference)
    let mut cand = base_candidate.clone();
    cand.artifact_footprint.default_image_payload_bytes =
        reference.artifact_footprint.default_image_payload_bytes * 5 / 4;
    let eval = GateEvaluationReport::evaluate(reference, &cand);
    assert!(!eval.arithmetic_all_passed());
    assert!(
        !eval
            .results
            .iter()
            .find(|g| g.name.contains("Default Image Payload"))
            .unwrap()
            .passed
    );
}

fn check_capacity_growth_and_shutdown_regressions(
    reference: &PerformanceReport,
    base_candidate: &PerformanceReport,
) {
    // 10. Pod density regressed (< 0.90x reference)
    let mut cand = base_candidate.clone();
    cand.pod_density.max_ready_replicas =
        VarianceSummary::from_samples(vec![reference.pod_density.max_ready_replicas.p50 * 0.80; 5])
            .unwrap();
    let eval = GateEvaluationReport::evaluate(reference, &cand);
    assert!(!eval.arithmetic_all_passed());
    assert!(
        !eval
            .results
            .iter()
            .find(|g| g.name.contains("Pod Density"))
            .unwrap()
            .passed
    );

    // 11. Sustained growth regressions (ratio > 1.10, OOM > 0, crash > 0, failures > 0, duration < 24)
    for (idx, (ratio, oom, crash, failures, hours)) in [
        (1.25f64, 0, 0, 0, 24),
        (1.01f64, 1, 0, 0, 24),
        (1.01f64, 0, 1, 0, 24),
        (1.01f64, 0, 0, 1, 24),
        (1.01f64, 0, 0, 0, 12),
    ]
    .into_iter()
    .enumerate()
    {
        let mut cand = base_candidate.clone();
        cand.sustained_growth.growth_ratio = ratio;
        let init_bytes = cand.sustained_growth.initial_settled_idle_median_bytes;
        cand.sustained_growth.final_settled_idle_median_bytes = if (ratio - 1.25f64).abs() < 0.01 {
            init_bytes * 5 / 4
        } else {
            (init_bytes * 101) / 100
        };
        cand.sustained_growth.oom_kill_count = oom;
        cand.sustained_growth.crash_count = crash;
        cand.sustained_growth.unexplained_failures = failures;
        cand.sustained_growth.duration_hours = hours;
        let eval = GateEvaluationReport::evaluate(reference, &cand);
        assert!(
            !eval.arithmetic_all_passed(),
            "sustained growth case {idx} should fail"
        );
        assert!(
            !eval
                .results
                .iter()
                .find(|g| g.name.contains("Sustained Growth"))
                .unwrap()
                .passed
        );
    }

    // 12. Shutdown regressions (graceful > 30s, escalation > 35s, surviving > 0, unrelated killed > 0)
    for (idx, (graceful, escalation, surviving, unrelated)) in [
        (32.0, 0.0, 0, 0),
        (10.0, 36.0, 0, 0),
        (10.0, 0.0, 1, 0),
        (10.0, 0.0, 0, 1),
    ]
    .into_iter()
    .enumerate()
    {
        let mut cand = base_candidate.clone();
        cand.shutdown.graceful_duration_seconds =
            VarianceSummary::from_samples(vec![graceful; 20]).unwrap();
        cand.shutdown.escalation_duration_seconds =
            VarianceSummary::from_samples(vec![escalation; 20]).unwrap();
        cand.shutdown.surviving_owned_processes = surviving;
        cand.shutdown.unrelated_processes_killed = unrelated;
        let eval = GateEvaluationReport::evaluate(reference, &cand);
        assert!(
            !eval.arithmetic_all_passed(),
            "shutdown regression case {idx} should fail"
        );
        assert!(
            !eval
                .results
                .iter()
                .find(|g| g.name.contains("Shutdown"))
                .unwrap()
                .passed
        );
    }
}

#[test]
fn pod_density_directionality_higher_is_better_verified() {
    let (reference, base_candidate) = pair();
    let ref_density = reference.pod_density.max_ready_replicas.p50; // 110.0
    let threshold = 0.90 * ref_density; // 99.0

    for (density_val, should_pass, desc) in [
        (
            130.0,
            true,
            "superior density exceeds reference (130 > 110)",
        ),
        (110.0, true, "parity density matches reference (110 == 110)"),
        (105.0, true, "slight decline within budget (105 > 99)"),
        (99.0, true, "exact boundary threshold (99.0 == 99.0)"),
        (98.9, false, "just below boundary threshold (98.9 < 99.0)"),
        (90.0, false, "regressed density (90 < 99)"),
        (50.0, false, "severely collapsed capacity (50 < 99)"),
    ] {
        let mut cand = base_candidate.clone();
        cand.pod_density.max_ready_replicas =
            VarianceSummary::from_samples(vec![density_val; 5]).unwrap();
        let eval = GateEvaluationReport::evaluate(&reference, &cand);
        let gate = eval
            .results
            .iter()
            .find(|g| g.name.contains("Pod Density"))
            .unwrap();
        assert!(
            gate.higher_is_better,
            "Pod density must be higher_is_better"
        );
        assert!((gate.threshold_multiplier - 0.90).abs() < f64::EPSILON);
        assert!((gate.target_threshold - threshold).abs() < f64::EPSILON);
        assert_eq!(
            gate.passed, should_pass,
            "Pod density {density_val} failed expectation: {desc}"
        );
    }

    let mut cand_latency = base_candidate.clone();
    cand_latency.startup_latencies.boot_to_api_seconds =
        VarianceSummary::from_samples(vec![20.0; 20]).unwrap();
    let eval_lat = GateEvaluationReport::evaluate(&reference, &cand_latency);
    let lat_gate = eval_lat
        .results
        .iter()
        .find(|g| g.name.contains("Boot-to-API"))
        .unwrap();
    assert!(
        !lat_gate.higher_is_better,
        "Latency must be lower_is_better"
    );
    assert!(!lat_gate.passed, "Regressed latency must fail");
}

#[test]
fn repeated_workload_idle_cycles_show_bounded_memory_growth_across_full_distribution() {
    use rubix_dev::perf::{analyze_workload_idle_cycles, generate_soak_cycles};

    // Case 1: Clean 24-hour soak with bounded memory growth across all 8 retained processes
    let clean_cycles = generate_soak_cycles(Architecture::Amd64, None, None);
    assert_eq!(clean_cycles.len(), 24);
    let analysis = analyze_workload_idle_cycles(&clean_cycles).unwrap();
    assert!(analysis.is_bounded);
    assert!(analysis.growth_ratio <= 1.10);
    assert_eq!(analysis.oom_total, 0);
    assert_eq!(analysis.crash_total, 0);
    assert_eq!(analysis.failed_probes_total, 0);
    assert_eq!(analysis.process_growth_ratios.len(), 8);
    for (proc_name, ratio) in &analysis.process_growth_ratios {
        assert!(
            *ratio <= 1.05,
            "retained process {proc_name} grew beyond expected jitter: {ratio}"
        );
    }

    // Case 2: Memory leak in a specific retained process (kubelet leaking 3% per cycle)
    let leaky_cycles = generate_soak_cycles(Architecture::Amd64, Some(("kubelet", 0.03)), None);
    let leaky_analysis = analyze_workload_idle_cycles(&leaky_cycles).unwrap();
    assert!(
        !leaky_analysis.is_bounded,
        "leaking kubelet must not be bounded"
    );
    assert!(
        *leaky_analysis.process_growth_ratios.get("kubelet").unwrap() > 1.50,
        "kubelet should show significant leak"
    );

    // Case 3: OOM kill occurring during soak
    let oom_cycles = generate_soak_cycles(Architecture::Amd64, None, Some(14));
    let oom_analysis = analyze_workload_idle_cycles(&oom_cycles).unwrap();
    assert!(
        !oom_analysis.is_bounded,
        "OOM event must fail bounded check"
    );
    assert_eq!(oom_analysis.oom_total, 1);

    // Case 4: Incomplete distribution (omitting one of the 8 canonical processes)
    let mut incomplete_cycles = clean_cycles.clone();
    for c in &mut incomplete_cycles {
        c.process_breakdown
            .retain(|p| p.process_name != "containerd-shim-runc-v2");
    }
    assert!(
        analyze_workload_idle_cycles(&incomplete_cycles).is_err(),
        "dropping a canonical retained process must fail cycle analysis"
    );
}

#[test]
fn unmeasured_footprint_and_startup_claims_are_rejected() {
    let (_, base_candidate) = pair();

    // 1. Dishonest sub-200MB footprint claim
    let mut cand = base_candidate.clone();
    cand.idle_footprint.summed_pss_bytes =
        VarianceSummary::from_samples(vec![150_000_000.0; 5]).unwrap();
    assert!(
        rubix_dev::perf::validation::validate_report(&cand).is_err(),
        "sub-200MB claim contradicting retained processes must be rejected"
    );

    // 2. Retained process with 0 memory (claiming zero-cost component)
    let mut cand = base_candidate.clone();
    cand.idle_footprint.retained_processes[0].pss_bytes = 0;
    assert!(
        rubix_dev::perf::validation::validate_report(&cand).is_err(),
        "zero PSS process must be rejected"
    );

    // 3. Physically impossible PSS > RSS
    let mut cand = base_candidate.clone();
    cand.idle_footprint.retained_processes[0].pss_bytes =
        cand.idle_footprint.retained_processes[0].rss_bytes + 1_000_000;
    assert!(
        rubix_dev::perf::validation::validate_report(&cand).is_err(),
        "PSS > RSS must be rejected"
    );

    // 4. Negative latency sample
    let mut cand = base_candidate.clone();
    cand.startup_latencies.boot_to_api_seconds.samples[0] = -5.0;
    assert!(
        rubix_dev::perf::validation::validate_report(&cand).is_err(),
        "negative latency must be rejected"
    );
}

#[test]
fn ci_gate_enforces_committed_thresholds_and_rebaseline_policy() {
    let perf_dir = root().join("tools/perf");

    // 1. gate-ci passes on clean committed baselines
    rubix_dev::perf::run_ci_regression_gates(&perf_dir)
        .expect("clean committed baselines must pass CI gate");

    let output = Command::new(env!("CARGO_BIN_EXE_rubix-perf"))
        .arg("gate-ci")
        .arg(&perf_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Performance CI regression gating PASSED"));

    // 2. Tampered threshold relaxation in inputs.json fails CI gate
    let tmp = tempfile::tempdir().unwrap();
    for file in [
        "inputs.json",
        "provenance.json",
        "fixtures/amd64-reference-go.json",
        "fixtures/amd64-candidate-rust.json",
        "fixtures/arm64-reference-go.json",
        "fixtures/arm64-candidate-rust.json",
        "fixtures/paired-comparison.json",
        "fixtures/secondary-targets.json",
    ] {
        let src = perf_dir.join(file);
        let dst = tmp.path().join(file);
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::copy(&src, &dst).unwrap();
    }

    let inputs_path = tmp.path().join("inputs.json");
    let mut inputs_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&inputs_path).unwrap()).unwrap();
    inputs_json["contract_thresholds"]["pod_density_median_multiplier"] = serde_json::json!(0.70);
    std::fs::write(
        &inputs_path,
        serde_json::to_vec_pretty(&inputs_json).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rubix-perf"))
        .arg("gate-ci")
        .arg(tmp.path())
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "relaxed threshold must fail gate-ci"
    );

    // 3. Missing architecture baseline fails CI gate
    let tmp2 = tempfile::tempdir().unwrap();
    for file in [
        "inputs.json",
        "provenance.json",
        "fixtures/amd64-reference-go.json",
        "fixtures/amd64-candidate-rust.json",
        "fixtures/secondary-targets.json",
    ] {
        let src = perf_dir.join(file);
        let dst = tmp2.path().join(file);
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::copy(&src, &dst).unwrap();
    }
    assert!(
        rubix_dev::perf::run_ci_regression_gates(tmp2.path()).is_err(),
        "missing arm64 architecture must fail CI gate"
    );
}

fn copy_ci_fixture_tree() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    for file in [
        "provenance.json",
        "inputs.json",
        "fixtures/amd64-reference-go.json",
        "fixtures/amd64-candidate-rust.json",
        "fixtures/arm64-reference-go.json",
        "fixtures/arm64-candidate-rust.json",
        "fixtures/paired-comparison.json",
        "fixtures/secondary-targets.json",
    ] {
        let destination = directory.path().join(file);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(root().join("tools/perf").join(file), destination).unwrap();
    }
    directory
}

#[test]
fn ci_provenance_requires_exact_unique_complete_inventory() {
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root().join("tools/perf/provenance.json")).unwrap())
            .unwrap();
    for removed in source["files"].as_object().unwrap().keys() {
        let directory = copy_ci_fixture_tree();
        let mut provenance = source.clone();
        provenance["files"].as_object_mut().unwrap().remove(removed);
        std::fs::write(
            directory.path().join("provenance.json"),
            serde_json::to_vec(&provenance).unwrap(),
        )
        .unwrap();
        let error = rubix_dev::perf::run_ci_regression_gates(directory.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("missing required file entry"), "{error}");
        assert!(error.contains(removed), "{error}");
    }
    for unexpected in [
        "",
        "/tmp/input.json",
        "../inputs.json",
        "./inputs.json",
        "fixtures\\candidate.json",
        "extra.json",
    ] {
        let directory = copy_ci_fixture_tree();
        let mut provenance = source.clone();
        provenance["files"][unexpected] = serde_json::json!("0".repeat(64));
        std::fs::write(
            directory.path().join("provenance.json"),
            serde_json::to_vec(&provenance).unwrap(),
        )
        .unwrap();
        let error = rubix_dev::perf::run_ci_regression_gates(directory.path())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("unexpected provenance file entry"),
            "{error}"
        );
    }
    let directory = copy_ci_fixture_tree();
    let mut empty = source.clone();
    empty["files"] = serde_json::json!({});
    std::fs::write(
        directory.path().join("provenance.json"),
        serde_json::to_vec(&empty).unwrap(),
    )
    .unwrap();
    assert!(
        rubix_dev::perf::run_ci_regression_gates(directory.path())
            .unwrap_err()
            .to_string()
            .contains("missing required file entry")
    );
    let duplicate = serde_json::to_string(&source).unwrap().replacen(
        "\"files\":{",
        &format!("\"files\":{{\"inputs.json\":\"{}\",", "0".repeat(64)),
        1,
    );
    std::fs::write(directory.path().join("provenance.json"), duplicate).unwrap();
    assert!(
        rubix_dev::perf::run_ci_regression_gates(directory.path())
            .unwrap_err()
            .to_string()
            .contains("duplicate JSON key")
    );
}

#[test]
fn ci_gate_rejects_inconsistent_growth_after_valid_rehash() {
    for candidate_path in [
        "fixtures/amd64-candidate-rust.json",
        "fixtures/amd64-reference-go.json",
        "fixtures/arm64-candidate-rust.json",
        "fixtures/arm64-reference-go.json",
    ] {
        let directory = copy_ci_fixture_tree();
        let mut candidate: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.path().join(candidate_path)).unwrap())
                .unwrap();
        candidate["sustained_growth"]["growth_ratio"] = serde_json::json!(1.001);
        let bytes = serde_json::to_vec(&candidate).unwrap();
        std::fs::write(directory.path().join(candidate_path), &bytes).unwrap();
        let provenance_path = directory.path().join("provenance.json");
        let mut provenance: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&provenance_path).unwrap()).unwrap();
        provenance["files"][candidate_path] = serde_json::json!(rubix_dev::sha256(&bytes));
        std::fs::write(&provenance_path, serde_json::to_vec(&provenance).unwrap()).unwrap();
        let error = rubix_dev::perf::run_ci_regression_gates(directory.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("reported sustained growth ratio"), "{error}");
    }
}

#[test]
fn rebaseline_policy_cli_accepts_directory_alias_and_preserves_unqualified_label() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-perf"))
        .arg("check-rebaseline-policy")
        .arg(root().join("tools/perf"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("live performance NOT QUALIFIED"));
}
