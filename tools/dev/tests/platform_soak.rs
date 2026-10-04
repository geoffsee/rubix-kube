use rubix_dev::platform_soak::{
    CandidateVerificationSummary, EnvironmentMapping, MatrixCompleteness, ObservedArtifact,
    PlatformSoakReport, PlatformSoakRunner, RestartSummary, SustainedSoakSummary,
};
use std::process::Command;

fn fixture() -> PlatformSoakReport {
    PlatformSoakRunner::new().run_fixture("0.1.0").unwrap()
}
fn candidate_files() -> (rubix_assets::ReleasePackageManifest, tempfile::TempDir) {
    // Deliberately synthetic bytes test identity checks, not archive/OCI layouts.
    let manifest = PlatformSoakRunner::synthetic_candidate_manifest("0.1.0");
    let directory = tempfile::tempdir().unwrap();
    for archive in &manifest.node_archives {
        std::fs::write(
            directory.path().join(&archive.filename),
            archive.filename.as_bytes(),
        )
        .unwrap();
    }
    for binary in &manifest.management_binaries {
        std::fs::write(
            directory.path().join(&binary.filename),
            binary.filename.as_bytes(),
        )
        .unwrap();
    }
    for image in &manifest.oci_images {
        let root = directory.path().join("oci").join(&image.asset_id);
        std::fs::create_dir_all(root.join("sha256")).unwrap();
        std::fs::write(root.join("index.json"), format!("index-{}", image.asset_id)).unwrap();
        for descriptor in &image.platforms {
            std::fs::write(
                root.join("sha256")
                    .join(format!("{}.json", &descriptor.digest[7..])),
                format!("{}-{}", image.asset_id, descriptor.platform),
            )
            .unwrap();
        }
    }
    (manifest, directory)
}

#[test]
fn fixtures_round_trip_but_never_qualify_even_with_relabelled_evidence() {
    let report = fixture();
    assert!(!report.overall_qualified);
    assert_eq!(
        report.candidate_verification,
        CandidateVerificationSummary::unobserved()
    );
    report.validate_fixture(Some("0.1.0")).unwrap();
    assert!(report.validate(Some("0.1.0")).is_err());
    assert!(
        PlatformSoakRunner::new()
            .run_qualification("0.1.0", None)
            .is_err()
    );
    let manifest = PlatformSoakRunner::synthetic_candidate_manifest("0.1.0");
    assert!(
        PlatformSoakRunner::new()
            .run_qualification("0.1.0", Some(&manifest))
            .is_err()
    );
    let mut forged = report.clone();
    forged.evidence_kind = "LiveLinux".into();
    forged.overall_qualified = true;
    assert!(forged.validate(None).is_err());
    assert!(forged.to_markdown().contains("Invalid Platform Fixture"));
    let decoded: PlatformSoakReport =
        serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
    assert_eq!(decoded, report);
    assert!(decoded.to_markdown().contains("NOT QUALIFIED"));
    assert!(!decoded.to_markdown().contains("PASS (Qualified)"));
}

#[test]
fn every_matrix_dimension_rejects_missing_duplicate_extra_and_false_results() {
    let canonical = EnvironmentMapping::canonical_matrix();
    MatrixCompleteness::validate(&canonical).unwrap();
    for index in 0..canonical.len() {
        let mut changed = canonical.clone();
        changed.remove(index);
        assert!(MatrixCompleteness::validate(&changed).is_err());
        let mut changed = canonical.clone();
        changed.push(canonical[index].clone());
        assert!(MatrixCompleteness::validate(&changed).is_err());
    }
    let mut changed = canonical.clone();
    changed[0].id = "unknown".into();
    assert!(MatrixCompleteness::validate(&changed).is_err());
    let mut changed = canonical;
    changed[0].result = rubix_dev::platform_soak::EnvironmentResult::Verified;
    assert!(MatrixCompleteness::validate(&changed).is_err());
}

#[test]
fn candidate_verification_requires_actual_complete_unique_bytes_and_independent_version() {
    let (manifest, files) = candidate_files();
    let summary =
        CandidateVerificationSummary::verify_files(&manifest, files.path(), "0.1.0").unwrap();
    assert_eq!(summary.total_artifacts, 52); // 20 distributions, 7 indexes, 25 platform descriptors
    assert_eq!(summary.total_artifacts, summary.matched_artifacts);
    assert!(CandidateVerificationSummary::verify(&manifest, &[], "0.1.0").is_err());
    assert!(CandidateVerificationSummary::verify_files(&manifest, files.path(), "9.9.9").is_err());
    let name = &manifest.node_archives[0].filename;
    let first = ObservedArtifact::from_file(name, &files.path().join(name)).unwrap();
    let duplicate = ObservedArtifact::from_file(name, &files.path().join(name)).unwrap();
    assert!(
        CandidateVerificationSummary::verify(&manifest, &[first, duplicate], "0.1.0")
            .unwrap_err()
            .contains("duplicate")
    );
    let mut invalid = manifest.clone();
    invalid.schema_version = 99;
    assert!(CandidateVerificationSummary::verify_files(&invalid, files.path(), "0.1.0").is_err());
    let image = &manifest.oci_images[0];
    let path = files
        .path()
        .join("oci")
        .join(&image.asset_id)
        .join("sha256")
        .join(format!("{}.json", &image.platforms[0].digest[7..]));
    std::fs::write(&path, b"different bytes").unwrap();
    assert!(CandidateVerificationSummary::verify_files(&manifest, files.path(), "0.1.0").is_err());
    std::fs::remove_file(path).unwrap();
    assert!(CandidateVerificationSummary::verify_files(&manifest, files.path(), "0.1.0").is_err());
}

