use std::{fs, process::Command};

#[path = "common/publication.rs"]
mod publication;

fn complete_names(dir: &std::path::Path) {
    let manifest = publication::candidate(dir, "v1.0.0");
    fs::write(
        dir.join("release-manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("archive inventory"));
    for filename in ["SHA256SUMS", "provenance.json", "licenses.json"] {
        assert!(!dir.path().join(filename).exists());
    }
    assert_eq!(
        fs::read(dir.path().join("release-manifest.json")).unwrap(),
        serde_json::to_vec(&publication::candidate(dir.path(), "v1.0.0")).unwrap()
    );
}

#[test]
fn prepared_manifest_file_digests_are_checked_even_without_layout_inventories() {
    let dir = tempfile::tempdir().unwrap();
    complete_names(dir.path());
    let before = fs::read(dir.path().join("release-manifest.json")).unwrap();
    fs::write(dir.path().join("rubixctl-linux-amd64"), b"tampered cli").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-provenance"))
        .arg(dir.path())
        .arg("v1.0.0")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("disagrees with the checksum manifest")
    );
    assert_eq!(
        fs::read(dir.path().join("release-manifest.json")).unwrap(),
        before
    );
    for name in ["SHA256SUMS", "provenance.json", "licenses.json"] {
        assert!(!dir.path().join(name).exists());
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing prepared release manifest"));
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid release metadata"));
}
