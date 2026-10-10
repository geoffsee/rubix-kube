//! Integration tests for recovery rehearsal qualification and Criterion 5 receipts (Issue #352 / E36.02).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rubix_dev::recovery_rehearsal::{
    CRITERION_NUMBER, RECEIPT_FILENAME, REPORT_FILENAME, capture_recovery_qualification,
    verify_recovery_receipt,
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
        description: "Lifecycle & State Retention Qualification (Criterion 5)".into(),
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
            duration_seconds: Some(120),
        },
        commands: vec![CommandExecution {
            command: vec!["rubix-recovery-rehearsal".into(), "capture".into()],
            exit_code: 0,
            stdout_sha256: None,
            stderr_sha256: None,
            duration_ms: Some(120_000),
        }],
        assertions: vec![
            ReceiptAssertionRecord {
                name: "crash_restart_state_retention".into(),
                passed: true,
                detail: Some("SIGKILL restart verified on live subprocesses".into()),
            },
            ReceiptAssertionRecord {
                name: "bounded_escalation_and_cleanup".into(),
                passed: true,
                detail: Some("escalation completed within 5s".into()),
            },
            ReceiptAssertionRecord {
                name: "datastore_outage_blocking_r2".into(),
                passed: true,
                detail: Some("datastore outage blocked degraded execution".into()),
            },
            ReceiptAssertionRecord {
                name: "reboot_state_retention".into(),
                passed: true,
                detail: Some("reboot state retention verified".into()),
            },
            ReceiptAssertionRecord {
                name: "wal_torn_write_fails_closed".into(),
                passed: true,
                detail: Some("wal torn write failed closed".into()),
            },
            ReceiptAssertionRecord {
                name: "startup_interruption_safe_reentry".into(),
                passed: true,
                detail: Some("interrupted startup safe re-entry".into()),
            },
            ReceiptAssertionRecord {
                name: "ownership_cleanup_isolation".into(),
                passed: true,
                detail: Some("ownership cleanup isolated".into()),
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
            completed_at: "2026-10-08T00:02:00Z".into(),
        },
    }
}

#[test]
fn test_recovery_rehearsal_capture_and_rehearsal_fail_closed() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let out_dir = temp_dir.path().join("rec_out");

    let (receipt_path, report_path) =
        capture_recovery_qualification(&out_dir, &root).expect("capture recovery qualification");

    assert!(receipt_path.is_file(), "receipt file must exist");
    assert!(report_path.is_file(), "report file must exist");
    assert_eq!(
        receipt_path.file_name().unwrap().to_str().unwrap(),
        RECEIPT_FILENAME
    );
    assert_eq!(
        report_path.file_name().unwrap().to_str().unwrap(),
        REPORT_FILENAME
    );

    // Verify report contents
    let report_content = fs::read_to_string(&report_path).expect("read report");
    let report_json: serde_json::Value =
        serde_json::from_str(&report_content).expect("parse report json");
    assert_eq!(report_json["criterion"], CRITERION_NUMBER);
    assert_eq!(report_json["assertions"].as_array().unwrap().len(), 7);

    // Schema and candidate inventory validation succeeds for rehearsal receipt
    let loaded = load_and_validate_receipt(&receipt_path, &root, CRITERION_NUMBER)
        .expect("schema and candidate digest validation");
    assert_eq!(loaded.criterion, CRITERION_NUMBER);
    assert_eq!(loaded.assertions.len(), 7);

    // Qualification verification MUST FAIL CLOSED on in-process rehearsal receipt
    let verify_err = verify_recovery_receipt(&receipt_path, &root)
        .expect_err("rehearsal receipt must fail closed for qualification");
    assert!(
        verify_err.to_string().contains("qualification rejected"),
        "error must explain qualification rejection: {verify_err}"
    );

    // Verification via directory path must also fail closed
    let dir_err = verify_recovery_receipt(&out_dir, &root)
        .expect_err("rehearsal dir must fail closed for qualification");
    assert!(dir_err.to_string().contains("qualification rejected"));
}

#[test]
fn test_recovery_rehearsal_qualifying_receipt_passes_verification() {
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

    let verified =
        verify_recovery_receipt(&receipt_path, &root).expect("verify qualifying receipt");
    assert_eq!(verified.criterion, CRITERION_NUMBER);
    assert_eq!(verified.assertions.len(), 7);
    assert!(verified.skips.is_empty());
}

