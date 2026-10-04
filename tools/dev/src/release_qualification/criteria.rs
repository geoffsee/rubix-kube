//! Roadmap #263 completion criteria and unavailable qualification evidence.
//!
//! Repository metadata, documentation, and synthetic fixtures cannot establish
//! current candidate-bound live qualification. No reader for trusted completion
//! receipts is implemented here; every criterion consequently remains pending.

use crate::Result;
use std::path::Path;

/// Status and evidence report for one Roadmap #263 completion criterion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriterionStatus {
    pub number: usize,
    pub name: &'static str,
    pub satisfied: bool,
    pub summary: String,
}

fn pending(number: usize, name: &'static str, evidence: &str) -> CriterionStatus {
    CriterionStatus {
        number,
        name,
        satisfied: false,
        summary: format!(
            "Pending: {evidence}; validated current candidate-bound receipts unavailable"
        ),
    }
}

/// Lists the missing acceptance-ledger qualification evidence.
pub fn check_criterion_1_epic_ledgers(_root: &Path) -> Result<CriterionStatus> {
    Ok(pending(
        1,
        "Epic Ledgers Audit (E01–E30)",
        "independent evidence for every required deliverable",
    ))
}

/// Lists the missing production component-boundary qualification evidence.
pub fn check_criterion_2_supervised_boundary(_root: &Path) -> Result<CriterionStatus> {
    Ok(pending(
        2,
        "Supervised Boundary & Datastore mTLS",
        "production startup and positive/negative live transport checks",
    ))
}

/// Matrix definitions do not prove actual target build, layout, or installation.
pub fn check_criterion_3_target_matrix() -> Result<CriterionStatus> {
    Ok(pending(
        3,
        "Target Architecture Matrix",
        "16 node cells, four OCI architectures and four management targets",
    ))
}

/// Lists the missing live addon, egress and authentication qualification evidence.
pub fn check_criterion_4_addons_and_egress(_root: &Path) -> Result<CriterionStatus> {
    Ok(pending(
        4,
        "Addons, Egress & D2K Authentication",
        "egress-denied image acquisition and positive/negative D2K authentication",
    ))
}

/// Lists the missing host/container lifecycle qualification evidence.
pub fn check_criterion_5_lifecycle_and_storage(_root: &Path) -> Result<CriterionStatus> {
    Ok(pending(
        5,
        "Lifecycle & State Retention",
        "install, reboot, recreate, interruption and retention matrices",
    ))
}

/// An existing harness or synthetic report cannot qualify conformance or soak.
pub fn check_criterion_6_conformance_and_soak(_root: &Path) -> Result<CriterionStatus> {
    Ok(pending(
        6,
        "Conformance & Recovery Qualification",
        "fresh live conformance, restart and platform soak results",
    ))
}

/// Documentation and fixture reports cannot qualify live performance budgets.
pub fn check_criterion_7_performance_budgets(_root: &Path) -> Result<CriterionStatus> {
    Ok(pending(
        7,
        "Performance & Memory Budgets",
        "matched live amd64/arm64 budgets, 24-hour settled memory and shutdown",
    ))
}

/// Constructed recovery fixtures cannot qualify an actual Kine migration.
pub fn check_criterion_8_state_migration(_root: &Path) -> Result<CriterionStatus> {
    Ok(pending(
        8,
        "Go-to-Rust Migration & Recovery Rehearsal",
        "supported-version live migration and interrupted recovery",
    ))
}

/// A local language-policy check does not establish all required final CI checks.
pub fn check_criterion_9_language_and_policy(_root: &Path) -> Result<CriterionStatus> {
    Ok(pending(
        9,
        "Language Policy & Toolchain Compliance",
        "final source/artifact debug, release, audit, drift and integration checks",
    ))
}

/// Digest syntax and source declarations cannot bind actual candidate bytes.
pub fn check_criterion_10_artifact_digest_bindings(_root: &Path) -> Result<CriterionStatus> {
    Ok(pending(
        10,
        "Cryptographic Digest Bindings & Provenance",
        "actual prepared inputs, complete candidate inventory and all qualification report bindings",
    ))
}

/// Document presence cannot establish a fresh operator handoff or publication.
pub fn check_criterion_11_operator_handoff(_root: &Path) -> Result<CriterionStatus> {
    Ok(pending(
        11,
        "Operator Documentation & Release Qualification",
        "fresh operator rehearsal, exact artifacts and trusted publication",
    ))
}

/// Reports all 11 pending criteria without treating metadata as qualification.
pub fn verify_all_criteria(root: &Path) -> Result<Vec<CriterionStatus>> {
    Ok(vec![
        check_criterion_1_epic_ledgers(root)?,
        check_criterion_2_supervised_boundary(root)?,
        check_criterion_3_target_matrix()?,
        check_criterion_4_addons_and_egress(root)?,
        check_criterion_5_lifecycle_and_storage(root)?,
        check_criterion_6_conformance_and_soak(root)?,
        check_criterion_7_performance_budgets(root)?,
        check_criterion_8_state_migration(root)?,
        check_criterion_9_language_and_policy(root)?,
        check_criterion_10_artifact_digest_bindings(root)?,
        check_criterion_11_operator_handoff(root)?,
    ])
}
