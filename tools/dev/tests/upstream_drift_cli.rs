//! End-to-end maintenance CLI exit codes, read-only reports, and explicit parser integration.
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, io::Write, path::Path, process::Command};
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn snapshot(directory: &Path) {
    fs::create_dir_all(directory).unwrap();
    for (source, destination) in [
        ("tools/drift/inventory.json.gz", "inventory.json.gz"),
        ("tools/defaults/expected.json", "defaults.json"),
        (
            "tools/defaults/apiserver.expected.json",
            "apiserver-defaults.json",
        ),
    ] {
        fs::copy(root().join(source), directory.join(destination)).unwrap();
    }
}
fn command(before: &Path, after: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rubix-drift"));
    cmd.arg("report")
        .arg("--before")
        .arg(before)
        .arg("--after")
        .arg(after);
    cmd
}
fn bytes(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}
#[test]
fn report_exit_codes_formats_and_input_bytes_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let before = dir.path().join("before");
    let after = dir.path().join("after");
    snapshot(&before);
    snapshot(&after);
    let output = command(&before, &after).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["status"],
        "unchanged"
    );
    let path = after.join("defaults.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["cases"]["rust_migration_probe"] = json!({"nested":"```\n# injected"});
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let original = bytes(&after);
    let output = command(&before, &after)
        .args(["--format", "markdown"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Component defaults"));
    assert!(text.contains("/defaults/rust_migration_probe"));
    assert!(!text.contains("\n# injected"));
    assert_eq!(bytes(&after), original);
    fs::remove_file(path).unwrap();
    assert_eq!(
        command(&before, &after).output().unwrap().status.code(),
        Some(2)
    );
}
#[test]
fn reports_reject_truncated_gzip_duplicates_and_one_sided_resolved_inputs() {
    let dir = tempfile::tempdir().unwrap();
    snapshot(dir.path());
    let path = dir.path().join("inventory.json.gz");
    let mut raw = fs::read(&path).unwrap();
    raw.truncate(raw.len() - 8);
    fs::write(&path, raw).unwrap();
    assert_eq!(
        command(dir.path(), dir.path())
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    snapshot(dir.path());
    fs::write(
        dir.path().join("defaults.json"),
        b"{\"schema_version\":1,\"schema_version\":1}",
    )
    .unwrap();
    assert_eq!(
        command(dir.path(), dir.path())
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    snapshot(dir.path());
    assert_eq!(
        command(dir.path(), dir.path())
            .arg("--before-resolved")
            .arg(dir.path())
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}
#[test]
fn combined_resolved_report_rejects_unbound_and_detects_rehashed_removal() {
    let dir = tempfile::tempdir().unwrap();
    let before = dir.path().join("before");
    let after = dir.path().join("after");
    for directory in [&before, &after] {
        snapshot(directory);
        fs::copy(
            root().join("tools/resolved-defaults/evidence/run0.json"),
            directory.join("resolved.json"),
        )
        .unwrap();
        fs::copy(
            root().join("tools/resolved-defaults/evidence/receipt.json"),
            directory.join("receipt.json"),
        )
        .unwrap();
    }
    let run = || {
        let mut cmd = command(&before, &after);
        cmd.arg("--before-resolved")
            .arg(&before)
            .arg("--after-resolved")
            .arg(&after);
        cmd.output().unwrap()
    };
    assert!(run().status.success());
    let mut value: Value =
        serde_json::from_slice(&fs::read(after.join("resolved.json")).unwrap()).unwrap();
    value["cases"]["default"]["flags_after"]
        .as_object_mut()
        .unwrap()
        .remove("secure-port");
    let raw = serde_json::to_vec(&value).unwrap();
    fs::write(after.join("resolved.json"), &raw).unwrap();
    assert_eq!(run().status.code(), Some(2));
    let mut receipt: Value =
        serde_json::from_slice(&fs::read(after.join("receipt.json")).unwrap()).unwrap();
    for name in ["run0.json", "run1.json"] {
        receipt["output_sha256"][name] = rubix_dev::sha256(&raw).into();
    }
    fs::write(
        after.join("receipt.json"),
        serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();
    let result = run();
    assert_eq!(
        result.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["removal_count"], 1);
    assert_eq!(
        report["changes"]["resolved_apiserver_options"][0]["kind"],
        "removed"
    );
    assert!(
        report["snapshot_sha256"]["after"]
            .get("resolved/resolved.json")
            .is_some()
    );
}
#[test]
fn upstream_invalid_command_and_unprepared_cache_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    for command in [
        "verify",
        "check-cri",
        "check-containerd",
        "check-kubernetes-bindings",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubix-upstream"))
            .arg(command)
            .arg("--cache-dir")
            .arg(dir.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
    }
    assert_eq!(
        Command::new(env!("CARGO_BIN_EXE_rubix-upstream"))
            .arg("unknown")
            .output()
            .unwrap()
            .status
            .code(),
        Some(1)
    );
}
#[cfg(unix)]
#[test]
fn corrupt_download_never_commits_a_blob() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("curl");
    let mut file = fs::File::create(&binary).unwrap();
    file.write_all(b"#!/bin/sh\nwhile [ \"$#\" -gt 0 ]; do\nif [ \"$1\" = --output ]; then shift; printf incorrect > \"$1\"; exit 0; fi\nshift\ndone\nexit 2\n").unwrap();
    drop(file);
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let cache = dir.path().join("cache");
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-upstream"))
        .arg("fetch")
        .arg("--cache-dir")
        .arg(&cache)
        .env("PATH", dir.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("SHA-256 mismatch"));
    assert!(!cache.exists());
}
