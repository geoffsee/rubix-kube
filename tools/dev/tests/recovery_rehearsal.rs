//! Integration tests for migration failure rehearsal and operator recovery (Issue #125 / Epic E30 Gate C14).
//!
//! Verifies:
//! 1. Interrupted transition stages across all supported starting versions (v1.1.8, v1.2.0, v1.3.0, v1.3.1-v1.3.3).
//! 2. Operator recovery restoring promised state across all 5 domains without relying on unavailable/overwritten backups.
//! 3. Dual format kubeconfig accommodation (both YAML and JSON) with cryptographic CA certificate verification.
//! 4. Refusal and diagnostic retention on missing or corrupted pre-upgrade backups.
//! 5. Version-specific known limitations and operator runbook procedures.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rubix_dev::release_qualification::receipt::{
    CandidateInventory, CandidateReceipt, validate_candidate_receipt,
};
use rubix_dev::repository_root;
use rubix_dev::state_transition::recovery::{
    BackupCondition, RehearsalScenario, TransitionStage, run_rehearsal,
    version_recovery_limitations,
};
use rubix_dev::state_transition::versions::SupportedStartingVersion;
use rubix_dev::state_transition::{
    KubeconfigFormat, OPTION_B_SCOPE_MARKER, assert_raw_sqlite_rejected,
    build_criterion_8_receipt_payload_with_inventory, run_live_migration_rehearsal,
};
use rubixctl::upgrade::{
    BackupIntegrityError, ReceiptKind, parse_receipt_file, validate_backup_integrity,
};

#[test]
fn test_recovery_rehearsal_all_supported_starting_versions() {
    for ver in SupportedStartingVersion::ALL {
        // Test with YAML kubeconfig
        let scenario_yaml = RehearsalScenario {
            starting_version: ver,
            target_version: "v1.4.0".into(),
            kubeconfig_format: KubeconfigFormat::Yaml,
            interrupt_stage: TransitionStage::ArtifactReplacement,
            backup_condition: BackupCondition::Valid,
        };
        let result_yaml = run_rehearsal(&scenario_yaml).unwrap();
        assert!(
            result_yaml.overall_success,
            "rehearsal failed for {ver} (YAML)"
        );
        assert!(result_yaml.recovery_executed);
        assert!(result_yaml.config_restored);
        assert!(result_yaml.pki_restored);
        assert!(result_yaml.client_access_verified);
        assert!(result_yaml.datastore_restored);
        assert!(result_yaml.storage_restored);
        assert!(result_yaml.receipts_cleaned);

        // Test with JSON kubeconfig
        let scenario_json = RehearsalScenario {
            starting_version: ver,
            target_version: "v1.4.0".into(),
            kubeconfig_format: KubeconfigFormat::Json,
            interrupt_stage: TransitionStage::ArtifactReplacement,
            backup_condition: BackupCondition::Valid,
        };
        let result_json = run_rehearsal(&scenario_json).unwrap();
        assert!(
            result_json.overall_success,
            "rehearsal failed for {ver} (JSON)"
        );
        assert!(result_json.client_access_verified);
        assert!(result_json.pki_restored);
        assert!(result_json.datastore_restored);
        assert!(result_json.config_restored);

        // Verify version-specific limitations are documented
        let lims = version_recovery_limitations(ver);
        assert!(!lims.is_empty(), "limitations must be documented for {ver}");
        assert!(lims.iter().any(|l| l.contains("Downtime window")));
        assert!(lims.iter().any(|l| l.contains("raw SQLite")));
    }
}

#[test]
fn test_recovery_v1_1_8_removes_migration_created_config_and_restores_flags() {
    let scenario = RehearsalScenario {
        starting_version: SupportedStartingVersion::V1_1_8,
        target_version: "v1.4.0".into(),
        kubeconfig_format: KubeconfigFormat::Yaml,
        interrupt_stage: TransitionStage::ConfigMigration,
        backup_condition: BackupCondition::Valid,
    };

    let result = run_rehearsal(&scenario).unwrap();
    assert!(result.overall_success);
    assert!(result.config_restored);
    assert!(result.pki_restored);
    assert!(result.datastore_restored);
    assert!(result.receipts_cleaned);

    let lims = version_recovery_limitations(SupportedStartingVersion::V1_1_8);
    assert!(lims.iter().any(|l| l.contains("Legacy CLI flags")));
    assert!(
        lims.iter()
            .any(|l| l.contains("unsupported in this version"))
    );
}

