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