#[test]
fn report_recomputes_all_bounds_and_refuses_forged_candidate_summary() {
    let report = fixture();
    let modifications: &[fn(&mut PlatformSoakReport)] = &[
        |r| r.soak_results[0].actual_duration_seconds = 1,
        |r| r.soak_results[0].declared_duration_hours = 1,
        |r| r.soak_results[0].workload_cycles_completed = 0,
        |r| r.soak_results[0].final_settled_idle_rss_bytes *= 2,
        |r| r.soak_results[0].initial_settled_idle_rss_bytes = 0,
        |r| r.soak_results[0].final_settled_idle_rss_bytes = 0,
        |r| r.soak_results[0].derived_growth_ratio = f64::NAN,
        |r| r.soak_results[0].passed = false,
        |r| r.soak_results[0].oom_count = 1,
        |r| r.soak_results[0].crash_count = 1,
        |r| r.soak_results[0].unexplained_probe_failures = 1,
        |r| {
            r.soak_results.remove(0);
        },
        |r| r.soak_results.push(r.soak_results[0].clone()),
        |r| {
            r.restart_results[0].bound_duration_ms = 999_999;
            r.restart_results[0].observed_duration_ms = 999_998;
        },
        |r| r.restart_results[0].state_preserved = false,
        |r| {
            r.restart_results.remove(0);
        },
        |r| r.historical_regressions[0].declared_bound = "anything passes".into(),
        |r| r.historical_regressions[0].passed = false,
        |r| r.epic_regressions[0].qualification_gate = "fake".into(),
        |r| r.epic_regressions[0].passed = false,
        |r| r.candidate_verification.all_matched = true,
        |r| r.candidate_verification.total_artifacts = 20,
        |r| r.schema_version = 1,
        |r| r.product = "foreign".into(),
        |r| r.timestamp = "unix:0".into(),
        |r| r.overall_qualified = true,
    ];
    for modify in modifications {
        let mut changed = report.clone();
        modify(&mut changed);
        assert!(
            changed.validate_fixture(None).is_err(),
            "tampered report accepted"
        );
        assert!(changed.to_markdown().contains("Invalid Platform Fixture"));
    }
    assert!(SustainedSoakSummary::evaluate_record("amd64", 86400, 0, 100, 100, 0, 0, 0).is_err());
    // At this magnitude f64 rounds a one-byte violation back to exactly 1.10.
    let initial = 10_000_000_000_000_000_000;
    let final_rss = 11_000_000_000_000_000_001;
    assert!(
        SustainedSoakSummary::evaluate_record("amd64", 86400, 1, initial, final_rss, 0, 0, 0)
            .is_err()
    );
    let mut rounded = report.clone();
    rounded.soak_results[0].initial_settled_idle_rss_bytes = initial;
    rounded.soak_results[0].final_settled_idle_rss_bytes = final_rss;
    rounded.soak_results[0].derived_growth_ratio = 1.10;
    assert!(rounded.validate_fixture(None).is_err());
    assert!(RestartSummary::validate_cases(&RestartSummary::canonical_cases()).is_ok());
}

#[cfg(unix)]
#[test]
fn observations_reject_symlinks_and_special_files_before_open() {
    let directory = tempfile::tempdir().unwrap();
    let regular = directory.path().join("regular");
    std::fs::write(&regular, b"observed bytes").unwrap();
    let link = directory.path().join("link");
    std::os::unix::fs::symlink(&regular, &link).unwrap();
    assert!(ObservedArtifact::from_file("link", &link).is_err());
    let fifo = directory.path().join("fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert!(ObservedArtifact::from_file("fifo", &fifo).is_err());
    assert!(ObservedArtifact::from_file("directory", directory.path()).is_err());
    assert!(ObservedArtifact::from_file("regular", &regular).is_ok());
}

#[test]
fn cli_separates_fixture_and_qualification_and_requires_artifact_files() {
    let executable = env!("CARGO_BIN_EXE_rubix-platform-soak");
    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(executable).args(["run"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not implemented"));
    let output = Command::new(executable)
        .arg("fixture")
        .arg("--output")
        .arg(directory.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let report = directory.path().join("platform-soak-report.json");
    assert!(
        !Command::new(executable)
            .arg("verify")
            .arg(&report)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(executable)
            .arg("verify-fixture")
            .arg(&report)
            .status()
            .unwrap()
            .success()
    );
    let (manifest, files) = candidate_files();
    let manifest_path = directory.path().join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let output = Command::new(executable)
        .arg("verify-candidate")
        .arg(&manifest_path)
        .arg(files.path())
        .args(["--expected-version", "0.1.0"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: CandidateVerificationSummary = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary.total_artifacts, 52);
    assert!(
        !Command::new(executable)
            .arg("verify-candidate")
            .arg(&manifest_path)
            .arg(directory.path())
            .args(["--expected-version", "0.1.0"])
            .status()
            .unwrap()
            .success()
    );
}
