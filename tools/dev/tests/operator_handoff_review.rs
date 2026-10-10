//! Verification of Candidate-Bound Operator Handoff Rehearsal and Criterion 11 qualification.

use std::fs;
use std::path::Path;

use rubix_dev::operator_rehearsal::{
    CRITERION_NUMBER, RECEIPT_FILENAME, REPORT_JSON_FILENAME, REPORT_MD_FILENAME,
    capture_operator_rehearsal, verify_operator_rehearsal_receipt,
};
use rubix_dev::release_qualification::criteria::check_criterion_11_operator_handoff;
use rubix_dev::release_qualification::receipt::{CandidateReceipt, ReceiptPayload, SkipRecord};
use rubix_dev::repository_root;
use rubix_dev::sha256;

#[test]
fn clean_checkout_evaluates_criterion_11_as_pending() -> rubix_dev::Result<()> {
    let root = repository_root(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let status = check_criterion_11_operator_handoff(&root)?;
    assert!(
        !status.satisfied,
        "Criterion 11 must be pending on clean checkout"
    );
    assert_eq!(status.number, CRITERION_NUMBER);
    assert_eq!(
        status.name,
        "Operator Documentation & Release Qualification"
    );
    assert_eq!(
        status.summary,
        "Pending: missing receipt 'criterion-11-operator-handoff.json'; \
         fresh operator rehearsal, exact artifacts and trusted publication; \
         validated current candidate-bound receipts unavailable"
    );
    Ok(())
}

#[tokio::test]
async fn operator_rehearsal_capture_and_verification() -> rubix_dev::Result<()> {
    let root = repository_root(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;

    let (receipt_path, report_json_path, report_md_path) =
        capture_operator_rehearsal(temp.path(), &root).await?;

    assert!(receipt_path.is_file(), "receipt file must exist");
    assert!(report_json_path.is_file(), "report JSON must exist");
    assert!(report_md_path.is_file(), "report MD must exist");

    assert_eq!(
        receipt_path.file_name().and_then(|s| s.to_str()),
        Some(RECEIPT_FILENAME)
    );
    assert_eq!(
        report_json_path.file_name().and_then(|s| s.to_str()),
        Some(REPORT_JSON_FILENAME)
    );
    assert_eq!(
        report_md_path.file_name().and_then(|s| s.to_str()),
        Some(REPORT_MD_FILENAME)
    );

    let expected_logs = [
        "01_preflight.log",
        "02_pki_bootstrap.log",
        "03_kubeconfig_yaml.log",
        "04_kubeconfig_json.log",
        "05_mutate_workload.log",
        "06_reconcile_storage.log",
        "07_verify_io.log",
        "08_metrics_probe.log",
        "09_metrics_prometheus.log",
        "10_metrics_openmetrics.log",
        "11_metrics_negotiation.log",
        "12_state_transition_matrix.log",
        "13_sqlite_rejection.log",
        "14_wal_integrity.log",
        "15_validate_data_path.log",
        "16_reset_dry_run.log",
        "17_reset_execute.log",
        "18_verify_isolation.log",
    ];

    for log_name in &expected_logs {
        let log_path = temp.path().join(log_name);
        assert!(log_path.is_file(), "missing expected log: {log_name}");
        let meta = fs::metadata(&log_path).map_err(|e| e.to_string())?;
        assert!(meta.len() > 0, "log file must not be empty: {log_name}");
    }

    let receipt = verify_operator_rehearsal_receipt(&receipt_path, &root)?;
    assert_eq!(receipt.criterion, CRITERION_NUMBER);
    assert!(
        receipt.skips.is_empty(),
        "criterion 11 must have zero skips"
    );
    assert_eq!(receipt.assertions.len(), 5);
    for assertion in &receipt.assertions {
        assert!(assertion.passed, "assertion {} must pass", assertion.name);
        if let Some(detail) = &assertion.detail {
            assert!(
                !detail.to_lowercase().contains("non-qualifying"),
                "assertion {} contains non-qualifying",
                assertion.name
            );
        }
    }
    assert_eq!(receipt.commands.len(), 18);
    for cmd in &receipt.commands {
        assert_eq!(cmd.exit_code, 0, "command {:?} must exit 0", cmd.command);
    }

    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn tampered_receipt_fails_closed() -> rubix_dev::Result<()> {
    let root = repository_root(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;

    let (receipt_path, _, _) = capture_operator_rehearsal(temp.path(), &root).await?;
    let content = fs::read_to_string(&receipt_path).map_err(|e| e.to_string())?;
    let receipt: CandidateReceipt = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    // Tampering 1: Hash mismatch
    let tampered_path_1 = temp.path().join("tampered_hash.json");
    let mut bad_receipt = receipt.clone();
    bad_receipt.integrity_hash =
        "0000000000000000000000000000000000000000000000000000000000000000".to_string();
    fs::write(
        &tampered_path_1,
        serde_json::to_string_pretty(&bad_receipt).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    assert!(
        verify_operator_rehearsal_receipt(&tampered_path_1, &root).is_err(),
        "receipt with forged integrity hash must fail verification"
    );

    // Tampering 2: Inject a skip record with validly recalculated hash
    let tampered_path_2 = temp.path().join("tampered_skip.json");
    let payload = ReceiptPayload {
        schema_version: receipt.schema_version,
        criterion: receipt.criterion,
        description: receipt.description.clone(),
        candidate: receipt.candidate.clone(),
        environment: receipt.environment.clone(),
        commands: receipt.commands.clone(),
        assertions: receipt.assertions.clone(),
        skips: vec![SkipRecord {
            name: "skipped_step".to_string(),
            reason: "justification".to_string(),
        }],
        cleanup: receipt.cleanup.clone(),
        timestamps: receipt.timestamps.clone(),
    };
    let canonical = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let hash = sha256(&canonical);
    let tampered_with_skip = CandidateReceipt {
        schema_version: payload.schema_version,
        criterion: payload.criterion,
        description: payload.description,
        candidate: payload.candidate,
        environment: payload.environment,
        commands: payload.commands,
        assertions: payload.assertions,
        skips: payload.skips,
        cleanup: payload.cleanup,
        timestamps: payload.timestamps,
        integrity_hash: hash,
    };
    fs::write(
        &tampered_path_2,
        serde_json::to_string_pretty(&tampered_with_skip).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let res = verify_operator_rehearsal_receipt(&tampered_path_2, &root);
    assert!(
        res.is_err(),
        "receipt with injected skip must fail verification"
    );
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("documented skips"),
        "error message should cite documented skips, got: {err_msg}"
    );

    // Tampering 3: Non-qualifying detail in assertion with valid hash
    let tampered_path_3 = temp.path().join("tampered_detail.json");
    let mut payload3 = ReceiptPayload {
        schema_version: receipt.schema_version,
        criterion: receipt.criterion,
        description: receipt.description.clone(),
        candidate: receipt.candidate.clone(),
        environment: receipt.environment.clone(),
        commands: receipt.commands.clone(),
        assertions: receipt.assertions.clone(),
        skips: vec![],
        cleanup: receipt.cleanup.clone(),
        timestamps: receipt.timestamps.clone(),
    };
    if let Some(first_assertion) = payload3.assertions.first_mut() {
        first_assertion.detail = Some("non-qualifying run on dev host".to_string());
    }
    let canonical3 = serde_json::to_vec(&payload3).map_err(|e| e.to_string())?;
    let hash3 = sha256(&canonical3);
    let tampered_with_detail = CandidateReceipt {
        schema_version: payload3.schema_version,
        criterion: payload3.criterion,
        description: payload3.description,
        candidate: payload3.candidate,
        environment: payload3.environment,
        commands: payload3.commands,
        assertions: payload3.assertions,
        skips: payload3.skips,
        cleanup: payload3.cleanup,
        timestamps: payload3.timestamps,
        integrity_hash: hash3,
    };
    fs::write(
        &tampered_path_3,
        serde_json::to_string_pretty(&tampered_with_detail).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let res3 = verify_operator_rehearsal_receipt(&tampered_path_3, &root);
    assert!(
        res3.is_err(),
        "receipt with non-qualifying assertion detail must fail verification"
    );

    Ok(())
}