#[test]
fn test_recovery_v1_3_0_preserves_and_restores_yaml_config() {
    let scenario = RehearsalScenario {
        starting_version: SupportedStartingVersion::V1_3_0,
        target_version: "v1.4.0".into(),
        kubeconfig_format: KubeconfigFormat::Yaml,
        interrupt_stage: TransitionStage::ServiceStart,
        backup_condition: BackupCondition::Valid,
    };

    let result = run_rehearsal(&scenario).unwrap();
    assert!(result.overall_success);
    assert!(result.config_restored);
    assert!(result.pki_restored);
    assert!(result.client_access_verified);
    assert!(result.datastore_restored);
    assert!(result.storage_restored);
    assert!(result.receipts_cleaned);

    let lims = version_recovery_limitations(SupportedStartingVersion::V1_3_0);
    assert!(lims.iter().any(|l| l.contains("MinConfigFileVersion")));
    assert!(lims.iter().any(|l| l.contains("0600 mode permissions")));
}

#[test]
fn test_recovery_rehearsal_across_all_transition_stages() {
    for stage in TransitionStage::ALL {
        let scenario = RehearsalScenario {
            starting_version: SupportedStartingVersion::V1_2_0,
            target_version: "v1.4.0".into(),
            kubeconfig_format: KubeconfigFormat::Yaml,
            interrupt_stage: stage,
            backup_condition: BackupCondition::Valid,
        };

        let result = run_rehearsal(&scenario).unwrap();
        match stage {
            TransitionStage::Validation | TransitionStage::Preparation => {
                assert!(
                    !result
                        .backend_calls
                        .iter()
                        .any(|c| c.starts_with("systemctl "))
                );
                assert_eq!(result.retained_backup_count, 0);
            },
            TransitionStage::Quiesce | TransitionStage::Snapshot => {
                assert!(
                    result
                        .backend_calls
                        .iter()
                        .any(|c| c == "systemctl start kubesolo")
                );
                assert_eq!(result.retained_backup_count, 0);
            },
            TransitionStage::ReceiptPending => {
                assert!(
                    result
                        .backend_calls
                        .iter()
                        .any(|c| c == "systemctl start kubesolo")
                );
                assert_eq!(result.retained_backup_count, 1);
                assert!(result.backup_validated);
                assert!(result.receipt_observed_before_recovery.is_none());
            },
            _ => {},
        }
        assert!(
            result.overall_success,
            "stage {stage:?} rehearsal failed: {:?}",
            result.recovery_diagnostic
        );
        assert!(
            result.receipts_cleaned,
            "stage {stage:?} receipts were not cleaned"
        );
    }
}

#[test]
fn test_refusal_when_backup_missing_or_deleted() {
    let scenario = RehearsalScenario {
        starting_version: SupportedStartingVersion::V1_3_1,
        target_version: "v1.4.0".into(),
        kubeconfig_format: KubeconfigFormat::Yaml,
        interrupt_stage: TransitionStage::ArtifactReplacement,
        backup_condition: BackupCondition::Missing,
    };

    let result = run_rehearsal(&scenario).unwrap();
    assert!(result.recovery_refused_as_expected);
    assert!(result.overall_success);
    assert!(
        result
            .recovery_diagnostic
            .as_ref()
            .unwrap()
            .contains("recovery refused: backup at")
    );
    assert!(
        !result.receipts_cleaned,
        "receipt must be preserved for operator diagnosis"
    );
}

#[test]
fn test_refusal_when_backup_is_corrupted() {
    let scenario = RehearsalScenario {
        starting_version: SupportedStartingVersion::V1_3_2,
        target_version: "v1.4.0".into(),
        kubeconfig_format: KubeconfigFormat::Json,
        interrupt_stage: TransitionStage::ArtifactReplacement,
        backup_condition: BackupCondition::Corrupted,
    };

    let result = run_rehearsal(&scenario).unwrap();
    assert!(result.recovery_refused_as_expected);
    assert!(result.overall_success);
    assert!(
        result
            .recovery_diagnostic
            .as_ref()
            .unwrap()
            .contains("missing required state directory")
    );
    assert!(
        !result.receipts_cleaned,
        "receipt must be preserved for operator diagnosis"
    );
}

