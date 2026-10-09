//! Integration tests for automated release qualification verification (Gate C16/C17 / Issue #126).

use rubix_dev::release_qualification::{
    attribution, audit_repository_metadata, criteria, digest_bindings, link_integrity,
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
    let report = audit_repository_metadata(&root)?;

    // Print summary to capture formatted audit in test output
    report.print_summary();

    assert!(report.upstream_input_metadata_checked > 0);
    assert!(report.upstream_source_metadata_checked > 0);
    assert!(report.catalog_metadata_checked > 0);
    assert_eq!(report.retained_components_attributed, 17);
    assert!(report.workspace_licenses_checked >= 3);
    assert!(report.documentation_summary.documents_checked >= 8);
    assert!(report.documentation_summary.local_links_verified > 0);
    assert!(report.documentation_summary.broken_links.is_empty());
    assert_eq!(report.criteria_reports.len(), 11);

    for criterion in &report.criteria_reports {
        if criterion.number == 4 {
            assert!(
                criterion.satisfied,
                "Criterion {} ({}) should be satisfied",
                criterion.number, criterion.name
            );
        } else {
            assert!(
                !criterion.satisfied,
                "Criterion {} ({}) should be pending",
                criterion.number, criterion.name
            );
        }
    }

    Ok(())
}

#[test]
fn test_cryptographic_digest_bindings() -> Result<()> {
    let root = root_dir()?;

    // 1. Upstream generator inputs (tools/upstream/inputs.json)
    let inputs_count = digest_bindings::check_upstream_input_metadata(&root)?;
    assert!(inputs_count > 0, "must verify upstream inputs");

    // 2. Upstream provenance sources (docs/architecture/upstream-inputs.json)
    let sources_count = digest_bindings::check_upstream_provenance_metadata(&root)?;
    assert!(sources_count > 0, "must verify upstream provenance sources");

    // 3. Catalog assets
    let catalog_count = digest_bindings::check_catalog_metadata()?;
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
        if res.number == 4 {
            assert!(
                res.satisfied,
                "Criterion {} ({}) should be satisfied: {}",
                res.number, res.name, res.summary
            );
        } else {
            assert!(
                !res.satisfied,
                "Criterion {} ({}) should be pending: {}",
                res.number, res.name, res.summary
            );
        }
    }

    Ok(())
}

#[test]
fn test_link_integrity_fails_closed_on_broken_link() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let doc_file = temp.path().join("test.md");

    fs::write(
        &doc_file,
        b"Check out this [missing link](does-not-exist.md)",
    )?;
    fs::create_dir_all(temp.path().join("docs/architecture"))?;
    fs::copy(&doc_file, temp.path().join("docs/architecture/test.md"))?;
    for document in link_integrity::REQUIRED_DOCUMENTS {
        let path = temp.path().join(document);
        fs::create_dir_all(path.parent().ok_or("missing parent")?)?;
        fs::write(path, b"Required document")?;
    }
    let error = link_integrity::verify_documentation_links(temp.path()).unwrap_err();
    assert!(error.to_string().contains("does-not-exist.md"));
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

