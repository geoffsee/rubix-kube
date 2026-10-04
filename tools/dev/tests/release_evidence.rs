//! Integration tests for Gate C16/C17 release evidence, attribution, and checksum manifests.
//!
//! Epic E30 / Issue #126.

use std::fs;
use std::path::Path;

use rubix_dev::release::{
    AttributionRecord, ReleaseNotes, assemble_release_evidence, build_release_package_manifest,
    verify_attribution_completeness, verify_kubeconfig_dual_format_accommodation,
    verify_release_evidence, verify_release_notes_completeness,
};
use rubix_dev::repository_root;

#[test]
fn test_committed_release_evidence_is_valid() {
    let root = repository_root(Path::new(".")).expect("repository root");
    let release_dir = root.join("docs/release");

    assert!(release_dir.is_dir(), "docs/release directory must exist");
    verify_release_evidence(&release_dir).expect("committed release evidence must verify cleanly");
}

#[tokio::test]
async fn test_release_evidence_assembly_in_temp_dir() {
    let root = repository_root(Path::new(".")).expect("repository root");
    let temp_dir = tempfile::tempdir().expect("temp dir");

    assemble_release_evidence(&root, temp_dir.path())
        .await
        .expect("assemble_release_evidence should succeed");
    verify_release_evidence(temp_dir.path())
        .expect("assembled release evidence in temp dir should verify cleanly");
}

#[tokio::test]
async fn test_checksum_manifest_detects_tampered_file() {
    let root = repository_root(Path::new(".")).expect("repository root");
    let temp_dir = tempfile::tempdir().expect("temp dir");

    assemble_release_evidence(&root, temp_dir.path())
        .await
        .expect("assemble");

    // Tamper with release-notes.md
    let notes_path = temp_dir.path().join("release-notes.md");
    let content = fs::read_to_string(&notes_path).expect("read notes");
    fs::write(&notes_path, format!("{content}\n<!-- tampered -->")).expect("write tampered notes");

    let err = verify_release_evidence(temp_dir.path()).expect_err("should detect tampering");
    assert!(
        err.to_string().contains("checksum")
            || err.to_string().contains("digest")
            || err.to_string().contains("disagrees"),
        "error should indicate checksum mismatch: {err}"
    );
}

#[test]
fn test_dual_format_kubeconfig_accommodation() {
    verify_kubeconfig_dual_format_accommodation()
        .expect("both YAML and JSON kubeconfigs must be accommodated identically");
}

#[test]
fn test_manifest_structure_and_matrix_completeness() {
    let manifest = build_release_package_manifest().expect("manifest build");

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.product_name, "rubix-kube");
    assert_eq!(manifest.version, "0.1.0");
    assert_eq!(manifest.node_archives.len(), 16);
    assert_eq!(manifest.management_binaries.len(), 4);
    assert_eq!(manifest.oci_images.len(), 7);

    // Verify Windows is explicitly excluded
    assert!(
        manifest
            .excluded_targets
            .iter()
            .any(|t| t.to_ascii_lowercase().contains("windows")),
        "Windows exclusion must be explicitly declared"
    );
}

#[test]
fn test_release_notes_content_and_disclaimers() {
    let notes = ReleaseNotes::build();
    verify_release_notes_completeness(&notes).expect("release notes completeness");

    let md = notes.to_markdown();
    assert!(
        md.contains("DO NOT claim official CNCF Certified Kubernetes qualification"),
        "must contain CNCF disclaimer"
    );
    assert!(
        md.contains("multi-node"),
        "must declare single-node non-certification for multi-node"
    );
    assert!(md.contains("D01"), "must include D01");
    assert!(md.contains("D11"), "must include D11");
    assert!(
        md.contains("v1.1.8"),
        "must include starting version v1.1.8"
    );
    assert!(
        md.contains("v1.3.3"),
        "must include starting version v1.3.3"
    );
    assert!(
        md.contains("1.10"),
        "must include 1.10 soak ratio threshold"
    );
}

#[test]
fn test_attribution_records_and_inventory() {
    let metadata_output = std::process::Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1"])
        .output()
        .expect("cargo metadata");
    assert!(metadata_output.status.success());

    let meta_str = std::str::from_utf8(&metadata_output.stdout).expect("utf8");
    let attribution = AttributionRecord::build(meta_str).expect("attribution build");
    verify_attribution_completeness(&attribution).expect("attribution completeness");

    let inventory = attribution.to_license_inventory();
    assert!(!inventory.rust_dependencies.is_empty());
    assert!(!inventory.retained_components.is_empty());

    let md = attribution.to_markdown();
    assert!(md.contains("Kubernetes"), "must attribute Kubernetes");
    assert!(md.contains("Kine"), "must attribute Kine");
    assert!(md.contains("containerd"), "must attribute containerd");
}