#[test]
fn test_refusal_when_backup_is_symlink() {
    let scenario = RehearsalScenario {
        starting_version: SupportedStartingVersion::V1_3_3,
        target_version: "v1.4.0".into(),
        kubeconfig_format: KubeconfigFormat::Yaml,
        interrupt_stage: TransitionStage::ArtifactReplacement,
        backup_condition: BackupCondition::Symlink,
    };

    let result = run_rehearsal(&scenario).unwrap();
    assert!(result.recovery_refused_as_expected);
    assert!(result.overall_success);
    assert!(
        result
            .recovery_diagnostic
            .as_ref()
            .unwrap()
            .contains("unsafe symlink")
    );
    assert!(!result.receipts_cleaned);
}

#[test]
fn test_service_start_failure_reverses_dirty_datastore_mutations() {
    // Interruption during ServiceStart simulates the new binary modifying the DB then crashing
    let scenario = RehearsalScenario {
        starting_version: SupportedStartingVersion::V1_1_8,
        target_version: "v1.4.0".into(),
        kubeconfig_format: KubeconfigFormat::Yaml,
        interrupt_stage: TransitionStage::ServiceStart,
        backup_condition: BackupCondition::Valid,
    };

    let result = run_rehearsal(&scenario).unwrap();
    assert!(result.overall_success);
    assert!(
        result.datastore_restored,
        "dirty datastore writes must be completely reversed"
    );
    assert!(
        result.pki_restored,
        "dirty PKI writes must be completely reversed"
    );
    assert!(result.config_restored);
    assert!(result.client_access_verified);
}

#[test]
fn test_receipt_committing_finalizes_healthy_target() {
    let scenario = RehearsalScenario {
        starting_version: SupportedStartingVersion::V1_3_0,
        target_version: "v1.4.0".into(),
        kubeconfig_format: KubeconfigFormat::Yaml,
        interrupt_stage: TransitionStage::ReceiptCommitting,
        backup_condition: BackupCondition::Valid,
    };

    let result = run_rehearsal(&scenario).unwrap();
    assert!(result.overall_success);
    assert!(result.recovery_executed);
    assert!(result.receipts_cleaned);
}

#[test]
fn test_post_commit_cleanup_interruption_cleans_receipt_without_rollback() {
    let scenario = RehearsalScenario {
        starting_version: SupportedStartingVersion::V1_3_0,
        target_version: "v1.4.0".into(),
        kubeconfig_format: KubeconfigFormat::Yaml,
        interrupt_stage: TransitionStage::PostCommitCleanup,
        backup_condition: BackupCondition::Valid,
    };

    let result = run_rehearsal(&scenario).unwrap();
    assert!(result.overall_success);
    assert!(result.receipts_cleaned);
}

#[test]
fn test_backup_validation_detects_empty_state_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let backup_dir = tmp.path().join("empty-backup");
    fs::create_dir_all(backup_dir.join("pki")).unwrap();
    fs::create_dir_all(backup_dir.join("kine/db")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&backup_dir, fs::Permissions::from_mode(0o700)).unwrap();
    }

    // pki and kine/db are empty directories
    let err = validate_backup_integrity(&backup_dir).unwrap_err();
    assert!(matches!(
        err,
        BackupIntegrityError::EmptyStateDirectory { .. }
    ));
}

#[test]
fn test_parse_receipt_file_extracts_all_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let receipt_path = tmp.path().join(".upgrade-pending");
    fs::write(
        &receipt_path,
        "from=v1.2.0\ntarget=v1.4.0\nbackup=/var/lib/kubesolo/backups/test\n",
    )
    .unwrap();

    let receipt = parse_receipt_file(&receipt_path, ReceiptKind::Pending).unwrap();
    assert_eq!(receipt.kind, ReceiptKind::Pending);
    assert_eq!(receipt.from, "v1.2.0");
    assert_eq!(receipt.target, "v1.4.0");
    assert_eq!(
        receipt.backup.to_str().unwrap(),
        "/var/lib/kubesolo/backups/test"
    );
}

#[test]
fn test_option_b_scope_marker_and_sqlite_rejection() {
    assert_eq!(
        OPTION_B_SCOPE_MARKER,
        "E36.04:scope: Option B in-process control plane selected (ADR amended 2026-10-07); raw SQLite non-interchangeable; explicit export/import required"
    );

    let tmp = tempfile::tempdir().unwrap();
    let sqlite_file = tmp.path().join("state.db");
    fs::write(&sqlite_file, b"SQLite format 3\0corrupted-state").unwrap();
    assert!(assert_raw_sqlite_rejected(&sqlite_file));

    let non_sqlite_file = tmp.path().join("native.db");
    fs::write(&non_sqlite_file, b"RUBXSNP1valid-header").unwrap();
    assert!(!assert_raw_sqlite_rejected(&non_sqlite_file));
}

