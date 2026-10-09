//! Integration tests for platform soak qualification and Criterion 6 receipts (Issue #352 / E36.02).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rubix_dev::platform_soak::qualification::{
    CRITERION_NUMBER, FULL_SOAK_DURATION_SECS, RECEIPT_FILENAME, REPORT_JSON_FILENAME,
    REPORT_MD_FILENAME, capture_soak, verify_soak_receipt,
};
use rubix_dev::release_qualification::receipt::AssertionRecord as ReceiptAssertionRecord;
use rubix_dev::release_qualification::receipt::{
    CURRENT_SCHEMA_VERSION, CandidateIdentity, CandidateReceipt, CleanupInventory,
    CommandExecution, EnvironmentInfo, ReceiptPayload, ReceiptTimestamps, SkipRecord,
    load_and_validate_receipt, load_candidate_inventory,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn qualifying_sample_payload(root: &Path) -> ReceiptPayload {
    let inventory = load_candidate_inventory(root).expect("load candidate inventory");
    ReceiptPayload {
        schema_version: CURRENT_SCHEMA_VERSION,
        criterion: CRITERION_NUMBER,
        description: "Platform Soak & Conformance Qualification (Criterion 6)".into(),
        candidate: CandidateIdentity {
            source_revision: inventory.source_revision,
            binary_digests: inventory.binary_digests,
            payload_digests: inventory.payload_digests,
        },
        environment: EnvironmentInfo {
            host: "linux-x86_64".into(),
            kernel: "6.6.137".into(),
            runner: "linux-baremetal".into(),
            os: Some("linux".into()),
            arch: Some("x86_64".into()),
            execution_mode: Some("live_node".into()),
            duration_seconds: Some(FULL_SOAK_DURATION_SECS),
        },
        commands: vec![CommandExecution {
            command: vec!["rubix-platform-soak".into(), "capture".into()],
            exit_code: 0,
            stdout_sha256: None,
            stderr_sha256: None,
            duration_ms: Some(FULL_SOAK_DURATION_SECS * 1000),
        }],
        assertions: vec![
            ReceiptAssertionRecord {
                name: "soak_memory_growth_bound".into(),
                passed: true,
                detail: Some("initial_rss_bytes: 100000000, final_rss_bytes: 102000000, growth_ratio: 1.020".into()),
            },
            ReceiptAssertionRecord {
                name: "soak_zero_oom_events".into(),
                passed: true,
                detail: Some("0 OOM events detected".into()),
            },
            ReceiptAssertionRecord {
                name: "soak_zero_crashes".into(),
                passed: true,
                detail: Some("0 crashes observed across all cycles".into()),
            },
            ReceiptAssertionRecord {
                name: "soak_zero_unexplained_probe_failures".into(),
                passed: true,
                detail: Some("0 probe failures observed across all cycles".into()),
            },
            ReceiptAssertionRecord {
                name: "soak_workload_cycles_positive".into(),
                passed: true,
                detail: Some("1000 workload cycles completed successfully (attempted: 1000, probe failures: 0)".into()),
            },
        ],
        skips: vec![],
        cleanup: CleanupInventory {
            cleaned_paths: vec!["/tmp/rubix-test".into()],
            remaining_containers: vec![],
            remaining_images: vec![],
            status: "complete".into(),
        },
        timestamps: ReceiptTimestamps {
            started_at: "2026-10-08T00:00:00Z".into(),
            completed_at: "2026-10-09T00:00:00Z".into(),
        },
    }
}

#[tokio::test]
async fn test_platform_soak_capture_and_rehearsal_fail_closed() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let out_dir = temp_dir.path().join("soak_out");

    let (receipt_path, report_json_path, report_md_path) = capture_soak(&out_dir, 2, 3, &root)
        .await
        .expect("capture soak");

    assert!(receipt_path.is_file(), "receipt file must exist");
    assert!(report_json_path.is_file(), "report json file must exist");
    assert!(report_md_path.is_file(), "report md file must exist");
    assert_eq!(
        receipt_path.file_name().unwrap().to_str().unwrap(),
        RECEIPT_FILENAME
    );
    assert_eq!(
        report_json_path.file_name().unwrap().to_str().unwrap(),
        REPORT_JSON_FILENAME
    );
    assert_eq!(
        report_md_path.file_name().unwrap().to_str().unwrap(),
        REPORT_MD_FILENAME
    );

    // Verify report json contents
    let report_content = fs::read_to_string(&report_json_path).expect("read report");
    let report_json: serde_json::Value =
        serde_json::from_str(&report_content).expect("parse report json");
    assert_eq!(report_json["criterion"], CRITERION_NUMBER);
    assert_eq!(report_json["partial"], true);
    assert!(report_json["cycles_completed"].as_u64().unwrap() > 0);
    assert_eq!(report_json["assertions"].as_array().unwrap().len(), 5);

    // Verify report markdown contents
    let md_content = fs::read_to_string(&report_md_path).expect("read md report");
    assert!(md_content.contains("Platform Soak & Conformance Rehearsal Report"));
    assert!(md_content.contains("soak_memory_growth_bound"));
    assert!(md_content.contains("sustained_24h_soak_completion"));

    // Schema validation passes for candidate-bound rehearsal receipt
    let loaded = load_and_validate_receipt(&receipt_path, &root, CRITERION_NUMBER)
        .expect("schema and candidate digest validation");
    assert_eq!(loaded.criterion, CRITERION_NUMBER);
    assert_eq!(loaded.assertions.len(), 5);

    // Qualification verification MUST FAIL CLOSED on in-process rehearsal receipt
    let verify_err = verify_soak_receipt(&receipt_path, &root)
        .expect_err("rehearsal receipt must fail closed for qualification");
    assert!(
        verify_err.to_string().contains("qualification rejected"),
        "error must explain qualification rejection: {verify_err}"
    );

    // Verification via directory path must also fail closed
    let dir_err = verify_soak_receipt(&out_dir, &root)
        .expect_err("rehearsal dir must fail closed for qualification");
    assert!(dir_err.to_string().contains("qualification rejected"));
}

