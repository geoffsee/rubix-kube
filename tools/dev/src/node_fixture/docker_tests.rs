use super::{
    build,
    common::{Result, Value, digest, inventory, json, load, read, save, sha256},
    docker,
};
use std::{fmt::Write as _, fs, path::Path};
fn root() -> Result<std::path::PathBuf> {
    rubix_dev::repository_root(&std::env::current_dir()?)
}
fn record(directory: &Path, label: &str, argv: &Value) -> Result<String> {
    let value = json!({"spawned":true,"owned_pid":123,"owner_directory":"/tmp/rubix-process-synthetic","owned_process_group_absent":true,"cleanup_complete":true,"output_eof":true,"cleanup_errors":[],"exit_code":0,"timeout":false,"cancelled":false,"argv":argv,"merged_output":true,"output_limit":false,"stdout_sha256":digest(&directory.join(format!("{label}.log")))?,"stderr_sha256":sha256(b"")});
    let path = directory.join(format!("{label}.command.json"));
    save(&path, &value)?;
    digest(&path)
}
fn staged(directory: &Path, family: &str) -> Result<Value> {
    let nonce = "a".repeat(32);
    let tag = format!("rubix-node-{family}-{nonce}");
    let source = inventory(&root()?, family)?;
    save(&directory.join("source-hashes.json"), &source)?;
    let directory_raw = root()?.join(format!("tools/node-{family}/evidence/first/artifact-build"));
    let mut raw = String::from_utf8(read(
        &directory_raw.join(if directory_raw.join("test.log").exists() {
            "test.log"
        } else {
            "run.log"
        }),
        32 * 1024 * 1024,
    )?)?;
    let original = build::verify_run(&root()?, family, &raw)?;
    let mut hashes = serde_json::Map::new();
    let mut metadata =
        json!({"target":"aarch64-unknown-linux-musl","revision":"b".repeat(40),"files":{}});
    for name in build::binaries(family)? {
        let bytes = name.as_bytes();
        let hash = sha256(bytes);
        raw = raw.replace(original[name].as_str().ok_or("hash")?, &hash);
        fs::write(directory.join(name), bytes)?;
        metadata["files"][name] = json!({"sha256":hash,"size":bytes.len()});
        hashes.insert(name.into(), hash.into());
    }
    let mut build_log = String::new();
    if family == "container" {
        build_log.push_str("#1 0.1 POLICY_TESTS_BEGIN\n#1 0.1 test tests::cancelled_result_publishes_once_and_is_terminal ... ok\n#1 0.1 test tests::uncertain_cleanup_is_quiet_terminal_even_when_observer_is_settled ... ok\n#1 0.1 test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n#1 0.1 POLICY_TESTS_END\n");
    }
    writeln!(build_log, "#1 0.1 RUBIX_BUILD_BIND_BEGIN {nonce}")?;
    for name in build::binaries(family)? {
        writeln!(
            build_log,
            "#1 0.1 {}  /out/{name}",
            hashes[name].as_str().ok_or("hash")?
        )?;
    }
    writeln!(build_log, "#1 0.1 RUBIX_BUILD_BIND_END {nonce}")?;
    fs::write(directory.join("build.log"), &build_log)?;
    fs::write(directory.join("test.log"), &raw)?;
    fs::write(directory.join("create.log"), b"created")?;
    save(&directory.join("artifact.json"), &metadata)?;
    let build_command =
        docker::build_command(family, &tag, &nonce, "/tmp/rubix-node-context-synthetic");
    let run = docker::run_command(family, &tag, "test")?;
    let mut receipt = json!({"schema":3,"family":family,"revision":"b".repeat(40),"dirty":false,"cancelled":false,"tag":tag,"nonce":nonce,"source_sha256":digest(&directory.join("source-hashes.json"))?,"build_command":build_command,"build_sha256":sha256(build_log.as_bytes()),"builder_sha256":hashes,"image_id":format!("sha256:{}","c".repeat(64)),"containers":[format!("{tag}-artifact"),format!("{tag}-test")],"runs":{"test":{"command":run,"raw_sha256":sha256(raw.as_bytes()),"binary_sha256":hashes,"records":{"safe_tests_and_cli":true}}},"commands":{},"errors":[],"cleanup_errors":[],"remaining_containers":[],"remaining_images":[],"artifact_sha256":digest(&directory.join("artifact.json"))?});
    receipt["commands"]["build"] = record(directory, "build", &receipt["build_command"])?.into();
    receipt["commands"]["test"] =
        record(directory, "test", &receipt["runs"]["test"]["command"])?.into();
    receipt["commands"]["create"] = record(
        directory,
        "create",
        &json!(["docker", "create", "--name", format!("{tag}-artifact"), tag]),
    )?
    .into();
    for name in build::binaries(family)? {
        let label = format!("copy-{name}");
        fs::write(directory.join(format!("{label}.log")), b"")?;
        receipt["commands"][&label] = record(
            directory,
            &label,
            &json!([
                "docker",
                "cp",
                format!("{tag}-artifact:/out/{name}"),
                directory.join(name)
            ]),
        )?
        .into();
    }
    for (label, argv) in docker::control_commands(family, &tag) {
        fs::write(
            directory.join(format!("{label}.log")),
            if label == "image-inspect" {
                format!("{}\n", receipt["image_id"].as_str().ok_or("image")?)
            } else {
                String::new()
            },
        )?;
        receipt["commands"][&label] = record(directory, &label, &argv)?.into();
    }
    save(&directory.join("receipt.json"), &receipt)?;
    docker::verify(&root()?, family, directory, true)?;
    Ok(receipt)
}
#[test]
fn complete_build_receipts_bind_every_field_and_artifact() -> Result<()> {
    for family in ["network", "container"] {
        let directory = tempfile::tempdir()?;
        let original = staged(directory.path(), family)?;
        for key in original.as_object().ok_or("receipt")?.keys() {
            let mut changed = original.clone();
            changed.as_object_mut().ok_or("receipt")?.remove(key);
            save(&directory.path().join("receipt.json"), &changed)?;
            assert!(
                docker::verify(&root()?, family, directory.path(), true).is_err(),
                "{key}"
            );
        }
        for (key, value) in [
            ("dirty", json!(true)),
            ("cancelled", json!(true)),
            ("schema", json!(true)),
            ("cleanup_errors", json!(["failed"])),
            ("remaining_images", Value::Null),
            ("source_sha256", json!("0".repeat(64))),
            ("containers", json!([])),
        ] {
            let mut changed = original.clone();
            changed[key] = value;
            save(&directory.path().join("receipt.json"), &changed)?;
            assert!(docker::verify(&root()?, family, directory.path(), true).is_err());
        }
        save(&directory.path().join("receipt.json"), &original)?;
        let name = build::binaries(family)?[0];
        fs::write(directory.path().join(name), b"substitution")?;
        assert!(docker::verify(&root()?, family, directory.path(), true).is_err());
    }
    Ok(())
}
#[test]
fn rehashed_failed_commands_and_false_builder_proofs_are_rejected() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let original = staged(directory.path(), "container")?;
    for label in ["build", "test", "create", "copy-prepare_node_host"] {
        let path = directory.path().join(format!("{label}.command.json"));
        let command = load(&path)?;
        for (key, value) in [
            ("exit_code", json!(1)),
            ("output_eof", json!(false)),
            ("output_limit", json!(true)),
            ("timeout", json!(true)),
            ("cancelled", json!(true)),
            ("owned_process_group_absent", json!(false)),
            ("cleanup_complete", json!(false)),
        ] {
            let mut changed = command.clone();
            changed[key] = value;
            save(&path, &changed)?;
            let mut receipt = original.clone();
            receipt["commands"][label] = digest(&path)?.into();
            save(&directory.path().join("receipt.json"), &receipt)?;
            assert!(docker::verify(&root()?, "container", directory.path(), true).is_err());
        }
        save(&path, &command)?;
    }
    let build = String::from_utf8(read(&directory.path().join("build.log"), 32 * 1024 * 1024)?)?;
    for changed in [
        build.replace(" ... ok", " ... FAILED"),
        build.replace("RUBIX_BUILD_BIND_BEGIN", "LOST_BEGIN"),
        format!("{build}{build}"),
        build.replace(&"a".repeat(32), &"b".repeat(32)),
    ] {
        assert!(build::builder_hashes(&changed, "container", &"a".repeat(32)).is_err());
    }
    Ok(())
}
#[test]
fn failed_daemon_cleanup_cannot_produce_empty_observations() -> Result<()> {
    let mut report =
        json!({"cleanup_errors":[],"remaining_containers":null,"remaining_images":null});
    let mut calls = 0;
    docker::cleanup(&mut report, "network", "owned", |_| {
        calls += 1;
        Err("daemon unavailable".into())
    })?;
    assert_eq!(calls, 5);
    assert!(report["remaining_containers"].is_null());
    assert!(report["remaining_images"].is_null());
    assert_eq!(
        report["cleanup_errors"].as_array().ok_or("errors")?.len(),
        5
    );
    Ok(())
}
#[test]
fn published_assessment_requires_current_rust_capture() -> Result<()> {
    docker::verify(
        &root()?,
        "assessment",
        &root()?.join("tools/node-assessment/rust-evidence"),
        false,
    )?;
    Ok(())
}
#[test]
fn published_network_builds_require_current_rust_capture() -> Result<()> {
    for name in ["first", "repeat"] {
        docker::verify(
            &root()?,
            "network",
            &root()?.join(format!(
                "tools/node-network/rust-evidence/{name}/artifact-build"
            )),
            false,
        )?;
    }
    Ok(())
}
#[test]
fn published_container_builds_require_current_rust_capture() -> Result<()> {
    for name in ["first", "repeat"] {
        docker::verify(
            &root()?,
            "container",
            &root()?.join(format!(
                "tools/node-container/rust-evidence/{name}/artifact-build"
            )),
            false,
        )?;
    }
    Ok(())
}
#[test]
fn probe_profile_binds_builder_runtime_and_containment() -> Result<()> {
    let nonce = "a".repeat(32);
    let directory = root()?.join("tools/parity/fixtures/preflight-probes/evidence");
    let raw = read(
        &directory.join(if directory.join("first.log").exists() {
            "first.log"
        } else {
            "run0.log"
        }),
        1024 * 1024,
    )?;
    let (_, hashes) = rubix_dev::preflight_probes::records(&raw)?;
    let mut log = format!("#1 0.1 RUBIX_BUILD_BIND_BEGIN {nonce}\n");
    for name in ["rubix_platform", "preflight_probe"] {
        writeln!(
            log,
            "#1 0.1 {}  /out/{name}-abcdef",
            hashes[name].as_str().ok_or("hash")?
        )?;
    }
    writeln!(log, "#1 0.1 RUBIX_BUILD_BIND_END {nonce}")?;
    assert_eq!(build::builder_hashes(&log, "probes", &nonce)?, hashes);
    assert!(
        build::builder_hashes(
            &log.replace("preflight_probe-abcdef", "unexpected-abcdef"),
            "probes",
            &nonce
        )
        .is_err()
    );
    let argv = docker::run_command("probes", "owned", "first")?;
    for required in [
        "--memory=256m",
        "--pids-limit=64",
        "--network=none",
        "--cap-drop=ALL",
        "RUBIX_PREFLIGHT_DISPOSABLE=1",
    ] {
        assert!(argv.as_array().ok_or("argv")?.contains(&json!(required)));
    }
    Ok(())
}
#[test]
fn published_probes_require_current_rust_capture() -> Result<()> {
    docker::verify(
        &root()?,
        "probes",
        &root()?.join("tools/parity/fixtures/preflight-probes/rust-evidence"),
        false,
    )?;
    Ok(())
}
#[test]
fn rehashed_daemon_inventory_image_and_cleanup_receipts_cannot_claim_success() -> Result<()> {
    for label in ["image-inspect", "cleanup-1", "cleanup-4", "cleanup-5"] {
        let directory = tempfile::tempdir()?;
        let mut receipt = staged(directory.path(), "network")?;
        let argv = load(&directory.path().join(format!("{label}.command.json")))?["argv"].clone();
        fs::write(
            directory.path().join(format!("{label}.log")),
            b"unexpected-owned-resource\n",
        )?;
        receipt["commands"][label] = record(directory.path(), label, &argv)?.into();
        if label == "cleanup-1" {
            let mut command = load(&directory.path().join(format!("{label}.command.json")))?;
            command["exit_code"] = json!(1);
            save(
                &directory.path().join(format!("{label}.command.json")),
                &command,
            )?;
            receipt["commands"][label] =
                digest(&directory.path().join(format!("{label}.command.json")))?.into();
        }
        save(&directory.path().join("receipt.json"), &receipt)?;
        assert!(
            docker::verify(&root()?, "network", directory.path(), true).is_err(),
            "{label}"
        );
    }
    Ok(())
}
