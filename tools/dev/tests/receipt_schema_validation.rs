//! Integration tests for candidate-bound receipt schema and trusted reader (Issue #348).

use rubix_dev::release_qualification::criteria::{
    self, check_criterion_1_epic_ledgers, verify_all_criteria,
};
use rubix_dev::release_qualification::receipt::{
    self, CandidateIdentity, CandidateInventory, CandidateReceipt, CleanupInventory,
    CommandExecution, EnvironmentInfo, MAX_RECEIPT_BYTES, ReceiptPayload, ReceiptTimestamps,
    compute_payload_integrity_hash,
};
use rubix_dev::release_qualification::{audit_repository_metadata, run_release_qualification};
use rubix_dev::{Result, repository_root};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn root_dir() -> Result<PathBuf> {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn sample_inventory() -> CandidateInventory {
    CandidateInventory {
        source_revision: "2ef1c4787989f11f868f81bb84ae2afd4a49a81d".into(),
        binary_digests: BTreeMap::from([(
            "rubix-kube".into(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
        )]),
        payload_digests: BTreeMap::from([(
            "bundle.manifest".into(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        )]),
    }
}

fn sample_payload(criterion: usize) -> ReceiptPayload {
    ReceiptPayload {
        schema_version: 1,
        criterion,
        description: format!("Qualification run for criterion {criterion}"),
        candidate: CandidateIdentity {
            source_revision: "2ef1c4787989f11f868f81bb84ae2afd4a49a81d".into(),
            binary_digests: BTreeMap::from([(
                "rubix-kube".into(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
            )]),
            payload_digests: BTreeMap::from([(
                "bundle.manifest".into(),
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            )]),
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
        assertions: vec![receipt::AssertionRecord {
            name: "service_healthy".into(),
            passed: true,
            detail: Some("verified response 200 OK".into()),
        }],
        skips: vec![receipt::SkipRecord {
            name: "optional_gpu_check".into(),
            reason: "no GPU available on host".into(),
        }],
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

#[test]
fn test_receipt_acceptance_valid_file() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let receipt_path = temp.path().join("criterion-01-epic-ledgers.json");
    let receipt = CandidateReceipt::new_signed(sample_payload(1))?;
    let json_bytes = serde_json::to_vec_pretty(&receipt)?;
    fs::write(&receipt_path, json_bytes)?;

    let inventory = sample_inventory();
    let loaded = receipt::load_and_validate_receipt_with_inventory(&receipt_path, &inventory, 1)?;
    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.criterion, 1);
    assert!(loaded.verify_integrity().is_ok());
    Ok(())
}

#[test]
fn test_compute_payload_integrity_hash_deterministic() -> Result<()> {
    let payload = sample_payload(1);
    let hash1 = compute_payload_integrity_hash(&payload)?;
    let hash2 = compute_payload_integrity_hash(&payload)?;
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64);
    Ok(())
}

#[cfg(unix)]
#[test]
fn test_receipt_rejection_symlink() -> Result<()> {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir()?;
    let real_receipt_path = temp.path().join("real-receipt.json");
    let receipt = CandidateReceipt::new_signed(sample_payload(1))?;
    fs::write(&real_receipt_path, serde_json::to_vec(&receipt)?)?;

    let symlink_path = temp.path().join("symlink-receipt.json");
    symlink(&real_receipt_path, &symlink_path)?;

    let err = receipt::load_receipt_from_path(&symlink_path).unwrap_err();
    assert!(
        err.to_string().contains("failed to read receipt file")
            || err.to_string().contains("symlink")
            || err.to_string().contains("open file"),
        "unexpected error message: {err}"
    );
    Ok(())
}

#[test]
fn test_receipt_rejection_oversized() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let receipt_path = temp.path().join("oversized.json");
    let target_size =
        usize::try_from(MAX_RECEIPT_BYTES + 1024).map_err(|e| format!("size overflow: {e}"))?;
    let padding = vec![b' '; target_size];
    fs::write(&receipt_path, padding)?;

    let err = receipt::load_receipt_from_path(&receipt_path).unwrap_err();
    assert!(
        err.to_string().contains("exceeded byte limit")
            || err.to_string().contains("failed to read receipt file"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn test_receipt_rejection_duplicate_json_keys() -> Result<()> {
    let payload = sample_payload(1);
    let receipt = CandidateReceipt::new_signed(payload)?;
    let serialized = serde_json::to_string(&receipt)?;
    let tampered = serialized.replace(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
    );
    let err = receipt::parse_receipt_bytes(tampered.as_bytes()).unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("duplicate")
            || err.to_string().contains("invalid json"),
        "expected duplicate key error: {err}"
    );
    Ok(())
}

#[test]
fn test_receipt_rejection_nonfinite_numbers() {
    let raw = br#"{"schema_version": NaN, "criterion": 1}"#;
    let err = receipt::parse_receipt_bytes(raw).unwrap_err();
    assert!(
        err.to_string().contains("expected value")
            || err.to_string().contains("nonfinite")
            || err.to_string().contains("invalid"),
        "expected nonfinite number error: {err}"
    );
}

#[test]
fn test_receipt_rejection_unknown_fields() -> Result<()> {
    let payload = sample_payload(1);
    let receipt = CandidateReceipt::new_signed(payload)?;
    let mut value = serde_json::to_value(&receipt)?;
    value
        .as_object_mut()
        .unwrap()
        .insert("unrecognized_field".into(), serde_json::json!("tamper"));
    let raw = serde_json::to_vec(&value)?;
    let err = receipt::parse_receipt_bytes(&raw).unwrap_err();
    assert!(
        err.to_string().contains("unknown field"),
        "expected unknown field rejection: {err}"
    );
    Ok(())
}

#[test]
fn test_receipt_rejection_missing_mandatory_fields() -> Result<()> {
    let payload = sample_payload(1);
    let receipt = CandidateReceipt::new_signed(payload)?;
    let mut value = serde_json::to_value(&receipt)?;
    value.as_object_mut().unwrap().remove("cleanup");
    let raw = serde_json::to_vec(&value)?;
    let err = receipt::parse_receipt_bytes(&raw).unwrap_err();
    assert!(
        err.to_string().contains("missing field `cleanup`"),
        "expected missing field rejection: {err}"
    );
    Ok(())
}

#[test]
fn test_receipt_rejection_tampered_payload_hash() -> Result<()> {
    let payload = sample_payload(1);
    let mut receipt = CandidateReceipt::new_signed(payload)?;
    receipt.commands[0].command = vec!["malicious_cmd".into()];
    let inventory = sample_inventory();
    let err = receipt::validate_candidate_receipt(&receipt, &inventory, 1).unwrap_err();
    assert!(
        err.to_string().contains("integrity hash mismatch"),
        "expected integrity hash mismatch: {err}"
    );
    Ok(())
}

#[test]
fn test_criterion_evaluation_pending_when_missing() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let status = criteria::evaluate_criterion(
        temp.path(),
        1,
        "Epic Ledgers Audit (E01–E30)",
        "epic-ledgers",
        "independent evidence for every required deliverable",
    )?;
    assert!(!status.satisfied);
    assert!(
        status
            .summary
            .contains("Pending: missing receipt 'criterion-01-epic-ledgers.json'")
    );
    assert!(
        status
            .summary
            .contains("validated current candidate-bound receipts unavailable")
    );
    Ok(())
}

#[test]
fn test_criterion_evaluation_pending_when_unqualified() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let receipts_dir = temp.path().join("docs/release/receipts");
    fs::create_dir_all(&receipts_dir)?;
    let receipt_path = receipts_dir.join("criterion-01-epic-ledgers.json");

    let mut payload = sample_payload(1);
    payload.assertions[0].passed = false;
    payload.assertions[0].detail = Some("critical failure observed".into());
    let receipt = CandidateReceipt::new_signed(payload)?;
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;

    let release_dir = temp.path().join("docs/release");
    fs::create_dir_all(&release_dir)?;
    let cell_inventory = serde_json::json!({
        "source_revision": "2ef1c4787989f11f868f81bb84ae2afd4a49a81d",
        "node_cells": [{
            "output": {
                "filename": "rubix-kube",
                "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            },
            "inputs": [{
                "path": "bundle.manifest",
                "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            }]
        }]
    });
    fs::write(
        release_dir.join("cell-inventory.json"),
        serde_json::to_vec(&cell_inventory)?,
    )?;

    let status = check_criterion_1_epic_ledgers(temp.path())?;
    assert!(!status.satisfied);
    assert!(
        status
            .summary
            .contains("Pending: unqualified receipt 'criterion-01-epic-ledgers.json'")
    );
    assert!(status.summary.contains("critical failure observed"));
    assert!(
        status
            .summary
            .contains("validated current candidate-bound receipts unavailable")
    );
    Ok(())
}

#[test]
fn test_criterion_evaluation_satisfied_when_valid() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let receipts_dir = temp.path().join("docs/release/receipts");
    fs::create_dir_all(&receipts_dir)?;
    let receipt_path = receipts_dir.join("criterion-01-epic-ledgers.json");

    let payload = sample_payload(1);
    let receipt = CandidateReceipt::new_signed(payload)?;
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;

    let release_dir = temp.path().join("docs/release");
    fs::create_dir_all(&release_dir)?;
    let cell_inventory = serde_json::json!({
        "source_revision": "2ef1c4787989f11f868f81bb84ae2afd4a49a81d",
        "node_cells": [{
            "output": {
                "filename": "rubix-kube",
                "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            },
            "inputs": [{
                "path": "bundle.manifest",
                "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            }]
        }]
    });
    fs::write(
        release_dir.join("cell-inventory.json"),
        serde_json::to_vec(&cell_inventory)?,
    )?;

    let status = check_criterion_1_epic_ledgers(temp.path())?;
    assert!(status.satisfied);
    assert!(
        status.summary.contains(
            "Satisfied: validated candidate-bound receipt 'criterion-01-epic-ledgers.json'"
        )
    );
    Ok(())
}

#[test]
fn test_default_repo_checkout_criteria_all_pending() -> Result<()> {
    let root = root_dir()?;
    let all = verify_all_criteria(&root)?;
    assert_eq!(all.len(), 11);
    for status in all {
        assert!(
            !status.satisfied,
            "criterion {} must be pending",
            status.number
        );
        assert!(
            status
                .summary
                .contains("Pending: missing receipt 'criterion-")
        );
        assert!(
            status
                .summary
                .contains("validated current candidate-bound receipts unavailable")
        );
    }
    let report = audit_repository_metadata(&root)?;
    assert_eq!(report.criteria_reports.len(), 11);
    assert!(report.criteria_reports.iter().all(|c| !c.satisfied));

    let err = run_release_qualification(&root).unwrap_err();
    assert!(err.to_string().contains("RELEASE UNQUALIFIED"));
    Ok(())
}

#[test]
fn test_readme_example_receipt_valid() -> Result<()> {
    let readme_receipt_json = r#"{
  "schema_version": 1,
  "criterion": 1,
  "description": "Sample valid qualification run",
  "candidate": {
    "source_revision": "2ef1c4787989f11f868f81bb84ae2afd4a49a81d",
    "binary_digests": {
      "rubix-kube": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "payload_digests": {
      "bundle.manifest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    }
  },
  "environment": {
    "host": "linux-arm64",
    "kernel": "6.6.137",
    "runner": "github-hosted-ubuntu-24.04-arm"
  },
  "commands": [
    {
      "command": [
        "rubix-kube",
        "--version"
      ],
      "exit_code": 0,
      "duration_ms": 15
    }
  ],
  "assertions": [
    {
      "name": "startup_verified",
      "passed": true,
      "detail": "clean startup confirmed"
    }
  ],
  "skips": [
    {
      "name": "musl_dynamic",
      "reason": "glibc host platform"
    }
  ],
  "cleanup": {
    "cleaned_paths": [
      "/tmp/test"
    ],
    "remaining_containers": [],
    "remaining_images": [],
    "status": "complete"
  },
  "timestamps": {
    "started_at": "2026-10-08T12:00:00Z",
    "completed_at": "2026-10-08T12:01:00Z"
  },
  "integrity_hash": "e53e9fe4d16e18734a4f399e7b110f9a164242332a91278254e10e0a58bb5983"
}"#;
    let receipt: CandidateReceipt = receipt::parse_receipt_bytes(readme_receipt_json.as_bytes())?;
    assert_eq!(receipt.schema_version, 1);
    assert_eq!(receipt.criterion, 1);
    receipt.verify_integrity()?;
    let inventory = sample_inventory();
    receipt::validate_candidate_receipt(&receipt, &inventory, 1)?;
    Ok(())
}
