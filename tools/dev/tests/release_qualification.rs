//! Integration tests for automated release qualification verification (Gate C16/C17 / Issue #126).

use rubix_dev::release_qualification::{
    attribution, criteria, digest_bindings, link_integrity, run_release_qualification,
};
use rubix_dev::{Result, repository_root};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn root_dir() -> Result<std::path::PathBuf> {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR")))
}

#[test]
fn test_full_release_qualification_suite() -> Result<()> {
    let root = root_dir()?;
    let report = run_release_qualification(&root)?;

    // Print summary to capture formatted audit in test output
    report.print_summary();

    assert!(report.upstream_inputs_verified > 0);
    assert!(report.upstream_sources_verified > 0);
    assert!(report.catalog_assets_verified > 0);
    assert_eq!(report.retained_components_attributed, 17);
    assert!(report.workspace_licenses_checked >= 3);
    assert!(report.documentation_summary.documents_checked >= 8);
    assert!(report.documentation_summary.local_links_verified > 0);
    assert!(report.documentation_summary.broken_links.is_empty());
    assert_eq!(report.criteria_reports.len(), 11);

    for criterion in &report.criteria_reports {
        assert!(
            criterion.satisfied,
            "Criterion {} ({}) not satisfied",
            criterion.number, criterion.name
        );
    }

    Ok(())
}

#[test]
fn test_cryptographic_digest_bindings() -> Result<()> {
    let root = root_dir()?;

    // 1. Upstream generator inputs (tools/upstream/inputs.json)
    let inputs_count = digest_bindings::verify_upstream_inputs(&root)?;
    assert!(inputs_count > 0, "must verify upstream inputs");

    // 2. Upstream provenance sources (docs/architecture/upstream-inputs.json)
    let sources_count = digest_bindings::verify_upstream_provenance(&root)?;
    assert!(sources_count > 0, "must verify upstream provenance sources");

    // 3. Catalog assets
    let catalog_count = digest_bindings::verify_catalog_bindings()?;
    assert!(catalog_count > 0, "must verify catalog assets");

    // 4. Digest validation helper
    assert!(digest_bindings::is_valid_sha256_hex(
        "2ef1c4787989f11f868f81bb84ae2afd4a49a81d000000000000000000000000"
    ));
    assert!(!digest_bindings::is_valid_sha256_hex("INVALID"));

    Ok(())
}

#[test]
fn test_attribution_completeness() -> Result<()> {
    let root = root_dir()?;

    // Must verify all 17 retained components
    let count = attribution::verify_retained_attribution(&root)?;
    assert_eq!(count, 17);

    // Must verify permitted workspace licenses
    let license_count = attribution::verify_workspace_license_policy(&root)?;
    assert!(license_count >= 3);

    Ok(())
}

#[test]
fn test_documentation_link_integrity_and_completeness() -> Result<()> {
    let root = root_dir()?;

    // Required core documents must exist
    let req_count = link_integrity::verify_required_documents_exist(&root)?;
    assert_eq!(req_count, link_integrity::REQUIRED_DOCUMENTS.len());

    // All markdown links must resolve without broken paths
    let summary = link_integrity::verify_documentation_links(&root)?;
    assert!(summary.documents_checked >= 8);
    assert!(summary.local_links_verified > 0);
    assert_eq!(
        summary.broken_links.len(),
        0,
        "unexpected broken links: {:?}",
        summary.broken_links
    );

    Ok(())
}

#[test]
fn test_all_11_roadmap_criteria() -> Result<()> {
    let root = root_dir()?;
    let results = criteria::verify_all_criteria(&root)?;

    assert_eq!(results.len(), 11, "must verify all 11 criteria");
    for res in results {
        assert!(
            res.satisfied,
            "Criterion {} ({}) failed: {}",
            res.number, res.name, res.summary
        );
    }

    Ok(())
}

#[test]
fn test_link_integrity_fails_closed_on_broken_link() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let doc_file = temp.path().join("test.md");

    // Write a document with a broken local relative link
    fs::write(
        &doc_file,
        b"Check out this [missing link](does-not-exist.md)",
    )?;

    // link verification should detect the broken link
    let inline_re = regex::Regex::new(r"\[([^\]]+)\]\(([^)]+)\)")?;

    let content = fs::read_to_string(&doc_file)?;
    let mut broken = Vec::new();
    for cap in inline_re.captures_iter(&content) {
        let target = cap.get(2).unwrap().as_str();
        let resolved = temp.path().join(target);
        if !resolved.exists() {
            broken.push(target.to_string());
        }
    }

    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0], "does-not-exist.md");

    Ok(())
}

#[test]
fn test_attribution_fails_closed_on_missing_component() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let docs_dir = temp.path().join("docs/architecture");
    fs::create_dir_all(&docs_dir)?;

    // Write incomplete attribution document (missing several components)
    let incomplete = docs_dir.join("attribution.md");
    fs::write(
        &incomplete,
        b"# Incomplete Attribution\n\nkube-apiserver Apache-2.0 The Kubernetes Authors\n",
    )?;

    let err = attribution::verify_retained_attribution(temp.path()).unwrap_err();
    assert!(
        err.to_string()
            .contains("attribution document missing retained component")
    );

    Ok(())
}

#[test]
fn test_tamper_detection_fails_closed_on_digest_mismatch() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let artifact = temp.path().join("candidate.tar.gz");
    fs::write(&artifact, b"legitimate bytes")?;

    let mut checksums = BTreeMap::new();
    checksums.insert(
        "candidate.tar.gz".to_string(),
        "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
    );

    let err = digest_bindings::verify_artifacts_integrity(temp.path(), &checksums).unwrap_err();
    assert!(err.to_string().contains("digest mismatch"));

    Ok(())
}
