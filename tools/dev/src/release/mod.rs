//! Release evidence, checksum manifests, and attribution slice for Gate C16/C17.
//!
//! Complies with Issue #126 (Epic E30.03):
//! - Checksum manifest (`SHA256SUMS`) binding all 16 node variant cells and 4 management artifacts
//! - Upstream license attribution for Kubernetes, Kine, containerd, and OCI image dependencies
//! - Production release notes with version transitions, breaking changes, deprecations, budgets, and disclaimer
//! - Conformance, performance, and state transition qualification reports

pub mod attribution;
pub mod evidence;
pub mod manifest;
pub mod notes;

pub use attribution::{
    AttributedComponent, AttributionRecord, ComponentCategory, verify_attribution_completeness,
};
pub use evidence::{
    PerfQualificationDocument, assemble_release_evidence,
    verify_kubeconfig_dual_format_accommodation, verify_release_evidence,
};
pub use manifest::{
    DISTRIBUTION_VERSION, MANAGEMENT_PREFIX, NODE_PREFIX, assemble_checksum_manifest,
    build_release_package_manifest, canonical_cell_digest, canonical_management_digest, sha256_hex,
    verify_checksum_manifest_binding,
};
pub use notes::{
    DeviationEntry, PerfBudgetSpec, ReleaseNotes, TransitionSpec, verify_release_notes_completeness,
};