#[test]
fn test_attribution_rejects_identity_and_wrong_column_tokens() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let destination = temp.path().join("docs/architecture/attribution.md");
    fs::create_dir_all(destination.parent().ok_or("missing parent")?)?;
    let original = fs::read_to_string(root_dir()?.join("docs/architecture/attribution.md"))?;
    for component in attribution::RETAINED_COMPONENTS {
        let fake_identity = original.replace(
            &format!("`{}`", component.name),
            &format!("`fake-{}`", component.name),
        );
        fs::write(&destination, fake_identity)?;
        let error = attribution::verify_retained_attribution(temp.path()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("missing retained component record")
        );

        let wrong_columns = original
            .lines()
            .map(|line| {
                if line.starts_with(&format!("| `{}`", component.name)) {
                    format!(
                        "| `{}` | WRONG {} {} | UNKNOWN | UNKNOWN | INVALID |",
                        component.name, component.license, component.copyright
                    )
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&destination, wrong_columns)?;
        let error = attribution::verify_retained_attribution(temp.path()).unwrap_err();
        assert!(error.to_string().contains("incomplete attribution fields"));

        for column in [3, 4] {
            let misplaced_field = original
                .lines()
                .map(|line| {
                    if line.starts_with(&format!("| `{}`", component.name)) {
                        let mut cells: Vec<_> = line.split('|').map(str::to_owned).collect();
                        let token = cells[column].clone();
                        cells[2].push_str(&token);
                        cells[column] = " UNKNOWN ".into();
                        cells.join("|")
                    } else {
                        line.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(&destination, misplaced_field)?;
            let error = attribution::verify_retained_attribution(temp.path()).unwrap_err();
            assert!(error.to_string().contains("incomplete attribution fields"));
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn test_artifact_inventory_rejects_symlink_root() -> Result<()> {
    use rubix_assets::ReleasePackager;
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir()?;
    let distribution = temp.path().join("distribution");
    fs::create_dir(&distribution)?;
    fs::write(distribution.join("candidate.tar.gz"), b"artifact")?;
    let checksums = BTreeMap::from([(
        "candidate.tar.gz".to_owned(),
        ReleasePackager::sha256_hex(b"artifact"),
    )]);
    assert_eq!(
        digest_bindings::verify_artifacts_integrity(&distribution, &checksums)?,
        1
    );
    let linked_root = temp.path().join("linked-distribution");
    symlink(&distribution, &linked_root)?;
    let error = digest_bindings::verify_artifacts_integrity(&linked_root, &checksums).unwrap_err();
    assert!(error.to_string().contains("not a symlink"));
    Ok(())
}

#[test]
fn attribution_rejects_component_name_suffixes() -> Result<()> {
    let root = root_dir()?;
    let temp = tempfile::tempdir()?;
    let docs = temp.path().join("docs/architecture");
    fs::create_dir_all(&docs)?;
    let content = fs::read_to_string(root.join("docs/architecture/attribution.md"))?;
    fs::write(
        docs.join("attribution.md"),
        content.replace("`kube-apiserver`", "`kube-apiserver-omitted`"),
    )?;
    let error = attribution::verify_retained_attribution(temp.path()).unwrap_err();
    assert!(error.to_string().contains("missing retained component"));
    Ok(())
}

#[test]
fn attribution_rejects_tokens_in_wrong_columns() -> Result<()> {
    let root = root_dir()?;
    let temp = tempfile::tempdir()?;
    let docs = temp.path().join("docs/architecture");
    fs::create_dir_all(&docs)?;
    let content = fs::read_to_string(root.join("docs/architecture/attribution.md"))?;
    let valid = "| `kube-apiserver` | Official v1.35.7 | Apache-2.0 | The Kubernetes Authors | https://github.com/kubernetes/kubernetes |";
    assert!(
        content.contains(valid),
        "fixture must contain the selected row"
    );
    fs::write(
        docs.join("attribution.md"),
        content.replace(valid, "| `kube-apiserver` | Official v1.35.7 | unknown | unknown | https://github.com/kubernetes/kubernetes Apache-2.0 The Kubernetes Authors |"),
    )?;
    let error = attribution::verify_retained_attribution(temp.path()).unwrap_err();
    assert!(error.to_string().contains("incomplete attribution fields"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn artifact_inventory_rejects_unlisted_unix_socket() -> Result<()> {
    let temp = tempfile::tempdir()?;
    fs::write(temp.path().join("candidate.tar.gz"), b"")?;
    let checksums = BTreeMap::from([(
        "candidate.tar.gz".to_owned(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned(),
    )]);
    assert_eq!(
        digest_bindings::verify_artifacts_integrity(temp.path(), &checksums)?,
        1
    );
    let _listener = std::os::unix::net::UnixListener::bind(temp.path().join("unlisted.sock"))?;
    let error = digest_bindings::verify_artifacts_integrity(temp.path(), &checksums).unwrap_err();
    assert!(error.to_string().contains("nonregular artifact"));
    Ok(())
}
