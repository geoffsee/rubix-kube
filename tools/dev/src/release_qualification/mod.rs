//! Repository metadata audit and pending release qualification (Issue #126).
//!
//! Input digest syntax, declared attribution and local document links are checked.
//! No actual prepared inputs, candidate artifacts or live completion receipts are
//! consumed by this entrypoint; C16/C17 and every completion criterion remain pending.

pub mod attribution;
pub mod criteria;
pub mod digest_bindings;
pub mod link_integrity;

use crate::Result;
use std::path::Path;

/// Repository metadata report; completion criteria remain unqualified.
#[derive(Debug, Clone)]
pub struct ReleaseQualificationReport {
    pub upstream_input_metadata_checked: usize,
    pub upstream_source_metadata_checked: usize,
    pub catalog_metadata_checked: usize,
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
            "                RUBIX KUBE REPOSITORY METADATA AUDIT (UNQUALIFIED)                   "
        );
        println!("===============================================================================");
        println!();
        println!("1. DECLARED INPUT METADATA (NOT BYTE OR PROVENANCE VERIFICATION)");
        println!(
            "  - Upstream generator inputs (tools/upstream/inputs.json):  {} metadata entries (sha256 syntax)",
            self.upstream_input_metadata_checked
        );
        println!(
            "  - Upstream sources (docs/architecture/upstream-inputs.json): {} metadata entries (commit syntax)",
            self.upstream_source_metadata_checked
        );
        println!(
            "  - Asset catalog entries:                                     {} metadata entries",
            self.catalog_metadata_checked
        );
        println!();
        println!("2. LICENSE & ATTRIBUTION COMPLETENESS");
        println!(
            "  - Retained components attributed (docs/architecture/attribution.md): {}/17 verified",
            self.retained_components_attributed
        );
        println!(
            "  - Permitted license categories (deny.toml):                          {} metadata entries",
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
                "[PENDING]"
            };
            println!(
                "  {:8} Criterion {:2}: {:<42} -> {}",
                mark, status.number, status.name, status.summary
            );
        }
        println!();
        println!("===============================================================================");
        println!("STATUS: RELEASE UNQUALIFIED (C16/C17 pending)");
        println!("===============================================================================");
    }
}

/// Audits repository metadata and reports all completion criteria as pending.
/// A successful Result means the metadata audit ran, never that a release qualified.
pub fn audit_repository_metadata(root: &Path) -> Result<ReleaseQualificationReport> {
    // 1. Cryptographic Digest Bindings
    let upstream_input_metadata_checked = digest_bindings::check_upstream_input_metadata(root)?;
    let upstream_source_metadata_checked =
        digest_bindings::check_upstream_provenance_metadata(root)?;
    let catalog_metadata_checked = digest_bindings::check_catalog_metadata()?;

    // 2. License & Attribution Completeness
    let retained_components_attributed = attribution::verify_retained_attribution(root)?;
    let workspace_licenses_checked = attribution::verify_workspace_license_policy(root)?;

    // 3. Link Integrity & Completeness
    let documentation_summary = link_integrity::verify_documentation_links(root)?;

    // 4. Roadmap #263 Completion Criteria (1–11)
    let criteria_reports = criteria::verify_all_criteria(root)?;

    Ok(ReleaseQualificationReport {
        upstream_input_metadata_checked,
        upstream_source_metadata_checked,
        catalog_metadata_checked,
        retained_components_attributed,
        workspace_licenses_checked,
        documentation_summary,
        criteria_reports,
    })
}

/// Fails closed until a trusted current candidate-bound completion receipt
/// verifier exists. Repository metadata success cannot qualify a release.
pub fn run_release_qualification(root: &Path) -> Result<ReleaseQualificationReport> {
    audit_repository_metadata(root)?;
    Err(
        "RELEASE UNQUALIFIED: validated current candidate-bound completion receipts unavailable"
            .into(),
    )
}
