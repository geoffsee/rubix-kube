//! CLI verification tool for migration failure rehearsal and operator recovery (Epic E30 / Issue #125).
//!
//! Rehearses migration failures across supported starting versions (v1.1.8, v1.2.0, v1.3.0, v1.3.1-v1.3.3)
//! and interrupted transition stages on disposable installations.
//! Verifies operator recovery/rollback, state restoration, client access, and refusal on missing/corrupted backups.

use std::process::ExitCode;

use rubix_dev::state_transition::KubeconfigFormat;
use rubix_dev::state_transition::recovery::{
    BackupCondition, RehearsalScenario, TransitionStage, run_rehearsal,
    version_recovery_limitations,
};
use rubix_dev::state_transition::versions::SupportedStartingVersion;

fn rehearse_supported_versions() -> Result<(), String> {
    println!("--- Rehearsing Operator Recovery Across Starting Versions ---");

    for ver in SupportedStartingVersion::ALL {
        println!(
            "\n[Starting Version: {}] ({})",
            ver.as_str(),
            ver.layout_description()
        );

        // Rehearse recovery from mid-upgrade failure (ArtifactReplacement) with both YAML and JSON kubeconfigs
        for format in [KubeconfigFormat::Yaml, KubeconfigFormat::Json] {
            let scenario = RehearsalScenario {
                starting_version: ver,
                target_version: "v1.4.0".into(),
                kubeconfig_format: format,
                interrupt_stage: TransitionStage::ArtifactReplacement,
                backup_condition: BackupCondition::Valid,
            };

            let result = run_rehearsal(&scenario)
                .map_err(|e| format!("rehearsal failed for {ver} ({format}): {e}"))?;

            if !result.overall_success {
                return Err(format!("overall success false for {ver} ({format})"));
            }

            println!(
                "  [ok] Format={format} Stage={:?} -> Recovery: executed={}, config_restored={}, pki_restored={}, client_access={}, datastore_restored={}, storage_restored={}, receipts_cleaned={}",
                scenario.interrupt_stage,
                result.recovery_executed,
                result.config_restored,
                result.pki_restored,
                result.client_access_verified,
                result.datastore_restored,
                result.storage_restored,
                result.receipts_cleaned,
            );
        }

        // Display known limitations for this version
        println!("  Known limitations ({}):", ver.as_str());
        for lim in version_recovery_limitations(ver) {
            println!("    - {lim}");
        }
    }
    println!();
    Ok(())
}

fn rehearse_interrupted_stages() -> Result<(), String> {
    println!("--- Rehearsing Interrupted Transition Stages (v1.2.0 baseline) ---");

    let stages_to_test = [
        TransitionStage::Validation,
        TransitionStage::Preparation,
        TransitionStage::Quiesce,
        TransitionStage::Snapshot,
        TransitionStage::ReceiptPending,
        TransitionStage::ArtifactReplacement,
        TransitionStage::ConfigMigration,
        TransitionStage::ServiceStart,
        TransitionStage::ReceiptCommitting,
        TransitionStage::PostCommitCleanup,
    ];

    for stage in stages_to_test {
        let scenario = RehearsalScenario {
            starting_version: SupportedStartingVersion::V1_2_0,
            target_version: "v1.4.0".into(),
            kubeconfig_format: KubeconfigFormat::Yaml,
            interrupt_stage: stage,
            backup_condition: BackupCondition::Valid,
        };

        let result = run_rehearsal(&scenario)
            .map_err(|e| format!("rehearsal failed for stage {stage:?}: {e}"))?;

        if !result.overall_success {
            return Err(format!("rehearsal failed for stage {stage:?}"));
        }

        println!(
            "  [ok] Stage {:<20} -> Receipt before: {:<20} Success: {}",
            stage.name(),
            result
                .receipt_observed_before_recovery
                .as_deref()
                .unwrap_or("none"),
            result.overall_success,
        );
    }
    println!();
    Ok(())
}

fn rehearse_unavailable_backup_refusal() -> Result<(), String> {
    println!("--- Rehearsing Refusal on Missing/Corrupted Backups ---");

    for condition in [BackupCondition::Missing, BackupCondition::Corrupted] {
        let scenario = RehearsalScenario {
            starting_version: SupportedStartingVersion::V1_3_0,
            target_version: "v1.4.0".into(),
            kubeconfig_format: KubeconfigFormat::Yaml,
            interrupt_stage: TransitionStage::ArtifactReplacement,
            backup_condition: condition,
        };

        let result = run_rehearsal(&scenario)
            .map_err(|e| format!("rehearsal failed for {condition:?}: {e}"))?;

        if !result.recovery_refused_as_expected {
            return Err(format!("expected recovery to be refused for {condition:?}"));
        }

        println!(
            "  [ok] Condition: {:<12} -> Recovery Refused: {} (Diagnostic: {})",
            format!("{:?}", condition),
            result.recovery_refused_as_expected,
            result.recovery_diagnostic.as_deref().unwrap_or("none"),
        );
    }
    println!();
    Ok(())
}

fn run() -> Result<(), String> {
    println!(
        "=== Rubix Migration Failure & Operator Recovery Rehearsal (Issue #125 / Gate C14) ===\n"
    );

    rehearse_supported_versions()?;
    rehearse_interrupted_stages()?;
    rehearse_unavailable_backup_refusal()?;

    println!("All migration failure rehearsal and operator recovery checks passed successfully.");
    println!(
        "Production migration remains qualified within isolated fixture boundaries; live Linux cluster qualification remains separate."
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        },
    }
}
