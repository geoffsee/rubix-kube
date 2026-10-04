//! Unqualified release fixture diagnostics and observed-byte checksum helpers.
pub mod attribution;
pub mod evidence;
pub mod manifest;
pub mod notes;
pub use attribution::{
    AttributedComponent, AttributionRecord, ComponentCategory, verify_attribution_completeness,
};
pub use evidence::{
    PerfQualificationDocument, StateEvidence, assemble_fixture_evidence, assemble_release_evidence,
    verify_fixture_evidence, verify_kubeconfig_dual_format_accommodation, verify_release_evidence,
};
pub use manifest::{
    DISTRIBUTION_VERSION, MANAGEMENT_PREFIX, NODE_PREFIX, assemble_checksum_manifest,
    build_release_package_manifest, sha256_hex, verify_checksum_inventory,
    verify_checksum_manifest_binding, verify_observed_artifacts,
};
pub use notes::{ReleaseNotes, verify_release_notes_completeness};