#[test]
fn test_platform_soak_qualifying_receipt_passes_verification() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let receipt_path = temp_dir.path().join(RECEIPT_FILENAME);

    let payload = qualifying_sample_payload(&root);
    let receipt = CandidateReceipt::new_with_integrity_hash(payload).expect("sign receipt");
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&receipt).expect("serialize"),
    )
    .expect("write receipt");

    let verified = verify_soak_receipt(&receipt_path, &root).expect("verify qualifying receipt");
    assert_eq!(verified.criterion, CRITERION_NUMBER);
    assert_eq!(verified.assertions.len(), 5);
    assert!(verified.skips.is_empty());
}

#[test]
fn test_platform_soak_rejection_non_linux() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let receipt_path = temp_dir.path().join("non_linux_receipt.json");

    let mut payload = qualifying_sample_payload(&root);
    payload.environment.host = "darwin-arm64".into();
    payload.environment.os = Some("macos".into());
    let receipt = CandidateReceipt::new_with_integrity_hash(payload).expect("sign receipt");
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&receipt).expect("serialize"),
    )
    .expect("write");

    let err = verify_soak_receipt(&receipt_path, &root).expect_err("must reject non-Linux");
    assert!(
        err.to_string().contains("non-Linux execution environment"),
        "error: {err}"
    );
}

#[test]
fn test_platform_soak_rejection_in_process_mode() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let receipt_path = temp_dir.path().join("in_process_receipt.json");

    let mut payload = qualifying_sample_payload(&root);
    payload.environment.execution_mode = Some("in_process".into());
    let receipt = CandidateReceipt::new_with_integrity_hash(payload).expect("sign receipt");
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&receipt).expect("serialize"),
    )
    .expect("write");

    let err = verify_soak_receipt(&receipt_path, &root).expect_err("must reject in-process");
    assert!(
        err.to_string()
            .contains("execution mode 'in_process' does not qualify"),
        "error: {err}"
    );
}

#[test]
fn test_platform_soak_rejection_duration_short() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let receipt_path = temp_dir.path().join("short_soak_receipt.json");

    let mut payload = qualifying_sample_payload(&root);
    payload.environment.duration_seconds = Some(3600);
    let receipt = CandidateReceipt::new_with_integrity_hash(payload).expect("sign receipt");
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&receipt).expect("serialize"),
    )
    .expect("write");

    let err = verify_soak_receipt(&receipt_path, &root).expect_err("must reject short soak");
    assert!(
        err.to_string().contains("full 24-hour soak required"),
        "error: {err}"
    );
}

#[test]
fn test_platform_soak_rejection_unpermitted_skips() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let receipt_path = temp_dir.path().join("skipped_soak_receipt.json");

    let mut payload = qualifying_sample_payload(&root);
    payload.skips.push(SkipRecord {
        name: "sustained_24h_soak_completion".into(),
        reason: "short run".into(),
    });
    let receipt = CandidateReceipt::new_with_integrity_hash(payload).expect("sign receipt");
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&receipt).expect("serialize"),
    )
    .expect("write");

    let err = verify_soak_receipt(&receipt_path, &root).expect_err("must reject unpermitted skips");
    assert!(err.to_string().contains("documented skips"), "error: {err}");
}

