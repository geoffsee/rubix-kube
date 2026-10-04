//! Regression coverage for falsely qualified metadata and unsafe inventories.
use rubix_dev::release_qualification::{attribution, criteria, digest_bindings};
use rubix_dev::{Result, repository_root, sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

fn put(root: &Path, path: &str, bytes: impl AsRef<[u8]>) -> Result<()> {
    let path = root.join(path);
    fs::create_dir_all(path.parent().ok_or("missing parent")?)?;
    fs::write(path, bytes)?;
    Ok(())
}

#[test]
fn absent_and_explicitly_unqualified_evidence_never_satisfies_criteria() -> Result<()> {
    let temp = tempfile::tempdir()?;
    assert!(
        criteria::verify_all_criteria(temp.path())?
            .iter()
            .all(|status| !status.satisfied)
    );
    put(
        temp.path(),
        "tools/perf/README.md",
        "paired budgets NOT QUALIFIED: synthetic fixtures only",
    )?;
    put(temp.path(), "tools/dev/src/bin/rubix-perf.rs", "")?;
    put(
        temp.path(),
        "docs/architecture/recovery-lifecycle-qualification.md",
        "NOT QUALIFIED; no capture performed",
    )?;
    put(temp.path(), "tools/dev/src/bin/rubix-conformance.rs", "")?;
    assert!(!criteria::check_criterion_7_performance_budgets(temp.path())?.satisfied);
    assert!(!criteria::check_criterion_6_conformance_and_soak(temp.path())?.satisfied);
    assert!(!criteria::check_criterion_10_artifact_digest_bindings(temp.path())?.satisfied);
    Ok(())
}

#[test]
fn qualification_cli_fails_closed_and_metadata_mode_is_explicitly_unqualified() -> Result<()> {
    let binary = env!("CARGO_BIN_EXE_rubix-qualification");
    let qualification = Command::new(binary).output()?;
    assert!(!qualification.status.success());
    let output = String::from_utf8(qualification.stderr)?;
    assert!(output.contains("RELEASE UNQUALIFIED"));
    assert!(!output.contains("ALL QUALIFICATION GATES PASSED"));
    let metadata = Command::new(binary).arg("--metadata-only").output()?;
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    assert!(String::from_utf8(metadata.stdout)?.contains("RELEASE REMAINS UNQUALIFIED"));
    assert!(!Command::new(binary).arg("--qualify").status()?.success());
    Ok(())
}

#[test]
fn component_fields_cannot_be_borrowed_from_neighbor_records() -> Result<()> {
    let root = repository_root(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let text = fs::read_to_string(root.join("docs/architecture/attribution.md"))?;
    let original = text
        .lines()
        .find(|line| line.starts_with("| `kube-apiserver` |"))
        .ok_or("missing row")?;
    for changed in [
        "| `kube-apiserver` | unknown | unknown | unknown | missing |",
        "| `kube-apiserver` | Official v1.35.7 | Apache-2.0 | The Kubernetes Authors | missing |",
        "| `kube-apiserver` | unknown | Apache-2.0 | The Kubernetes Authors | https://github.com/kubernetes/kubernetes |",
    ] {
        let temp = tempfile::tempdir()?;
        put(
            temp.path(),
            "docs/architecture/attribution.md",
            text.replace(original, changed),
        )?;
        assert!(attribution::verify_retained_attribution(temp.path()).is_err());
    }
    let temp = tempfile::tempdir()?;
    put(
        temp.path(),
        "docs/architecture/attribution.md",
        format!("{text}\n{original}"),
    )?;
    assert!(attribution::verify_retained_attribution(temp.path()).is_err());
    Ok(())
}

#[test]
fn artifact_inventory_rejects_traversal_absolute_aliases_and_unlisted_files() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dist = temp.path().join("dist");
    fs::create_dir(&dist)?;
    fs::write(temp.path().join("outside"), b"outside")?;
    fs::write(dist.join("candidate"), b"candidate")?;
    for name in [
        "../outside".to_string(),
        temp.path().join("outside").display().to_string(),
        "./candidate".into(),
        "nested/candidate".into(),
        "nested\\candidate".into(),
        ".".into(),
        String::new(),
    ] {
        let sums = BTreeMap::from([(name, sha256(b"outside"))]);
        assert!(digest_bindings::verify_artifacts_integrity(&dist, &sums).is_err());
    }
    let sums = BTreeMap::from([("candidate".into(), sha256(b"candidate"))]);
    assert_eq!(
        digest_bindings::verify_artifacts_integrity(&dist, &sums)?,
        1
    );
    fs::write(dist.join("unlisted"), b"unlisted")?;
    assert!(digest_bindings::verify_artifacts_integrity(&dist, &sums).is_err());
    fs::remove_file(dist.join("unlisted"))?;
    fs::remove_file(dist.join("candidate"))?;
    assert!(digest_bindings::verify_artifacts_integrity(&dist, &sums).is_err());
    fs::create_dir(dist.join("candidate"))?;
    assert!(digest_bindings::verify_artifacts_integrity(&dist, &sums).is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn artifact_inventory_rejects_file_and_root_symlinks_and_fifos() -> Result<()> {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir()?;
    let dist = temp.path().join("dist");
    fs::create_dir(&dist)?;
    fs::write(temp.path().join("outside"), b"outside")?;
    symlink(temp.path().join("outside"), dist.join("candidate"))?;
    let sums = BTreeMap::from([("candidate".into(), sha256(b"outside"))]);
    assert!(digest_bindings::verify_artifacts_integrity(&dist, &sums).is_err());
    fs::remove_file(dist.join("candidate"))?;
    assert!(
        Command::new("mkfifo")
            .arg(dist.join("candidate"))
            .status()?
            .success()
    );
    assert!(digest_bindings::verify_artifacts_integrity(&dist, &sums).is_err());
    fs::remove_file(dist.join("candidate"))?;
    fs::write(dist.join("candidate"), b"outside")?;
    symlink(&dist, temp.path().join("alias"))?;
    assert!(
        digest_bindings::verify_artifacts_integrity(&temp.path().join("alias"), &sums).is_err()
    );
    Ok(())
}
