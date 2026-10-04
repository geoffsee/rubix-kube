//! Budget profiling, component breakdown, regression analysis, and scope decision tests.

use rubix_dev::perf::harness::{build_candidate_fixture, build_reference_fixture};
use rubix_dev::perf::{
    Architecture, BudgetDomain, BudgetProfileReport, PerformanceReport, VarianceSummary,
    generate_budget_markdown_report, verify_optimization_parity,
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

fn fixture_pair(arch: Architecture) -> (PerformanceReport, PerformanceReport) {
    (build_reference_fixture(arch), build_candidate_fixture(arch))
}

#[test]
fn e01_budget_compliance_amd64_and_arm64() {
    for arch in [Architecture::Amd64, Architecture::Arm64] {
        let (reference, candidate) = fixture_pair(arch);
        let profile = BudgetProfileReport::analyze(&reference, &candidate).unwrap();

        assert_eq!(profile.architecture, arch);
        assert!(
            profile.all_budgets_satisfied,
            "all metrics must be within E01 budget thresholds for {arch:?}"
        );
        assert_eq!(
            profile.total_regressions_detected, 0,
            "candidate must not regress on any metric for {arch:?}"
        );
        assert_eq!(profile.metrics.len(), 12);

        // Verify each domain is represented
        for domain in [
            BudgetDomain::Startup,
            BudgetDomain::IdleMemory,
            BudgetDomain::BinaryDistribution,
            BudgetDomain::ResilienceAndDensity,
        ] {
            assert!(
                profile.metrics.iter().any(|m| m.domain == domain),
                "domain {domain:?} must have metrics"
            );
        }
    }
}

#[test]
fn retained_process_profiling_and_apiserver_dominance() {
    let (reference, candidate) = fixture_pair(Architecture::Amd64);
    let profile = BudgetProfileReport::analyze(&reference, &candidate).unwrap();

    assert_eq!(profile.process_profiles.len(), 8);
    // Rank 1 must be apiserver
    let top = &profile.process_profiles[0];
    assert_eq!(top.role, "apiserver");
    assert!(top.process_name.contains("apiserver"));
    // apiserver consumes > 200 MiB PSS
    assert!(
        top.pss_bytes >= 200 * 1024 * 1024,
        "kube-apiserver alone must exceed 200 MiB PSS"
    );
    assert!(
        top.pss_fraction_percent > 40.0,
        "kube-apiserver should account for > 40% of total node idle PSS"
    );

    // Sum of all retained processes must exceed 400 MiB
    let total_pss: u64 = profile.process_profiles.iter().map(|p| p.pss_bytes).sum();
    assert!(
        total_pss >= 400 * 1024 * 1024,
        "whole-node idle PSS must honestly reflect supervised components"
    );

    // Verify all 8 required roles are accounted for
    for required in rubix_dev::perf::REQUIRED_RETAINED_PROCESSES {
        assert!(
            profile.process_profiles.iter().any(|p| p.role == required),
            "process role '{required}' must be present in profile"
        );
    }
}

#[test]
fn binary_distribution_profiling_and_node_daemon_share() {
    let (reference, candidate) = fixture_pair(Architecture::Amd64);
    let profile = BudgetProfileReport::analyze(&reference, &candidate).unwrap();

    assert_eq!(profile.binary_profiles.len(), 9);
    // Top 3 binaries must be core Kubernetes binaries
    let top_names: Vec<&str> = profile
        .binary_profiles
        .iter()
        .take(3)
        .map(|b| b.component_name.as_str())
        .collect();
    assert!(top_names.contains(&"kube-apiserver"));
    assert!(top_names.contains(&"kube-controller-manager"));
    assert!(top_names.contains(&"kubelet"));

    // rubix-kube must be a small fraction (< 10%) of extracted binaries
    let daemon = profile
        .binary_profiles
        .iter()
        .find(|b| b.component_name == "rubix-kube")
        .expect("rubix-kube must be in binary profile");
    assert!(
        daemon.fraction_percent < 10.0,
        "rubix-kube should be < 10% of total extracted payload"
    );
}

#[test]
fn scoped_optimizations_quantified_with_parity_invariants() {
    let (reference, candidate) = fixture_pair(Architecture::Amd64);
    let profile = BudgetProfileReport::analyze(&reference, &candidate).unwrap();

    assert_eq!(profile.optimizations.len(), 4);

    let daemon_mem = profile
        .optimizations
        .iter()
        .find(|o| o.title.contains("Node Daemon Idle Memory"))
        .unwrap();
    assert!(
        daemon_mem.reduction_percent > 50.0,
        "daemon memory optimization must achieve > 50% reduction"
    );
    assert!(
        daemon_mem.parity_invariants_verified.is_empty(),
        "input comparisons cannot establish protocol or lifecycle parity"
    );

    let daemon_bin = profile
        .optimizations
        .iter()
        .find(|o| o.title.contains("Node Executable Distribution Size"))
        .unwrap();
    assert!(
        daemon_bin.reduction_percent > 50.0,
        "daemon binary size reduction must exceed 50%"
    );

    let archive_opt = profile
        .optimizations
        .iter()
        .find(|o| o.title.contains("Release Archive Compression"))
        .unwrap();
    assert!(
        archive_opt.reduction_percent > 10.0,
        "archive compression must achieve > 10% reduction"
    );

    let boot_opt = profile
        .optimizations
        .iter()
        .find(|o| o.title.contains("Boot-to-API Comparison"))
        .unwrap();
    assert!(
        boot_opt.reduction_percent > 20.0,
        "boot-to-API latency must improve by > 20%"
    );
}

#[test]
fn regression_detection_and_profiling() {
    let (reference, mut candidate) = fixture_pair(Architecture::Amd64);

    // Simulate regression: bloat node daemon memory to 60 MiB (> Go ref 44 MiB)
    for p in &mut candidate.idle_footprint.retained_processes {
        if p.process_name.contains("node-daemon") {
            p.pss_bytes = 60 * 1024 * 1024;
            p.rss_bytes = 65 * 1024 * 1024;
        }
    }
    // Recompute sample pss
    candidate.idle_footprint.summed_pss_bytes =
        VarianceSummary::from_samples(vec![600_000_000.0; 5]).unwrap();

    let profile = BudgetProfileReport::analyze(&reference, &candidate).unwrap();
    assert!(
        profile.total_regressions_detected >= 1,
        "regression in idle memory must be detected"
    );
    let mem_metric = profile
        .metrics
        .iter()
        .find(|m| m.name.contains("Summed PSS"))
        .unwrap();
    assert!(
        mem_metric.is_regression,
        "summed PSS must be flagged as regression"
    );

    // Simulate fatal budget violation: cold boot p95 exceeds 1.10x threshold
    let ref_cold = reference.startup_latencies.first_pod_cold_seconds.p95;
    let limit = 1.10 * ref_cold;
    candidate.startup_latencies.first_pod_cold_seconds =
        VarianceSummary::from_samples(vec![limit + 5.0; 20]).unwrap();

    let profile2 = BudgetProfileReport::analyze(&reference, &candidate).unwrap();
    assert!(
        !profile2.all_budgets_satisfied,
        "candidate exceeding limit must fail all_budgets_satisfied"
    );
}

#[test]
fn parity_verification_enforces_required_processes_and_stability() {
    let (_, mut candidate) = fixture_pair(Architecture::Amd64);

    // 1. Missing required process fails parity
    candidate
        .idle_footprint
        .retained_processes
        .retain(|p| !p.process_name.contains("kine"));
    assert!(
        verify_optimization_parity(&candidate).is_err(),
        "omitting kine must violate parity"
    );

    // 2. Soak crashes fail parity
    let (_, mut candidate2) = fixture_pair(Architecture::Amd64);
    candidate2.sustained_growth.crash_count = 1;
    assert!(
        verify_optimization_parity(&candidate2).is_err(),
        "soak crash must violate parity"
    );

    // 3. Surviving processes fail parity
    let (_, mut candidate3) = fixture_pair(Architecture::Amd64);
    candidate3.shutdown.surviving_owned_processes = 1;
    assert!(
        verify_optimization_parity(&candidate3).is_err(),
        "surviving owned process must violate parity"
    );

    // 4. Probe success rate < 1.0 fails parity
    let (_, mut candidate4) = fixture_pair(Architecture::Amd64);
    candidate4.pod_density.probe_success_rate = 0.99;
    assert!(
        verify_optimization_parity(&candidate4).is_err(),
        "probe success rate < 1.0 must violate parity"
    );
}

#[test]
fn explicit_scope_decisions_inventory() {
    let (reference, candidate) = fixture_pair(Architecture::Amd64);
    let profile = BudgetProfileReport::analyze(&reference, &candidate).unwrap();

    let decision_ids: Vec<&str> = profile
        .scope_decisions
        .iter()
        .map(|d| d.decision_id.as_str())
        .collect();

    assert!(decision_ids.contains(&"DEC-01-SUB200MB-REFUSAL"));
    assert!(decision_ids.contains(&"DEC-02-NO-LOW-MEMORY-EDGE-OVERRIDES"));
    assert!(decision_ids.contains(&"DEC-03-UNDER-60S-STARTUP-REFUSAL"));
    assert!(decision_ids.contains(&"DEC-04-IMAGE-PAYLOAD-SEPARATION"));
    assert!(decision_ids.contains(&"DEC-05-SECONDARY-TARGET-GAPS"));

    for dec in &profile.scope_decisions {
        assert!(!dec.title.is_empty());
        assert!(!dec.empirical_evidence.is_empty());
        assert!(!dec.contract_justification.is_empty());
        assert_eq!(dec.status, "Enforced & Documented");
    }
}

#[test]
fn cli_profile_command_generates_markdown_and_fails_closed() {
    let directory = root().join("tools/perf");
    for architecture in ["amd64", "arm64"] {
        let reference = directory.join(format!("fixtures/{architecture}-reference-go.json"));
        let candidate = directory.join(format!("fixtures/{architecture}-candidate-rust.json"));

        let output = Command::new(env!("CARGO_BIN_EXE_rubix-perf"))
            .arg("profile")
            .arg("--reference")
            .arg(&reference)
            .arg("--candidate")
            .arg(&candidate)
            .output()
            .unwrap();

        // Must fail closed because live capture importer is not implemented
        assert!(
            !output.status.success(),
            "profile must fail closed on synthetic fixtures"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(stdout.contains("Budget Profiling & Scope Decision Analysis"));
        assert!(stdout.contains("Retained Process Memory Profiling Breakdown"));
        assert!(stdout.contains("Scoped Optimizations with Before/After Justifications"));
        assert!(stdout.contains("DEC-01-SUB200MB-REFUSAL"));
        assert!(stdout.contains("DEC-02-NO-LOW-MEMORY-EDGE-OVERRIDES"));
        assert!(stdout.contains("DEC-03-UNDER-60S-STARTUP-REFUSAL"));
        assert!(stderr.contains("profile is not qualified performance evidence"));
    }
}

#[test]
fn markdown_report_generation_structure() {
    let (reference, candidate) = fixture_pair(Architecture::Amd64);
    let profile = BudgetProfileReport::analyze(&reference, &candidate).unwrap();
    let md = generate_budget_markdown_report(&profile);

    assert!(
        md.contains("# Budget Profiling & Scope Decision Analysis: AMD64 Candidate vs Reference")
    );
    assert!(md.contains("## 1. Executive Summary & E01 Budget Evaluation"));
    assert!(md.contains("## 2. Metric-by-Metric Budget Comparison"));
    assert!(md.contains("## 3. Retained Process Memory Profiling Breakdown"));
    assert!(md.contains("## 4. Component Binary Footprint Profiling"));
    assert!(md.contains("## 5. Scoped Optimizations with Before/After Justifications"));
    assert!(md.contains("## 6. Explicit Budget Scope Decisions with Evidence"));
    assert!(md.contains("ALL 12 GATES SATISFIED WITHIN E01 CONTRACT BUDGETS"));
}

#[test]
fn report_conclusions_use_actual_inputs_without_verified_attribution() {
    let (mut reference, mut candidate) = fixture_pair(Architecture::Arm64);
    reference.startup_latencies.first_pod_cold_seconds =
        VarianceSummary::from_samples(vec![71.23; 20]).unwrap();
    candidate
        .idle_footprint
        .retained_processes
        .iter_mut()
        .find(|p| p.process_name == "kubelet")
        .unwrap()
        .pss_bytes = 900 * 1024 * 1024;
    candidate.idle_footprint.summed_pss_bytes =
        VarianceSummary::from_samples(vec![900_000_000.0; 5]).unwrap();
    let report = BudgetProfileReport::analyze(&reference, &candidate).unwrap();
    assert!(
        report
            .optimizations
            .iter()
            .all(|o| o.parity_invariants_verified.is_empty())
    );
    let decision = report
        .scope_decisions
        .iter()
        .find(|d| d.decision_id.starts_with("DEC-03"))
        .unwrap();
    assert!(decision.empirical_evidence.contains("71.23s"));
    assert!(decision.empirical_evidence.contains("arm64"));
    assert!(!decision.empirical_evidence.contains("amd64"));
    let md = generate_budget_markdown_report(&report);
    assert!(!md.contains("equal or better on all primary metrics"));
    assert!(md.contains("`kubelet` has the largest"));
    assert!(!md.contains("zstd-19"));
    assert!(!md.contains("Parity Invariants Verified"));
    let binary_share: f64 = report
        .binary_profiles
        .iter()
        .map(|p| p.fraction_percent)
        .sum();
    assert!((binary_share - 100.0).abs() < 0.001);
}

#[test]
fn profile_uses_authoritative_shutdown_outcome_beyond_p95() {
    let (reference, mut candidate) = fixture_pair(Architecture::Amd64);
    let mut samples = vec![1.0; 19];
    samples.push(36.0);
    candidate.shutdown.graceful_duration_seconds = VarianceSummary::from_samples(samples).unwrap();
    let profile = BudgetProfileReport::analyze(&reference, &candidate).unwrap();
    let gate = profile
        .metrics
        .iter()
        .find(|m| m.name.contains("Shutdown"))
        .unwrap();
    assert!((gate.candidate_value - 1.0).abs() < f64::EPSILON);
    assert!(!gate.within_budget);
    assert!(!profile.all_budgets_satisfied);
}
