//! Integration tests for platform soak qualification and Criterion 6 receipts (Issue #352 / E36.02).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rubix_dev::platform_soak::qualification::{
    CRITERION_NUMBER, RECEIPT_FILENAME, REPORT_JSON_FILENAME, REPORT_MD_FILENAME, capture_soak,
    verify_soak_receipt,
};
use rubix_dev::release_qualification::receipt::CandidateReceipt;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[tokio::test]
async fn test_platform_soak_capture_and_verify_programmatic() {
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
    assert_eq!(report_json["cycles_completed"], 3);
    assert_eq!(report_json["assertions"].as_array().unwrap().len(), 5);

    // Verify report markdown contents
    let md_content = fs::read_to_string(&report_md_path).expect("read md report");
    assert!(md_content.contains("Platform Soak & Conformance Qualification Report"));
    assert!(md_content.contains("soak_memory_growth_bound"));
    assert!(md_content.contains("sustained_24h_soak_completion"));

    // Verify receipt using verification function (file path)
    let receipt = verify_soak_receipt(&receipt_path, &root).expect("verify receipt by file");
    assert_eq!(receipt.criterion, CRITERION_NUMBER);
    assert_eq!(receipt.assertions.len(), 5);
    for a in &receipt.assertions {
        assert!(a.passed, "assertion '{}' must pass", a.name);
    }

    // Verify receipt using directory path
    let receipt_dir = verify_soak_receipt(&out_dir, &root).expect("verify receipt by directory");
    assert_eq!(receipt_dir.criterion, CRITERION_NUMBER);

    // Check required assertions are present
    let names: Vec<_> = receipt.assertions.iter().map(|a| a.name.as_str()).collect();
    assert!(names.contains(&"soak_memory_growth_bound"));
    assert!(names.contains(&"soak_zero_oom_events"));
    assert!(names.contains(&"soak_zero_crashes"));
    assert!(names.contains(&"soak_zero_unexplained_probe_failures"));
    assert!(names.contains(&"soak_workload_cycles_positive"));

    // Check documented skips
    let skip_names: Vec<_> = receipt.skips.iter().map(|s| s.name.as_str()).collect();
    assert!(skip_names.contains(&"sustained_24h_soak_completion"));
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

    // Test verify-receipt CLI with file
    let verify_status = Command::new(env!("CARGO_BIN_EXE_rubix-platform-soak"))
        .arg("verify-receipt")
        .arg(&receipt_path)
        .current_dir(&root)
        .status()
        .expect("execute verify-receipt command");
    assert!(
        verify_status.success(),
        "verify-receipt command must succeed"
    );

    // Test verify-receipt CLI with directory
    let verify_dir_status = Command::new(env!("CARGO_BIN_EXE_rubix-platform-soak"))
        .arg("verify-receipt")
        .arg(&out_dir)
        .current_dir(&root)
        .status()
        .expect("execute verify-receipt directory command");
    assert!(
        verify_dir_status.success(),
        "verify-receipt directory command must succeed"
    );

    // Test matrix subcommand preservation
    let matrix_status = Command::new(env!("CARGO_BIN_EXE_rubix-platform-soak"))
        .arg("matrix")
        .current_dir(&root)
        .status()
        .expect("execute matrix command");
    assert!(matrix_status.success(), "matrix command must succeed");
}