#[tokio::test]
async fn test_synthetic_migration_matrix_all_12_combinations() {
    let mut total_elapsed_ms = 0;

    for ver in SupportedStartingVersion::ALL {
        for format in [KubeconfigFormat::Yaml, KubeconfigFormat::Json] {
            let res = run_live_migration_rehearsal(ver, format)
                .await
                .expect("synthetic migration rehearsal must succeed");

            assert!(
                res.overall_success,
                "overall success failed for {ver} ({format})"
            );
            assert_eq!(res.starting_version, ver);
            assert_eq!(res.kubeconfig_format, format);
            assert!(res.raw_sqlite_rejected);
            assert_eq!(res.export_format, "RUBXSNP1");
            assert!(res.revisions_monotonic);
            assert!(res.restored_revision >= res.source_max_revision);
            assert!(res.keys_identical);
            assert_eq!(res.source_records_count, 10);
            assert_eq!(res.active_keys_count, 8);
            assert!(!res.ca_fingerprint_sha256.is_empty());
            assert!(res.ca_fingerprint_preserved);
            assert!(res.admin_identity_verified);
            assert!(res.static_manifests_preserved);
            assert!(res.pv_storage_preserved);
            assert!(res.conversion_elapsed_ms < 60_000);

            total_elapsed_ms += res.conversion_elapsed_ms;
        }
    }

    println!(
        "Total measured conversion elapsed time across 12 synthetic migration runs: {total_elapsed_ms}ms (in-memory; does not represent live cluster downtime)"
    );
}

#[tokio::test]
async fn test_criterion_8_receipt_generation_and_validation() {
    let result =
        run_live_migration_rehearsal(SupportedStartingVersion::V1_3_0, KubeconfigFormat::Yaml)
            .await
            .expect("live migration rehearsal must succeed");

    let inventory = CandidateInventory {
        source_revision: "2ef1c4787989f11f868f81bb84ae2afd4a49a81d".into(),
        binary_digests: BTreeMap::from([(
            "rubix-kube".into(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
        )]),
        payload_digests: BTreeMap::from([(
            "bundle.manifest".into(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        )]),
    };

    let started_at_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let payload =
        build_criterion_8_receipt_payload_with_inventory(&inventory, &[result], started_at_secs);
    assert_eq!(payload.criterion, 8);
    assert_eq!(payload.schema_version, 1);
    assert!(
        payload
            .assertions
            .iter()
            .any(|a| a.name == "option_b_scope_marker_verified"
                && a.detail.as_deref() == Some(OPTION_B_SCOPE_MARKER))
    );
    assert!(
        payload
            .assertions
            .iter()
            .any(|a| a.name == "v1.3.0_YAML_raw_sqlite_rejected" && a.passed)
    );
    assert!(
        payload
            .assertions
            .iter()
            .any(|a| a.name == "v1.3.0_YAML_datastore_monotonic_revisions" && a.passed)
    );
    assert!(
        payload
            .assertions
            .iter()
            .any(|a| a.name == "v1.3.0_YAML_keys_identical" && a.passed)
    );
    assert!(
        payload
            .assertions
            .iter()
            .any(|a| a.name == "v1.3.0_YAML_ca_fingerprint_verified" && a.passed)
    );
    assert!(
        payload
            .assertions
            .iter()
            .any(|a| a.name == "v1.3.0_YAML_admin_identity_verified" && a.passed)
    );
    assert!(
        payload
            .assertions
            .iter()
            .any(|a| a.name == "v1.3.0_YAML_static_manifests_preserved" && a.passed)
    );
    assert!(
        payload
            .assertions
            .iter()
            .any(|a| a.name == "v1.3.0_YAML_pv_storage_preserved" && a.passed)
    );
    assert!(
        payload
            .assertions
            .iter()
            .any(|a| a.name == "v1.3.0_YAML_downtime_measured" && a.passed)
    );

    let receipt = CandidateReceipt::new_with_integrity_hash(payload).unwrap();
    receipt.verify_integrity().unwrap();
    validate_candidate_receipt(&receipt, &inventory, 8).unwrap();

    // Verify repository checkout integrity: receipt must NOT be committed to docs/release/receipts/
    let root = repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let uncommitted_receipt_path =
        root.join("docs/release/receipts/criterion-08-state-migration.json");
    assert!(
        !uncommitted_receipt_path.exists(),
        "Criterion 8 receipt must NOT be committed to repository checkout before live Linux execution"
    );
}
