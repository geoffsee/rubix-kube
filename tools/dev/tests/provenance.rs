use rubix_dev::provenance::{
    ChecksumError, ChecksumManifest, generate_license_inventory, generate_manifest,
    generate_source_provenance,
};
use std::fs;

const GENERATOR: &str = r#"{"sources":[{"id":"openapi","path":"a","url":"https://x/a",
"sha256":"483500149ee52ce5753d75f5639101d985bb4f5e902cc05b1ba7627465d62446","bytes":3}]}"#;
const INVENTORY: &str = r#"{"baseline":"2ef1c47","sources":[{"repository":"https://github.com/k/k",
"tag":"v1","commit":"abc"}]}"#;

fn dist() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("rubix-kube-0.1.0-linux-amd64.tar.gz"),
        b"node",
    )
    .unwrap();
    fs::write(dir.path().join("other.bin"), b"other").unwrap();
    dir
}

#[test]
fn checksums_round_trip_and_are_deterministic() {
    let dir = dist();
    let first = ChecksumManifest::generate(dir.path()).unwrap();
    let second = ChecksumManifest::generate(dir.path()).unwrap();
    assert_eq!(first.render(), second.render());
    assert_eq!(ChecksumManifest::parse(&first.render()).unwrap(), first);
    first.verify(dir.path()).unwrap();
}

#[test]
fn tampering_missing_and_unlisted_files_are_detected() {
    let dir = dist();
    let manifest = ChecksumManifest::generate(dir.path()).unwrap();

    fs::write(dir.path().join("other.bin"), b"tampered").unwrap();
    let error = manifest.verify(dir.path()).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ChecksumError>(),
        Some(ChecksumError::Mismatch { file, .. }) if file == "other.bin"
    ));

    fs::remove_file(dir.path().join("other.bin")).unwrap();
    let error = manifest.verify(dir.path()).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ChecksumError>(),
        Some(ChecksumError::Missing(_))
    ));

    fs::write(dir.path().join("other.bin"), b"other").unwrap();
    fs::write(dir.path().join("extra.bin"), b"x").unwrap();
    let error = manifest.verify(dir.path()).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ChecksumError>(),
        Some(ChecksumError::Unlisted(_))
    ));
}

#[test]
fn malformed_checksum_lines_are_rejected() {
    for text in [
        "nothex  file",
        "abc  file",
        &format!("{}  ../escape", "0".repeat(64)),
        &format!("{} file", "0".repeat(64)),
    ] {
        assert!(ChecksumManifest::parse(text).is_err(), "{text}");
    }
    let line = format!("{}  a", "0".repeat(64));
    assert!(ChecksumManifest::parse(&format!("{line}\n{line}\n")).is_err());
}

#[test]
fn provenance_ties_inputs_to_artifact_digests_without_overclaiming() {
    let dir = dist();
    let checksums = ChecksumManifest::generate(dir.path()).unwrap();
    let provenance = generate_source_provenance(GENERATOR, INVENTORY, "0.1.0", &checksums).unwrap();
    assert_eq!(provenance.artifacts.len(), 2);
    assert_eq!(provenance.generated_inputs.len(), 1);
    assert_eq!(provenance.upstream_sources[0].commit, "abc");
    assert!(
        provenance
            .retained_components
            .iter()
            .any(|c| c.id == "Kine")
    );
    assert!(!provenance.reproducibility.bit_for_bit_claimed);
    assert!(
        !provenance
            .reproducibility
            .remaining_nondeterminism
            .is_empty()
    );

    let bad = GENERATOR.replace("483500", "zz3500");
    assert!(generate_source_provenance(&bad, INVENTORY, "0.1.0", &checksums).is_err());
    assert!(generate_source_provenance("{}", INVENTORY, "0.1.0", &checksums).is_err());
}

#[test]
fn license_inventory_is_sorted_and_marks_unrecorded() {
    let metadata = r#"{"packages":[{"name":"b","version":"1","license":"MIT"},
{"name":"a","version":"2","license":null}]}"#;
    let inventory = generate_license_inventory(metadata).unwrap();
    assert_eq!(inventory.rust_dependencies[0].name, "a");
    assert_eq!(inventory.rust_dependencies[0].license, "unrecorded");
    assert_eq!(inventory.rust_dependencies[1].license, "MIT");
    assert!(
        inventory
            .retained_components
            .iter()
            .any(|c| c.name == "Containerd")
    );
}

#[test]
fn manifest_generation_uses_real_digests_and_ignores_foreign_versions() {
    let dir = dist();
    fs::write(
        dir.path().join("rubix-kube-9.9.9-linux-amd64.tar.gz"),
        b"old",
    )
    .unwrap();
    let manifest = generate_manifest(dir.path(), "0.1.0", "rubix-kube", "rubixctl").unwrap();
    assert!(manifest.node_archives.len() <= 1);
    for archive in &manifest.node_archives {
        assert_eq!(archive.sha256.len(), 64);
    }
    // Incomplete candidates fail full matrix verification instead of passing silently.
    assert!(
        rubix_assets::ReleasePackager::verify_release_manifest(
            &manifest,
            "rubix-kube",
            "rubixctl",
            "0.1.0"
        )
        .is_err()
    );
}
