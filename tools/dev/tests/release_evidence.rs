//! Regression coverage for fixture/qualification separation and evidence tampering.
use rubix_dev::{release::*, repository_root};
use std::{fs, path::Path};
fn root() -> std::path::PathBuf {
    repository_root(Path::new(".")).unwrap()
}
fn rehash(dir: &Path) {
    fs::write(
        dir.join("SHA256SUMS"),
        assemble_checksum_manifest(dir).unwrap(),
    )
    .unwrap();
}
async fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assemble_fixture_evidence(&root(), dir.path())
        .await
        .unwrap();
    dir
}
#[test]
fn committed_diagnostics_are_unqualified() {
    verify_fixture_evidence(&root(), &root().join("docs/release")).unwrap();
    assert!(verify_release_evidence(&root().join("docs/release")).is_err());
}
#[tokio::test]
async fn production_assembly_refuses_without_mutating_output() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        assemble_release_evidence(&root(), dir.path())
            .await
            .is_err()
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}
#[tokio::test]
async fn arbitrary_pass_flags_are_rejected_after_rehash() {
    let dir = fixture().await;
    let file = dir.path().join("performance-qualification-report.json");
    let mut report: PerfQualificationDocument =
        serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    for gate in &mut report.amd64_evaluation.results {
        gate.name = "duplicated".into();
        gate.candidate_value = 1e20;
        gate.target_threshold = 0.;
        gate.passed = true;
    }
    fs::write(file, serde_json::to_vec(&report).unwrap()).unwrap();
    rehash(dir.path());
    assert!(
        verify_fixture_evidence(&root(), dir.path())
            .unwrap_err()
            .to_string()
            .contains("recomputed")
    );
}
#[tokio::test]
async fn missing_state_assertions_cannot_claim_success() {
    let dir = fixture().await;
    let file = dir
        .path()
        .join("state-transition-qualification-report.json");
    fs::write(file, r#"{"schema_version":2,"status":"UNQUALIFIED_FIXTURE_ONLY","observed_transitions":[],"qualified":true,"note":""}"#).unwrap();
    rehash(dir.path());
    assert!(
        verify_fixture_evidence(&root(), dir.path())
            .unwrap_err()
            .to_string()
            .contains("state transitions")
    );
}
#[tokio::test]
async fn published_license_inventory_and_notices_checked() {
    let dir = fixture().await;
    let valid_inventory = fs::read(dir.path().join("licenses.json")).unwrap();
    fs::write(dir.path().join("licenses.json"), "{}").unwrap();
    rehash(dir.path());
    assert!(verify_fixture_evidence(&root(), dir.path()).is_err());
    fs::write(dir.path().join("licenses.json"), valid_inventory).unwrap();
    fs::write(dir.path().join("attribution.md"), "").unwrap();
    rehash(dir.path());
    assert!(
        verify_fixture_evidence(&root(), dir.path())
            .unwrap_err()
            .to_string()
            .contains("attribution")
    );
}
#[tokio::test]
async fn omitted_mandatory_checksums_rejected() {
    let dir = fixture().await;
    let file = dir.path().join("SHA256SUMS");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        file,
        text.lines()
            .filter(|line| !line.ends_with("licenses.json"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    assert!(
        verify_fixture_evidence(&root(), dir.path())
            .unwrap_err()
            .to_string()
            .contains("inventory")
    );
}
#[tokio::test]
async fn unexpected_file_requires_checksum_coverage() {
    let dir = fixture().await;
    fs::write(dir.path().join("unbound.json"), "{}").unwrap();
    assert!(verify_fixture_evidence(&root(), dir.path()).is_err());
}
#[tokio::test]
async fn source_observations_must_remain_valid() {
    let dir = fixture().await;
    fs::write(dir.path().join("amd64-reference-go.json"), "{}").unwrap();
    rehash(dir.path());
    assert!(verify_fixture_evidence(&root(), dir.path()).is_err());
}
#[test]
fn seeded_manifest_cannot_verify_missing_artifacts() {
    let manifest =
        rubix_dev::platform_soak::PlatformSoakRunner::synthetic_candidate_manifest("0.1.0");
    assert!(verify_observed_artifacts(tempfile::tempdir().unwrap().path(), &manifest).is_err());
    // Production rejects even an entirely empty directory, independently of manifest status.
    assert!(verify_release_evidence(tempfile::tempdir().unwrap().path()).is_err());
}
#[cfg(unix)]
#[test]
fn symlink_checksum_inputs_rejected() {
    let dir = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/etc/passwd", dir.path().join("linked")).unwrap();
    assert!(assemble_checksum_manifest(dir.path()).is_err());
}
#[test]
fn yaml_json_parser_equivalence() {
    verify_kubeconfig_dual_format_accommodation().unwrap();
}
#[test]
fn production_cli_never_reports_fixture_as_qualified() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rubix-release"))
        .args(["verify", root().join("docs/release").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("qualification unavailable")
    );
}
#[test]
fn nested_oci_files_are_bound_and_tampering_detected() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("oci/image/sha256")).unwrap();
    let nested = dir.path().join("oci/image/sha256/descriptor.json");
    fs::write(&nested, "descriptor bytes").unwrap();
    let checksums = assemble_checksum_manifest(dir.path()).unwrap();
    verify_checksum_inventory(&checksums, dir.path()).unwrap();
    fs::write(nested, "changed bytes").unwrap();
    assert!(
        verify_checksum_inventory(&checksums, dir.path())
            .unwrap_err()
            .to_string()
            .contains("checksum mismatch")
    );
    assert!(
        verify_checksum_inventory(&format!("{}  ../outside\n", "0".repeat(64)), dir.path())
            .is_err()
    );
}
#[cfg(unix)]
#[test]
fn backslash_filename_cannot_alias_an_oci_path() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("oci")).unwrap();
    fs::write(dir.path().join("oci/index.json"), "bound bytes").unwrap();
    fs::write(dir.path().join("oci\\index.json"), "unbound alias bytes").unwrap();
    assert!(assemble_checksum_manifest(dir.path()).is_err());
}
#[tokio::test]
async fn fixture_overview_cannot_add_qualification_claims_after_rehash() {
    let dir = fixture().await;
    let file = dir.path().join("README.md");
    let mut text = fs::read_to_string(&file).unwrap();
    text.push_str("Production migration and all platforms qualified.\n");
    fs::write(file, text).unwrap();
    rehash(dir.path());
    assert!(
        verify_fixture_evidence(&root(), dir.path())
            .unwrap_err()
            .to_string()
            .contains("canonical unqualified")
    );
}
#[test]
fn workspace_and_registry_sources_follow_cargo_metadata() {
    let metadata = serde_json::json!({"workspace_members":["local"], "packages":[
        {"id":"local","name":"rubix-assets","version":"0.1.0","license":"Apache-2.0","source":null},
        {"id":"patched","name":"patched-parser","version":"1.0.0","license":"MIT","source":null},
        {"id":"remote","name":"remote-crate","version":"1.0.0","license":"MIT","source":"registry+https://github.com/rust-lang/crates.io-index"}
    ]});
    let record = AttributionRecord::build(&metadata.to_string()).unwrap();
    for (name, expected) in [
        ("rubix-assets", "workspace path crate"),
        ("patched-parser", "local path dependency"),
        (
            "remote-crate",
            "registry+https://github.com/rust-lang/crates.io-index",
        ),
    ] {
        assert_eq!(
            record
                .components
                .iter()
                .find(|entry| entry.name == name)
                .unwrap()
                .upstream_repository,
            expected
        );
    }
}
#[tokio::test]
async fn conformance_exclusions_never_claim_live_concurrency_qualification() {
    let report = rubix_dev::conformance::QualificationRunner::new()
        .run_fixture()
        .await
        .unwrap();
    let text = report.to_json().unwrap();
    assert!(text.contains("no live single-node concurrency is qualified"));
    assert!(!text.contains("Single-node concurrency is qualified via"));
    report.verify_fixture().unwrap();
}
