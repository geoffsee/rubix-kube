//! Integration tests for the performance measurement harness, statistical calculations,
//! contract gate evaluations, and committed baselines.

use rubix_dev::perf::{
    GateEvaluationReport, SecondaryTargetsRegistry, VarianceSummary, generate_markdown_report,
    load_report, verify_retained_process_coverage,
};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
#[allow(clippy::float_cmp)]
fn test_statistical_summary_and_nearest_rank_p95() {
    // 20 sorted values: 1.0, 2.0, ..., 20.0
    let samples: Vec<f64> = (1..=20).map(f64::from).collect();
    let summary = VarianceSummary::from_samples(samples).expect("summary from 20 samples");

    assert_eq!(summary.count, 20);
    assert_eq!(summary.min, 1.0);
    assert_eq!(summary.max, 20.0);
    assert_eq!(summary.mean, 10.5);

    // Nearest-rank percentile for P=95 and N=20:
    // rank = ceil((95 / 100) * 20) = ceil(19.0) = 19.
    // 19th element (1-indexed) is 19.0.
    assert_eq!(summary.p95, 19.0);

    // Median for P=50 and N=20:
    // rank = ceil((50 / 100) * 20) = ceil(10.0) = 10.
    assert_eq!(summary.p50, 10.0);
    assert_eq!(summary.p90, 18.0);

    // Sample variance for 1..20: sum((x - 10.5)^2) / 19 = 665 / 19 = 35.0
    assert!((summary.variance - 35.0).abs() < 1e-6);
    assert!((summary.std_dev - 35.0f64.sqrt()).abs() < 1e-6);
}

#[test]
fn test_gate_evaluation_directionality() {
    let ref_report =
        rubix_dev::perf::harness::build_reference_baseline(rubix_dev::perf::Architecture::Amd64);

    let mut cand_report =
        rubix_dev::perf::harness::build_candidate_baseline(rubix_dev::perf::Architecture::Amd64);

    // Normal candidate passes all gates
    let eval = GateEvaluationReport::evaluate(&ref_report, &cand_report);
    assert!(eval.all_passed, "Baseline candidate should pass all gates");

    // Test Pod Density: higher is better (threshold >= 0.90 * ref)
    // Ref density is 110.0. 0.90 * 110 = 99.0.
    // If candidate density is 90.0, it must fail!
    cand_report.pod_density.max_ready_replicas.p50 = 90.0;
    let eval_density_fail = GateEvaluationReport::evaluate(&ref_report, &cand_report);
    assert!(
        !eval_density_fail.all_passed,
        "Density 90.0 < 99.0 must fail the density gate"
    );
    let density_gate = eval_density_fail
        .results
        .iter()
        .find(|g| g.name.contains("Pod Density"))
        .expect("pod density gate");
    assert!(!density_gate.passed);
    assert!(density_gate.higher_is_better);

    // If candidate density is 100.0 (>= 99.0), it should pass
    cand_report.pod_density.max_ready_replicas.p50 = 100.0;
    let eval_density_pass = GateEvaluationReport::evaluate(&ref_report, &cand_report);
    let density_gate_pass = eval_density_pass
        .results
        .iter()
        .find(|g| g.name.contains("Pod Density"))
        .unwrap();
    assert!(density_gate_pass.passed);

    // Test Boot-to-API latency: lower is better (candidate p95 <= 1.10 * ref p95)
    // Ref p95 is ~14.05. Limit is ~15.455. If candidate is 16.0, it must fail!
    cand_report.startup_latencies.boot_to_api_seconds.p95 = 16.0;
    let eval_boot_fail = GateEvaluationReport::evaluate(&ref_report, &cand_report);
    let boot_gate = eval_boot_fail
        .results
        .iter()
        .find(|g| g.name.contains("Boot-to-API"))
        .unwrap();
    assert!(!boot_gate.passed);
    assert!(!boot_gate.higher_is_better);

    // Reset boot latency
    cand_report.startup_latencies.boot_to_api_seconds.p95 = 10.0;

    // Test Shutdown surviving processes: if > 0, must fail!
    cand_report.shutdown.surviving_owned_processes = 1;
    let eval_shutdown_fail = GateEvaluationReport::evaluate(&ref_report, &cand_report);
    let shutdown_gate = eval_shutdown_fail
        .results
        .iter()
        .find(|g| g.name.contains("Shutdown"))
        .unwrap();
    assert!(!shutdown_gate.passed);
}

#[test]
fn test_retained_process_coverage_enforcement() {
    let mut report =
        rubix_dev::perf::harness::build_candidate_baseline(rubix_dev::perf::Architecture::Amd64);

    // Full report has all 8 processes
    assert!(verify_retained_process_coverage(&report).is_ok());

    // Remove kube-apiserver
    report
        .idle_footprint
        .retained_processes
        .retain(|p| !p.process_name.contains("apiserver"));
    let err = verify_retained_process_coverage(&report).unwrap_err();
    assert!(err.to_string().contains("apiserver"));

    // Remove containerd
    let mut report2 =
        rubix_dev::perf::harness::build_candidate_baseline(rubix_dev::perf::Architecture::Amd64);
    report2
        .idle_footprint
        .retained_processes
        .retain(|p| !p.process_name.contains("containerd"));
    let err2 = verify_retained_process_coverage(&report2).unwrap_err();
    assert!(err2.to_string().contains("containerd"));
}

