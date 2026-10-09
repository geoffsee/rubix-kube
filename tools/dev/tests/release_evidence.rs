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
fn distribution_attribution_matches_repository_license() {
    let metadata = serde_json::json!({"workspace_members":[], "packages":[]});
    let record = AttributionRecord::build(&metadata.to_string()).unwrap();
    assert_eq!(record.distribution_license, "ISC");
    assert_eq!(
        record.license_texts["ISC"],
        fs::read_to_string(root().join("LICENSE")).unwrap()
    );
    let kubernetes = record
        .components
        .iter()
        .find(|component| component.name == "kube-apiserver")
        .unwrap();
    assert_eq!(kubernetes.spdx_license, "Apache-2.0");
    assert!(record.to_markdown().contains("UNQUALIFIED_FIXTURE_ONLY"));
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
    for unsupported_claim in ["multiple hours", "multi-hour", "1000+", "1,000+"] {
        assert!(!text.contains(unsupported_claim));
    }
    report.verify_fixture().unwrap();
}

#[test]
fn committed_cell_inventory_records_all_20_receipts() {
    let inventory_file = root().join("docs/release/cell-inventory.json");
    let inventory: CellInventory =
        serde_json::from_slice(&fs::read(&inventory_file).unwrap()).unwrap();
    assert_eq!(inventory.node_cells.len(), 16);
    assert_eq!(inventory.management_targets.len(), 4);
    assert_eq!(inventory.host_target, "aarch64-unknown-linux-gnu");

    // Verify cell 6 (arm64/glibc offline) is built
    let cell6 = inventory
        .node_cells
        .iter()
        .find(|c| c.cell == Some(6))
        .unwrap();
    assert_eq!(cell6.status, "built");
    assert_eq!(
        cell6.target_name,
        "kubesolo-0.1.0-linux-arm64-offline.tar.gz"
    );
    assert!(cell6.reason.is_none());
    assert!(cell6.output.is_some());
    assert_eq!(cell6.inputs.len(), 2);

    // Verify the remaining 15 foreign cells have unbuildable reason
    for c in &inventory.node_cells {
        if c.cell != Some(6) {
            assert_eq!(c.status, "unbuildable_foreign_target");
            assert!(c.reason.as_ref().is_some_and(|r| !r.trim().is_empty()));
            assert!(c.output.is_none());
        }
    }

    // Verify native linux-arm64 management target is built
    let linux_arm64 = inventory
        .management_targets
        .iter()
        .find(|t| t.target_name == "rubixctl-linux-arm64")
        .unwrap();
    assert_eq!(linux_arm64.status, "built");
    assert!(linux_arm64.reason.is_none());
    assert!(linux_arm64.output.is_some());

    // Verify the other 3 foreign management targets have unbuildable reason
    for t in &inventory.management_targets {
        if t.target_name != "rubixctl-linux-arm64" {
            assert_eq!(t.status, "unbuildable_foreign_target");
            assert!(t.reason.as_ref().is_some_and(|r| !r.trim().is_empty()));
            assert!(t.output.is_none());
        }
    }

    verify_cell_inventory(&inventory).unwrap();
}

#[test]
fn committed_cleanup_receipt_is_valid() {
    let cleanup_file = root().join("docs/release/cleanup-receipt.json");
    let cleanup: CleanupReceipt =
        serde_json::from_slice(&fs::read(&cleanup_file).unwrap()).unwrap();
    assert_eq!(cleanup.status, "complete");
    assert_eq!(
        cleanup.target_name,
        "kubesolo-0.1.0-linux-arm64-offline.tar.gz"
    );
    assert!(!cleanup.verified_files.is_empty());
    assert!(!cleanup.cleaned_paths.is_empty());
    verify_cleanup_receipt(&cleanup).unwrap();
}