#[test]
fn test_platform_soak_rejection_non_qualifying_detail() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let receipt_path = temp_dir.path().join("non_qualifying_detail_receipt.json");

    let mut payload = qualifying_sample_payload(&root);
    payload.assertions[0].detail =
        Some("measured memory growth: 1.01x (non-qualifying in-process rehearsal)".into());
    let receipt = CandidateReceipt::new_with_integrity_hash(payload).expect("sign receipt");
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&receipt).expect("serialize"),
    )
    .expect("write");

    let err = verify_soak_receipt(&receipt_path, &root)
        .expect_err("must reject non-qualifying measurement");
    assert!(
        err.to_string().contains("non-qualifying measurement"),
        "error: {err}"
    );
}

#[tokio::test]
async fn test_platform_soak_tamper_rejection() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let out_dir = temp_dir.path().join("soak_tamper");

    let (receipt_path, _, _) = capture_soak(&out_dir, 1, 1, &root)
        .await
        .expect("capture soak");

    let raw_bytes = fs::read(&receipt_path).expect("read receipt");
    let mut receipt: CandidateReceipt =
        serde_json::from_slice(&raw_bytes).expect("parse candidate receipt");

    // Invert one assertion's passed status without updating hash
    if let Some(first) = receipt.assertions.first_mut() {
        first.passed = false;
    }
    let tampered_bytes = serde_json::to_vec_pretty(&receipt).expect("serialize tampered receipt");
    let tampered_path = out_dir.join("tampered_receipt.json");
    fs::write(&tampered_path, tampered_bytes).expect("write tampered receipt");

    // Verification must fail closed on tampered receipt
    let verify_res = verify_soak_receipt(&tampered_path, &root);
    assert!(
        verify_res.is_err(),
        "tampered receipt must fail verification"
    );
}

#[test]
fn test_platform_soak_cli_execution() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let out_dir = temp_dir.path().join("cli_soak");

    // Test capture CLI
    let capture_status = Command::new(env!("CARGO_BIN_EXE_rubix-platform-soak"))
        .arg("capture")
        .arg("--output")
        .arg(&out_dir)
        .arg("--duration")
        .arg("1")
        .arg("--cycles")
        .arg("2")
        .current_dir(&root)
        .status()
        .expect("execute capture command");
    assert!(capture_status.success(), "capture command must succeed");

    let receipt_path = out_dir.join(RECEIPT_FILENAME);
    assert!(receipt_path.is_file(), "receipt must be generated by CLI");

    // Test verify-receipt CLI fails closed on rehearsal receipt
    let verify_rehearsal_status = Command::new(env!("CARGO_BIN_EXE_rubix-platform-soak"))
        .arg("verify-receipt")
        .arg(&receipt_path)
        .current_dir(&root)
        .status()
        .expect("execute verify-receipt command");
    assert!(
        !verify_rehearsal_status.success(),
        "verify-receipt command must fail closed on rehearsal receipt"
    );

    // Test verify-receipt CLI succeeds on qualifying receipt
    let qual_dir = temp_dir.path().join("cli_qual");
    fs::create_dir_all(&qual_dir).expect("create qual dir");
    let qual_receipt_path = qual_dir.join(RECEIPT_FILENAME);
    let payload = qualifying_sample_payload(&root);
    let receipt = CandidateReceipt::new_with_integrity_hash(payload).expect("sign receipt");
    fs::write(
        &qual_receipt_path,
        serde_json::to_vec_pretty(&receipt).expect("serialize"),
    )
    .expect("write");

    let verify_qual_status = Command::new(env!("CARGO_BIN_EXE_rubix-platform-soak"))
        .arg("verify-receipt")
        .arg(&qual_receipt_path)
        .current_dir(&root)
        .status()
        .expect("execute verify-receipt command on qual receipt");
    assert!(
        verify_qual_status.success(),
        "verify-receipt command must succeed on qualifying receipt"
    );

    // Test matrix subcommand preservation
    let matrix_status = Command::new(env!("CARGO_BIN_EXE_rubix-platform-soak"))
        .arg("matrix")
        .current_dir(&root)
        .status()
        .expect("execute matrix command");
    assert!(matrix_status.success(), "matrix command must succeed");
}