#[test]
fn test_committed_baselines_integrity_and_provenance() {
    let root = repo_root();
    let perf_dir = root.join("tools/perf");
    let baselines_dir = perf_dir.join("baselines");
    let prov_path = perf_dir.join("provenance.json");

    assert!(prov_path.is_file(), "tools/perf/provenance.json must exist");
    let prov_bytes = std::fs::read(&prov_path).expect("read provenance");
    let prov: serde_json::Value = serde_json::from_slice(&prov_bytes).expect("parse provenance");
    let files_map = prov["files"].as_object().expect("provenance files map");

    // Verify SHA-256 for all listed files
    for (rel_path, expected_hash_val) in files_map {
        let file_path = perf_dir.join(rel_path);
        assert!(file_path.is_file(), "File {rel_path} must exist on disk");
        let content = std::fs::read(&file_path).expect("read file for hash check");
        let actual_hash = rubix_dev::sha256(&content);
        assert_eq!(
            expected_hash_val.as_str().unwrap(),
            actual_hash,
            "Checksum mismatch for {rel_path}"
        );
    }

    // Load and verify both pairs
    let amd64_ref =
        load_report(&baselines_dir.join("amd64-reference-go.json")).expect("load amd64 ref");
    let amd64_cand =
        load_report(&baselines_dir.join("amd64-candidate-rust.json")).expect("load amd64 cand");
    let arm64_ref =
        load_report(&baselines_dir.join("arm64-reference-go.json")).expect("load arm64 ref");
    let arm64_cand =
        load_report(&baselines_dir.join("arm64-candidate-rust.json")).expect("load arm64 cand");

    for r in [&amd64_ref, &amd64_cand, &arm64_ref, &arm64_cand] {
        verify_retained_process_coverage(r).expect("all 8 retained processes covered");
    }

    let amd64_eval = GateEvaluationReport::evaluate(&amd64_ref, &amd64_cand);
    assert!(
        amd64_eval.all_passed,
        "amd64 candidate must pass all 12 contract gates against Go reference"
    );

    let arm64_eval = GateEvaluationReport::evaluate(&arm64_ref, &arm64_cand);
    assert!(
        arm64_eval.all_passed,
        "arm64 candidate must pass all 12 contract gates against Go reference"
    );

    // Verify paired-comparison.json
    let paired_path = baselines_dir.join("paired-comparison.json");
    let paired_bytes = std::fs::read(&paired_path).expect("read paired comparison");
    let paired: serde_json::Value =
        serde_json::from_slice(&paired_bytes).expect("parse paired comparison");
    assert_eq!(paired["all_passed"], true);

    // Verify secondary targets registry
    let sec_path = baselines_dir.join("secondary-targets.json");
    let sec_bytes = std::fs::read(&sec_path).expect("read secondary targets");
    let sec: SecondaryTargetsRegistry =
        serde_json::from_slice(&sec_bytes).expect("parse secondary targets");
    sec.validate().expect("secondary targets validation");

    let armv7_gap = sec.targets.get("armv7").expect("armv7 gap record");
    assert_eq!(armv7_gap.maximum_pod_density, 45);
    assert!(!armv7_gap.d2k_supported);
    assert!(armv7_gap.crun_source_build_required);

    let riscv64_gap = sec.targets.get("riscv64").expect("riscv64 gap record");
    assert_eq!(riscv64_gap.maximum_pod_density, 30);
    assert!(!riscv64_gap.portainer_supported);
    assert!(!riscv64_gap.d2k_supported);
}

#[test]
fn test_markdown_report_formatting() {
    let amd64_ref =
        rubix_dev::perf::harness::build_reference_baseline(rubix_dev::perf::Architecture::Amd64);
    let amd64_cand =
        rubix_dev::perf::harness::build_candidate_baseline(rubix_dev::perf::Architecture::Amd64);
    let eval = GateEvaluationReport::evaluate(&amd64_ref, &amd64_cand);
    let sec = SecondaryTargetsRegistry::default_contract();

    let md = generate_markdown_report(&amd64_ref, &amd64_cand, &eval, Some(&sec));
    assert!(md.contains("# Performance Baseline Comparison: AMD64 (Go) vs AMD64 (Rust)"));
    assert!(md.contains("## 1. Contract Gates Evaluation Summary"));
    assert!(md.contains("## 2. Declared Hardware & Workload Specification"));
    assert!(md.contains("## 3. Startup Latencies (20 Fresh Boots)"));
    assert!(md.contains("## 4. Whole-Distribution Idle Footprint & Retained Processes"));
    assert!(md.contains("## 5. Artifact Footprint"));
    assert!(md.contains("## 6. Pod Density & Soak Stability"));
    assert!(md.contains("## 7. Explicit Secondary Architecture Gaps"));
    assert!(md.contains("PASSED (All 12 Gates Met)"));
    assert!(md.contains("armv7"));
    assert!(md.contains("riscv64"));
}
