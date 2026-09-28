use std::{fs, path::Path, process::Command};
#[test]
fn api_oracle_cli_rejects_matching_typed_mutations_without_echoing_data() {
    let fixture = rubix_dev::api_json::reviewed().unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("fixture.json");
    fs::write(&path, serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        Command::new(env!("CARGO_BIN_EXE_rubix-api-json"))
            .arg("verify")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let mut changed = fixture;
    for name in ["custom-create", "custom-read"] {
        changed["cases"][name]["spec"]["unknown"]["bool"] = 0.into();
    }
    changed["secret-marker"] = "synthetic-do-not-echo-marker".into();
    fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-api-json"))
        .arg("verify")
        .arg(path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("synthetic-do-not-echo-marker"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("validation failed"));
}
#[test]
fn runtime_and_fetch_require_explicit_disposable_environment_before_mutation() {
    for binary in [
        env!("CARGO_BIN_EXE_rubix-api-json"),
        env!("CARGO_BIN_EXE_rubix-component-boundary"),
    ] {
        for command in ["runtime", "fetch", "stage-sources"] {
            let result = Command::new(binary)
                .arg(command)
                .env_remove("RUBIX_DISPOSABLE_FIXTURE")
                .env_remove("RUBIX_DISPOSABLE_BUILD")
                .output()
                .unwrap();
            assert!(!result.status.success(), "{binary} {command}");
        }
    }
}
#[test]
fn existing_capture_output_is_rejected_before_docker_execution() {
    let temporary = tempfile::tempdir().unwrap();
    let sentinel = temporary.path().join("sentinel");
    fs::write(&sentinel, b"retained").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-api-json"))
        .args(["capture", "--output"])
        .arg(temporary.path())
        .env("PATH", Path::new("/definitely-no-executables"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read(sentinel).unwrap(), b"retained");
}