#[tokio::test]
async fn cell_inventory_tampering_is_detected() {
    let dir = fixture().await;
    let file = dir.path().join("cell-inventory.json");
    let mut inventory: CellInventory = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    // Tamper with cell 6 status
    let cell6 = inventory
        .node_cells
        .iter_mut()
        .find(|c| c.cell == Some(6))
        .unwrap();
    cell6.status = "unbuildable_foreign_target".into();
    cell6.reason = Some("fake reason".into());
    fs::write(&file, serde_json::to_vec_pretty(&inventory).unwrap()).unwrap();
    rehash(dir.path());

    // verify_fixture_evidence should fail with cell validation error
    let err = verify_fixture_evidence(&root(), dir.path()).unwrap_err();
    assert!(err.to_string().contains("cell 6 status must be 'built'"));

    // verify_release_evidence should also catch it before returning the fail-closed error
    let err_release = verify_release_evidence(dir.path()).unwrap_err();
    assert!(
        err_release
            .to_string()
            .contains("cell 6 status must be 'built'")
    );
}

#[tokio::test]
async fn cleanup_receipt_tampering_is_detected() {
    let dir = fixture().await;
    let file = dir.path().join("cleanup-receipt.json");
    let mut cleanup: CleanupReceipt = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    cleanup.status = "incomplete".into();
    fs::write(&file, serde_json::to_vec_pretty(&cleanup).unwrap()).unwrap();
    rehash(dir.path());

    let err = verify_fixture_evidence(&root(), dir.path()).unwrap_err();
    assert!(
        err.to_string()
            .contains("cleanup receipt status must be 'complete'")
    );

    let err_release = verify_release_evidence(dir.path()).unwrap_err();
    assert!(
        err_release
            .to_string()
            .contains("cleanup receipt status must be 'complete'")
    );
}

#[test]
fn receipts_directory_is_covered_by_checksum_manifest() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("receipts")).unwrap();
    let receipt = dir.path().join("receipts/criterion-01-epic-ledgers.json");
    fs::write(&receipt, "receipt payload bytes").unwrap();
    let checksums = assemble_checksum_manifest(dir.path()).unwrap();
    assert!(checksums.contains("receipts/criterion-01-epic-ledgers.json"));
    verify_checksum_inventory(&checksums, dir.path()).unwrap();
    fs::write(&receipt, "tampered receipt payload bytes").unwrap();
    let err = verify_checksum_inventory(&checksums, dir.path()).unwrap_err();
    assert!(err.to_string().contains("checksum mismatch"));
}

use rubix_dev::release_qualification::receipt::{
    AssertionRecord, CandidateIdentity, CandidateReceipt, CleanupInventory, CommandExecution,
    EnvironmentInfo, ReceiptPayload, ReceiptTimestamps,
};

fn sample_payload(criterion: usize) -> ReceiptPayload {
    ReceiptPayload {
        schema_version: 1,
        criterion,
        description: format!("Qualification run for criterion {criterion}"),
        candidate: CandidateIdentity {
            source_revision: "4d067c2e97297d42bbcae506820e7ad431328aae".into(),
            binary_digests: std::collections::BTreeMap::new(),
            payload_digests: std::collections::BTreeMap::new(),
        },
        environment: EnvironmentInfo {
            host: "linux-arm64".into(),
            kernel: "6.6.137".into(),
            runner: "github-hosted-ubuntu-24.04-arm".into(),
        },
        commands: vec![CommandExecution {
            command: vec!["rubix-kube".into(), "--check".into()],
            exit_code: 0,
            stdout_sha256: None,
            stderr_sha256: None,
            duration_ms: Some(25),
        }],
        assertions: vec![AssertionRecord {
            name: "service_healthy".into(),
            passed: true,
            detail: Some("verified response 200 OK".into()),
        }],
        skips: vec![],
        cleanup: CleanupInventory {
            cleaned_paths: vec!["/tmp/rubix-test".into()],
            remaining_containers: vec![],
            remaining_images: vec![],
            status: "complete".into(),
        },
        timestamps: ReceiptTimestamps {
            started_at: "2026-10-08T14:00:00Z".into(),
            completed_at: "2026-10-08T14:02:00Z".into(),
        },
    }
}

