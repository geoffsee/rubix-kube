use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
#[test]
fn defaults_cli_selects_each_hash_bound_variant() {
    for name in ["expected.json", "apiserver.expected.json"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubix-defaults"))
            .arg("verify")
            .arg(root().join("tools/defaults").join(name))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap()["status"],
            "unchanged"
        );
    }
}
#[test]
fn resolved_cli_rejects_unreviewed_flag_and_boolean_coercion() {
    let source = root().join("tools/resolved-defaults/expected.json");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_rubix-resolved-defaults"))
            .arg("verify")
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    let original: Value = serde_json::from_slice(&fs::read(source).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    for kind in ["empty", "numeric", "flag"] {
        let mut changed = original.clone();
        match kind {
            "empty" => changed = json!({}),
            "numeric" => changed["controls"]["server_started"] = 0.into(),
            _ => changed["cases"]["default"]["flags_after"]["request-timeout"] = "61s".into(),
        }
        let path = dir.path().join("changed.json");
        fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            !Command::new(env!("CARGO_BIN_EXE_rubix-resolved-defaults"))
                .arg("verify")
                .arg(&path)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}
#[test]
fn invalid_prepared_archives_fail_before_docker_or_output() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad");
    fs::write(&bad, b"bad").unwrap();
    for binary in [
        env!("CARGO_BIN_EXE_rubix-defaults"),
        env!("CARGO_BIN_EXE_rubix-resolved-defaults"),
    ] {
        let output = dir.path().join("output");
        let result = Command::new(binary)
            .args(["capture", "--source-archive"])
            .arg(&bad)
            .arg("--go-archive")
            .arg(&bad)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("identity mismatch"));
        assert!(!output.exists());
    }
}
#[cfg(unix)]
#[test]
fn shared_runner_bounds_logs_and_cleanup_continues_after_settled_command_failures() {
    use rubix_dev::defaults::capture::{OwnedDocker, Runner, arguments};
    use std::{collections::BTreeMap, os::unix::fs::PermissionsExt};
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("logs");
    fs::create_dir(&log).unwrap();
    let mut runner = Runner::new(&log);
    runner.launcher = Some(PathBuf::from(env!("CARGO_BIN_EXE_rubix-defaults")));
    assert!(runner.bounded(&arguments(&["/bin/sh","-c","while :; do printf xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx; done"]),"overflow",3,64).is_err());
    assert!(fs::metadata(log.join("overflow.log")).unwrap().len() <= 64);
    let docker = dir.path().join("docker");
    fs::write(&docker, b"#!/bin/sh\nprintf unavailable >&2\nexit 17\n").unwrap();
    fs::set_permissions(&docker, fs::Permissions::from_mode(0o755)).unwrap();
    let mut environment = std::env::vars().collect::<BTreeMap<_, _>>();
    environment.insert("PATH".into(), dir.path().to_string_lossy().into_owned());
    runner.environment = Some(environment);
    runner.commands.cancellation.request();
    let owned = OwnedDocker {
        tag: "owned-image".into(),
        image_id: None,
        containers: vec!["owned-a".into(), "owned-b".into()],
    };
    let mut report = json!({"containers":owned.containers,"errors":[],"cleanup_errors":[]});
    runner.finish(&mut report, &log, &owned).unwrap();
    assert_eq!(report["cleanup_errors"].as_array().unwrap().len(), 6);
    assert_eq!(report["remaining_containers"], Value::Null);
    assert_eq!(report["remaining_images"], Value::Null);
    assert!(log.join("receipt.json").exists());
    for index in 1..=6 {
        assert!(log.join(format!("cleanup-{index}.command.json")).exists());
    }
}
#[cfg(unix)]
#[test]
fn cancellation_during_owned_cleanup_is_recorded_and_rejects_success() {
    use rubix_dev::defaults::capture::{OwnedDocker, Runner, successful};
    use std::{
        collections::BTreeMap,
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    fs::create_dir(&logs).unwrap();
    let docker = dir.path().join("docker");
    fs::write(&docker, b"#!/bin/sh\nif [ \"$1 $2\" = 'image rm' ]; then : > \"$MARKER\"; while [ ! -f \"$RELEASE\" ]; do :; done; fi\nexit 0\n").unwrap();
    fs::set_permissions(&docker, fs::Permissions::from_mode(0o755)).unwrap();
    let marker = dir.path().join("marker");
    let release = dir.path().join("release");
    let mut environment = std::env::vars().collect::<BTreeMap<_, _>>();
    environment.insert("PATH".into(), dir.path().to_string_lossy().into_owned());
    environment.insert("MARKER".into(), marker.to_string_lossy().into_owned());
    environment.insert("RELEASE".into(), release.to_string_lossy().into_owned());
    let mut runner = Runner::new(&logs);
    runner.environment = Some(environment);
    runner.launcher = Some(PathBuf::from(env!("CARGO_BIN_EXE_rubix-defaults")));
    let cancellation = runner.commands.cancellation.clone();
    let trigger = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !marker.exists() {
            assert!(Instant::now() < deadline, "cleanup handshake timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
        cancellation.request();
        fs::write(release, b"release").unwrap();
    });
    let owned = OwnedDocker {
        tag: "owned-test".into(),
        image_id: None,
        containers: vec![],
    };
    let mut report = json!({"errors":[],"cleanup_errors":[]});
    runner.finish(&mut report, &logs, &owned).unwrap();
    trigger.join().unwrap();
    assert_eq!(report["cancelled"], true);
    assert_eq!(report["cleanup_errors"], json!([]));
    assert_eq!(report["remaining_images"], json!([]));
    assert!(!successful(&report));
}
