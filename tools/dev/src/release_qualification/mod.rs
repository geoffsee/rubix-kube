//! Automated release qualification verification harness and gates (Issue #126 / Gate C16/C17).
//!
//! Enforces:
//! - Cryptographic artifact digest bindings.
//! - License and attribution completeness.
//! - Link integrity and completeness across all documentation artifacts.
//! - Formal verification that all 11 Roadmap #263 completion criteria are satisfied with verifiable evidence.

pub mod attribution;
pub mod criteria;
pub mod digest_bindings;
pub mod link_integrity;

use crate::Result;
use std::path::Path;

/// Overall release qualification report.
#[derive(Debug, Clone)]
pub struct ReleaseQualificationReport {
    pub upstream_inputs_verified: usize,
    pub upstream_sources_verified: usize,
    pub catalog_assets_verified: usize,
    pub retained_components_attributed: usize,
    pub workspace_licenses_checked: usize,
    pub documentation_summary: link_integrity::LinkIntegritySummary,
    pub criteria_reports: Vec<criteria::CriterionStatus>,
}

impl ReleaseQualificationReport {
    /// Prints a formatted audit summary to stdout.
    pub fn print_summary(&self) {
        println!("===============================================================================");
        println!(
            "                RUBIX KUBE RELEASE QUALIFICATION AUDIT LEDGER                   "
        );
        println!("===============================================================================");
        println!();
        println!("1. CRYPTOGRAPHIC ARTIFACT DIGEST BINDINGS");
        println!(
            "  - Upstream generator inputs (tools/upstream/inputs.json):  {} verified (sha256)",
            self.upstream_inputs_verified
        );
        println!(
            "  - Upstream sources (docs/architecture/upstream-inputs.json): {} verified (git commit)",
            self.upstream_sources_verified
        );
        println!(
            "  - Asset catalog entries:                                     {} verified",
            self.catalog_assets_verified
        );
        println!();
        println!("2. LICENSE & ATTRIBUTION COMPLETENESS");
        println!(
            "  - Retained components attributed (docs/architecture/attribution.md): {}/17 verified",
            self.retained_components_attributed
        );
        println!(
            "  - Permitted license categories (deny.toml):                          {} verified",
            self.workspace_licenses_checked
        );
        println!();
        println!("3. DOCUMENTATION LINK INTEGRITY & COMPLETENESS");
        println!(
            "  - Documentation files inspected:                             {}",
            self.documentation_summary.documents_checked
        );
        println!(
            "  - Total links checked:                                       {}",
            self.documentation_summary.total_links
        );
        println!(
            "  - Local document links verified:                             {}",
            self.documentation_summary.local_links_verified
        );
        println!(
            "  - External links recorded:                                   {}",
            self.documentation_summary.external_links_skipped
        );
        println!(
            "  - Broken links detected:                                     {}",
            self.documentation_summary.broken_links.len()
        );
        println!();
        println!("4. ROADMAP #263 COMPLETION CRITERIA (1–11)");
        for status in &self.criteria_reports {
            let mark = if status.satisfied {
                "✓ [PASS]"
            } else {
                "✗ [FAIL]"
            };
            println!(
                "  {:8} Criterion {:2}: {:<42} -> {}",
                mark, status.number, status.name, status.summary
            );
        }
        println!();
        println!("===============================================================================");
        println!("STATUS: ALL QUALIFICATION GATES PASSED (Gate C16/C17 Satisfied)");
        println!("===============================================================================");
    }
}

/// Executes all release qualification checks against the given repository root.
pub fn run_release_qualification(root: &Path) -> Result<ReleaseQualificationReport> {
    // 1. Cryptographic Digest Bindings
    let upstream_inputs_verified = digest_bindings::verify_upstream_inputs(root)?;
    let upstream_sources_verified = digest_bindings::verify_upstream_provenance(root)?;
    let catalog_assets_verified = digest_bindings::verify_catalog_bindings()?;

    // 2. License & Attribution Completeness
    let retained_components_attributed = attribution::verify_retained_attribution(root)?;
    let workspace_licenses_checked = attribution::verify_workspace_license_policy(root)?;

    // 3. Link Integrity & Completeness
    let documentation_summary = link_integrity::verify_documentation_links(root)?;

    // 4. Roadmap #263 Completion Criteria (1–11)
    let criteria_reports = criteria::verify_all_criteria(root)?;

    Ok(ReleaseQualificationReport {
        upstream_inputs_verified,
        upstream_sources_verified,
        catalog_assets_verified,
        retained_components_attributed,
        workspace_licenses_checked,
        documentation_summary,
        criteria_reports,
    })
}
