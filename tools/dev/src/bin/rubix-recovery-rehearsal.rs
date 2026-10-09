//! CLI verification tool for migration failure rehearsal, operator recovery, and in-process synthetic migration rehearsal (Epic E30 / Issue #125 / Epic E36 / Issue #354).
//!
//! Rehearses:
//! 1. Operator recovery and rollback across supported starting versions (v1.1.8, v1.2.0, v1.3.0, v1.3.1-v1.3.3)
//!    and interrupted transition stages on disposable installations.
//! 2. In-process synthetic Go-to-Rust Kine `SQLite` migration across all 6 supported starting versions and both kubeconfig formats.
//! 3. Option B in-process control plane boundary per ADR (amended 2026-10-07):
//!    raw `SQLite` rejection, explicit export/import into native RUBXSNP1 format, monotonic revisions.
//! 4. Preservation of PKI CA fingerprint, admin x509 chain, static manifests, and PV storage.
//! 5. In-process conversion elapsed time measurement in milliseconds across the conversion window
//!    (does NOT represent live cluster downtime).
//! 6. Optional generation of Criterion 8 candidate qualification receipt (fails closed in checkout).
//!
//! NOTE: Generated Kine records are synthetic test fixtures and do NOT satisfy `tools/parity`'s requirement
//! for genuine pinned upstream Kine `SQLite` fixtures. Live Linux rehearsal with genuine upstream databases
//! from the 6 `KubeSolo` versions remains pending.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rubix_dev::release_qualification::receipt::{CandidateReceipt, load_candidate_inventory};
use rubix_dev::state_transition::recovery::{
    BackupCondition, RehearsalScenario, TransitionStage, run_rehearsal,
    version_recovery_limitations,
};
use rubix_dev::state_transition::versions::SupportedStartingVersion;
use rubix_dev::state_transition::{
    KubeconfigFormat, OPTION_B_SCOPE_MARKER, SyntheticMigrationResult,
    build_criterion_8_receipt_payload_with_inventory, run_synthetic_migration_rehearsal,
};

struct Args {
    generate_receipt: Option<PathBuf>,
    show_help: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let mut generate_receipt = None;
    let mut show_help = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--generate-receipt" => {
                let path = args
                    .next()
                    .ok_or_else(|| "--generate-receipt requires a path argument".to_string())?;
                generate_receipt = Some(PathBuf::from(path));
            },
            "--rehearse-synthetic-migration" | "--rehearse-live-migration" => {
                // Explicit flag supported for symmetry with receipt command execution record
            },
            "-h" | "--help" => {
                show_help = true;
            },
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Args {
        generate_receipt,
        show_help,
    })
}

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

    let stages_to_test = TransitionStage::ALL;

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

    for condition in [
        BackupCondition::Missing,
        BackupCondition::Corrupted,
        BackupCondition::Symlink,
    ] {
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

async fn rehearse_synthetic_migrations() -> Result<Vec<SyntheticMigrationResult>, String> {
    println!("--- Rehearsing In-Process Synthetic Go-to-Rust Migration Matrix (Issue #354) ---");
    println!(
        "  NOTE: Generated records are synthetic fixtures; live Linux rehearsal with genuine upstream Kine SQLite databases remains pending."
    );
    let mut results = Vec::new();

    for ver in SupportedStartingVersion::ALL {
        for format in [KubeconfigFormat::Yaml, KubeconfigFormat::Json] {
            let result = run_synthetic_migration_rehearsal(ver, format)
                .await
                .map_err(|e| {
                    format!("synthetic migration rehearsal failed for {ver} ({format}): {e}")
                })?;

            if !result.overall_success {
                return Err(format!("overall success false for {ver} ({format})"));
            }

            println!(
                "  [ok] Version={:<6} Format={:<4} -> Raw SQLite rejected={}, Monotonic revs={}/{}, Keys match={}, PKI CA sha256={}..., CA preserved={}, Admin cert verified={}, Static manifests={}, PV storage={}, Elapsed={}ms",
                result.starting_version.as_str(),
                result.kubeconfig_format,
                result.raw_sqlite_rejected,
                result.restored_revision,
                result.source_max_revision,
                result.keys_identical,
                &result.ca_fingerprint_sha256[..12],
                result.ca_fingerprint_preserved,
                result.admin_identity_verified,
                result.static_manifests_preserved,
                result.pv_storage_preserved,
                result.conversion_elapsed_ms,
            );

            results.push(result);
        }
    }

    println!();
    Ok(results)
}

async fn run() -> Result<(), String> {
    let args = parse_args()?;
    if args.show_help {
        println!(
            "Usage: rubix-recovery-rehearsal [--rehearse-synthetic-migration] [--generate-receipt <PATH>]"
        );
        return Ok(());
    }

    println!(
        "=== Rubix Migration Failure, Operator Recovery & In-Process Synthetic Rehearsal (Issues #125, #354) ===\n"
    );
    println!("{OPTION_B_SCOPE_MARKER}\n");

    rehearse_supported_versions()?;
    rehearse_interrupted_stages()?;
    rehearse_unavailable_backup_refusal()?;
    let synthetic_results = rehearse_synthetic_migrations().await?;

    if let Some(receipt_path) = args.generate_receipt {
        println!("--- Generating Candidate-Bound Criterion 8 Qualification Receipt ---");
        let inventory = if std::env::var_os("RUBIX_CANDIDATE_INVENTORY_PATH").is_some() {
            load_candidate_inventory(Path::new(""))
                .map_err(|e| format!("failed to load candidate inventory from environment: {e}"))?
        } else {
            let root = rubix_dev::repository_root(Path::new(env!("CARGO_MANIFEST_DIR")))
                .map_err(|e| format!("failed to find repository root: {e}"))?;
            load_candidate_inventory(&root)
                .map_err(|e| format!("failed to load candidate inventory: {e}"))?
        };
        let payload =
            build_criterion_8_receipt_payload_with_inventory(&inventory, &synthetic_results);
        let receipt = CandidateReceipt::new_with_integrity_hash(payload)
            .map_err(|e| format!("failed to generate criterion 8 receipt: {e}"))?;
        let receipt_json = serde_json::to_string_pretty(&receipt)
            .map_err(|e| format!("failed to serialize receipt: {e}"))?;
        if let Some(parent) = receipt_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create directory {}: {e}", parent.display()))?;
        }
        std::fs::write(&receipt_path, receipt_json)
            .map_err(|e| format!("failed to write receipt to {}: {e}", receipt_path.display()))?;
        println!("  [ok] Receipt written to {}", receipt_path.display());
        println!(
            "  NOTICE: Candidate qualification receipt generated for operator review; release checkout remains uncommitted per qualification policy."
        );
        println!();
    }

    println!(
        "All in-process synthetic migration and operator recovery rehearsal checks passed successfully."
    );
    println!(
        "Synthetic and in-process checks passed; candidate qualification receipt remains pending live Linux infrastructure execution."
    );
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        },
    }
}
