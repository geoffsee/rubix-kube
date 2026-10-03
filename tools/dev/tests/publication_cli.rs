use std::{fs, process::Command};

fn complete_names(dir: &std::path::Path) {
    for variant in rubix_assets::Matrix::all_node_variants() {
        fs::write(
            dir.join(variant.archive_filename("rubix-kube", "v1.0.0")),
            b"node",
        )
        .unwrap();
    }
    for target in rubix_assets::Matrix::all_management_targets() {
        fs::write(dir.join(target.binary_filename("rubixctl")), b"cli").unwrap();
    }
}

#[test]
fn unexpected_artifacts_and_missing_required_inventories_fail_before_writes() {
    let dir = tempfile::tempdir().unwrap();
    complete_names(dir.path());
    fs::write(dir.path().join("unexpected.bin"), b"unrecognized").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-provenance"))
        .arg(dir.path())
        .arg("v1.0.0")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized release artifact"));
    fs::remove_file(dir.path().join("unexpected.bin")).unwrap();
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-provenance"))
        .arg(dir.path())
        .arg("v1.0.0")
        .arg(repository)
        .arg(dir.path().join("missing-inventories"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    for filename in [
        "SHA256SUMS",
        "release-manifest.json",
        "provenance.json",
        "licenses.json",
    ] {
        assert!(!dir.path().join(filename).exists());
    }
}

#[test]
fn raw_native_binaries_fail_before_any_publication_metadata_is_written() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("rubix-kube"), b"native node").unwrap();
    fs::write(dir.path().join("rubixctl"), b"native management").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-provenance"))
        .arg(dir.path())
        .arg("v1.0.0")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("node archive cell count"));
    for filename in [
        "SHA256SUMS",
        "release-manifest.json",
        "provenance.json",
        "licenses.json",
    ] {
        assert!(!dir.path().join(filename).exists(), "{filename}");
    }
}

#[test]
fn matrix_rejects_a_self_declared_foreign_product() {
    use rubix_assets::{ManagementArtifact, Matrix, NodeArchiveArtifact, ReleasePackageManifest};
    let dir = tempfile::tempdir().unwrap();
    let manifest = ReleasePackageManifest {
        schema_version: 1,
        product_name: "foreign".into(),
        version: "v1.0.0".into(),
        node_archives: Matrix::all_node_variants()
            .iter()
            .map(|v| NodeArchiveArtifact {
                cell: v.cell,
                filename: v.archive_filename("foreign", "v1.0.0"),
                architecture: "amd64".into(),
                libc: "glibc".into(),
                variant: "online".into(),
                size_bytes: 1,
                sha256: "0".repeat(64),
                bundled_assets: vec![],
            })
            .collect(),
        management_binaries: Matrix::all_management_targets()
            .iter()
            .map(|t| ManagementArtifact {
                os: t.os.to_string(),
                architecture: t.architecture.to_string(),
                filename: t.binary_filename("rubixctl"),
                size_bytes: 1,
                sha256: "0".repeat(64),
            })
            .collect(),
        oci_images: vec![],
        excluded_targets: vec!["windows".into()],
    };
    let path = dir.path().join("manifest.json");
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-matrix"))
        .arg("verify-manifest")
        .arg(path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("prefix"));
}