#[test]
fn test_recovery_rehearsal_rejection_non_linux() {
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

    let err = verify_recovery_receipt(&receipt_path, &root).expect_err("must reject non-Linux");
    assert!(
        err.to_string().contains("non-Linux execution environment"),
        "error: {err}"
    );
}

#[test]
fn test_recovery_rehearsal_rejection_in_process_mode() {
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

    let err = verify_recovery_receipt(&receipt_path, &root).expect_err("must reject in-process");
    assert!(
        err.to_string()
            .contains("execution mode 'in_process' does not qualify"),
        "error: {err}"
    );
}

#[test]
fn test_recovery_rehearsal_rejection_unpermitted_skips() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let receipt_path = temp_dir.path().join("skipped_recovery_receipt.json");

    let mut payload = qualifying_sample_payload(&root);
    payload.skips.push(SkipRecord {
        name: "physical_host_reboot".into(),
        reason: "skipped".into(),
    });
    let receipt = CandidateReceipt::new_with_integrity_hash(payload).expect("sign receipt");
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&receipt).expect("serialize"),
    )
    .expect("write");

    let err =
        verify_recovery_receipt(&receipt_path, &root).expect_err("must reject unpermitted skips");
    assert!(err.to_string().contains("documented skips"), "error: {err}");
}

#[test]
fn test_recovery_rehearsal_rejection_non_qualifying_detail() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let receipt_path = temp_dir.path().join("non_qualifying_detail_receipt.json");

    let mut payload = qualifying_sample_payload(&root);
    payload.assertions[0].detail = Some("non-qualifying in-process rehearsal".into());
    let receipt = CandidateReceipt::new_with_integrity_hash(payload).expect("sign receipt");
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&receipt).expect("serialize"),
    )
    .expect("write");

    let err = verify_recovery_receipt(&receipt_path, &root)
        .expect_err("must reject non-qualifying measurement");
    assert!(
        err.to_string().contains("non-qualifying measurement"),
        "error: {err}"
    );
}

#[test]
fn test_recovery_rehearsal_tamper_rejection() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let out_dir = temp_dir.path().join("rec_tamper");

    let (receipt_path, _) =
        capture_recovery_qualification(&out_dir, &root).expect("capture recovery qualification");

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
    let verify_res = verify_recovery_receipt(&tampered_path, &root);
    assert!(
        verify_res.is_err(),
        "tampered receipt must fail verification"
    );
}

#[test]
fn test_recovery_rehearsal_cli_execution() {
    let root = repo_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let out_dir = temp_dir.path().join("cli_rec");

    // Test capture CLI
    let capture_status = Command::new(env!("CARGO_BIN_EXE_rubix-recovery-rehearsal"))
        .arg("capture")
        .arg("--output")
        .arg(&out_dir)
        .current_dir(&root)
        .status()
        .expect("execute capture command");
    assert!(capture_status.success(), "capture command must succeed");

    let receipt_path = out_dir.join(RECEIPT_FILENAME);
    assert!(receipt_path.is_file(), "receipt must be generated by CLI");

    // Test verify CLI fails closed on rehearsal receipt
    let verify_rehearsal_status = Command::new(env!("CARGO_BIN_EXE_rubix-recovery-rehearsal"))
        .arg("verify")
        .arg(&receipt_path)
        .current_dir(&root)
        .status()
        .expect("execute verify command");
    assert!(
        !verify_rehearsal_status.success(),
        "verify command must fail closed on rehearsal receipt"
    );

    // Test verify CLI succeeds on qualifying receipt
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

    let verify_qual_status = Command::new(env!("CARGO_BIN_EXE_rubix-recovery-rehearsal"))
        .arg("verify")
        .arg(&qual_receipt_path)
        .current_dir(&root)
        .status()
        .expect("execute verify command on qual receipt");
    assert!(
        verify_qual_status.success(),
        "verify command must succeed on qualifying receipt"
    );

    // Test migration command fallback
    let migration_status = Command::new(env!("CARGO_BIN_EXE_rubix-recovery-rehearsal"))
        .arg("migration")
        .current_dir(&root)
        .status()
        .expect("execute migration command");
    assert!(
        migration_status.success(),
        "migration fallback must succeed"
    );
}