#[tokio::test]
async fn candidate_byte_tampering_is_detected_across_all_reports() {
    let dir = tempfile::tempdir().unwrap();
    let artifact_name = "rubixctl-linux-arm64";
    let artifact_path = dir.path().join(artifact_name);
    let original_bytes = b"original binary bytes";
    fs::write(&artifact_path, original_bytes).unwrap();
    let observed_digest = rubix_dev::release::sha256_hex(original_bytes);
    let expected_digest = "0000000000000000000000000000000000000000000000000000000000000000";

    let mut payload = sample_payload(6);
    payload
        .candidate
        .binary_digests
        .insert(artifact_name.into(), expected_digest.into());
    let receipt = CandidateReceipt::new_signed(payload).unwrap();

    let reports = [
        "conformance",
        "performance",
        "state transition",
        "soak",
        "attribution",
    ];

    for report_name in reports {
        let err = verify_report_candidate_digests(report_name, &receipt, dir.path()).unwrap_err();
        let expected_msg = format!(
            "digest mismatch in {report_name} for '{artifact_name}': expected '{expected_digest}', observed '{observed_digest}'"
        );
        assert_eq!(err.to_string(), expected_msg);
    }
}

#[tokio::test]
async fn unbuildable_foreign_cell_artifacts_are_rejected() {
    let dir = fixture().await;
    let foreign_target = "rubixctl-linux-amd64";

    // 1. Artifact must not exist as a file in the release directory
    let foreign_file = dir.path().join(foreign_target);
    fs::write(&foreign_file, "foreign target binary").unwrap();
    let err = verify_release_evidence(dir.path()).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!(
            "unbuildable foreign target '{foreign_target}' must not exist in release directory"
        )
    );
    fs::remove_file(&foreign_file).unwrap();

    // 2. Artifact must not exist under bin/ either
    let bin_dir = dir.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let bin_foreign_file = bin_dir.join(foreign_target);
    fs::write(&bin_foreign_file, "foreign target binary").unwrap();
    let err = verify_release_evidence(dir.path()).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!(
            "unbuildable foreign target '{foreign_target}' must not exist in release directory"
        )
    );
    fs::remove_file(&bin_foreign_file).unwrap();

    // 3. Artifact must not appear in SHA256SUMS
    let sums_file = dir.path().join("SHA256SUMS");
    let sums = fs::read_to_string(&sums_file).unwrap();
    let sums = format!(
        "{sums}0000000000000000000000000000000000000000000000000000000000000000  {foreign_target}\n"
    );
    fs::write(&sums_file, sums).unwrap();
    let err = verify_release_evidence(dir.path()).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("unbuildable foreign target '{foreign_target}' must not appear in SHA256SUMS")
    );
}

#[tokio::test]
async fn roundtrip_regenerate_and_verify_release_evidence() {
    let dir = fixture().await;
    let inventory =
        rubix_dev::release_qualification::receipt::load_candidate_inventory(dir.path()).unwrap();
    let candidate = inventory.to_candidate_identity();

    let receipts_dir = dir.path().join("receipts");
    fs::create_dir_all(&receipts_dir).unwrap();
    for (criterion, slug) in [
        (6, "conformance-and-soak"),
        (7, "performance-budgets"),
        (8, "state-migration"),
        (10, "artifact-digest-bindings"),
    ] {
        let mut payload = sample_payload(criterion);
        payload.candidate = candidate.clone();
        let receipt = CandidateReceipt::new_signed(payload).unwrap();
        let filename =
            rubix_dev::release_qualification::criteria::receipt_filename(criterion, slug);
        fs::write(
            receipts_dir.join(filename),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
    }

    // Regenerate qualification reports bound to the candidate receipts
    regenerate_release_reports(&root(), dir.path())
        .await
        .unwrap();

    // Rehash SHA256SUMS to cover regenerated reports and receipts
    rehash(dir.path());

    // verify_release_evidence must succeed!
    verify_release_evidence(dir.path()).unwrap();
}
