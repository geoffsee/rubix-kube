//! Semantic checks on settled command receipts, independent of their hashes.
use super::{Path, Result, Value, equal, json, load, read, require};
pub(crate) fn verify_command(
    directory: &Path,
    label: &str,
    argv: &[String],
    exits: &[i64],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let receipt = load(&directory.join(format!("{label}.command.json")))?;
    for (key, value) in [
        ("spawned", json!(true)),
        ("owned_process_group_absent", json!(true)),
        ("cleanup_complete", json!(true)),
        ("output_eof", json!(true)),
        ("cleanup_errors", json!([])),
        ("timeout", json!(false)),
        ("cancelled", json!(false)),
        ("merged_output", json!(false)),
        ("output_limit", json!(false)),
    ] {
        equal(&receipt[key], &value, &format!("{label} {key}"))?;
    }
    require(
        receipt["owned_pid"]
            .as_u64()
            .is_some_and(|pid| pid > 0 && i32::try_from(pid).is_ok()),
        "owned command PID",
    )?;
    require(
        receipt["owner_directory"]
            .as_str()
            .is_some_and(|path| Path::new(path).is_absolute()),
        "command owner metadata",
    )?;
    equal(
        &receipt["argv"],
        &json!(argv),
        &format!("{label} actual argv"),
    )?;
    require(
        receipt["exit_code"]
            .as_i64()
            .is_some_and(|code| exits.contains(&code)),
        format!("{label} expected exit"),
    )?;
    let stdout = read(&directory.join(format!("{label}.stdout")), 256 * 1024)?;
    let stderr = read(&directory.join(format!("{label}.stderr")), 256 * 1024)?;
    equal(
        &receipt["stdout_sha256"],
        &json!(rubix_dev::sha256(&stdout)),
        "raw command stdout",
    )?;
    equal(
        &receipt["stderr_sha256"],
        &json!(rubix_dev::sha256(&stderr)),
        "raw command stderr",
    )?;
    Ok((stdout, stderr))
}
pub(crate) fn ssh(report: &Value) -> Result<Vec<String>> {
    let private = report["owned_temporary_directory"]
        .as_str()
        .ok_or("private path")?;
    let port = report["ssh_forward"]
        .as_str()
        .ok_or("port")?
        .strip_prefix("127.0.0.1:")
        .ok_or("loopback SSH")?;
    let mut args = vec![
        "ssh".into(),
        "-F".into(),
        "/dev/null".into(),
        "-i".into(),
        format!("{private}/client"),
    ];
    for option in [
        "BatchMode=yes".into(),
        "IdentitiesOnly=yes".into(),
        "IdentityAgent=none".into(),
        "GlobalKnownHostsFile=/dev/null".into(),
        "StrictHostKeyChecking=yes".into(),
        format!("UserKnownHostsFile={private}/known_hosts"),
        "ConnectTimeout=3".into(),
        "ConnectionAttempts=1".into(),
        "ServerAliveInterval=5".into(),
        "ServerAliveCountMax=2".into(),
    ] {
        args.push("-o".into());
        args.push(option);
    }
    args.extend(["-p".into(), port.into(), "root@127.0.0.1".into()]);
    Ok(args)
}
pub(crate) fn remote(
    directory: &Path,
    report: &Value,
    label: &str,
    command: &str,
    exits: &[i64],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut args = ssh(report)?;
    args.push(command.into());
    verify_command(directory, label, &args, exits)
}
#[allow(clippy::too_many_lines, reason = "Exact lifecycle command contract")]
pub(crate) fn shared(directory: &Path, report: &Value, inputs: &Value, reboot: bool) -> Result<()> {
    let private = report["owned_temporary_directory"]
        .as_str()
        .ok_or("private path")?;
    let (version, _) = verify_command(
        directory,
        "qemu-version",
        &["qemu-system-aarch64".into(), "--version".into()],
        &[0],
    )?;
    equal(
        &json!(String::from_utf8(version)?),
        &report["qemu_version"],
        "raw QEMU version",
    )?;
    let image = report["base_image_path"]
        .as_str()
        .ok_or("base image path")?;
    require(
        Path::new(image).is_absolute(),
        "absolute verified image cache path",
    )?;
    require(
        Path::new(image).file_name().and_then(|s| s.to_str())
            == inputs["image"]["url"]
                .as_str()
                .and_then(|s| s.rsplit('/').next()),
        "pinned image basename",
    )?;
    verify_command(
        directory,
        "overlay",
        &[
            "qemu-img".into(),
            "create".into(),
            "-f".into(),
            "qcow2".into(),
            "-F".into(),
            "qcow2".into(),
            "-b".into(),
            image.into(),
            format!("{private}/disk.qcow2"),
            "8G".into(),
        ],
        &[0],
    )?;
    for name in ["client", "host"] {
        verify_command(
            directory,
            &format!("key-{name}"),
            &[
                "ssh-keygen".into(),
                "-q".into(),
                "-t".into(),
                "ed25519".into(),
                "-N".into(),
                String::new(),
                "-C".into(),
                format!("rubix-alpine-fixture-{name}"),
                "-f".into(),
                format!("{private}/{name}"),
            ],
            &[0],
        )?;
    }
    readiness(
        directory,
        report,
        "ssh-readiness",
        "ssh_readiness_label",
        "true",
        false,
    )?;
    let (raw, _) = remote(
        directory,
        report,
        "cloud-init",
        "cloud-init status --wait --long --format json",
        &[report["cloud_init_exit"].as_i64().ok_or("cloud exit")?],
    )?;
    equal(
        &rubix_dev::json::parse(&raw)?,
        &report["cloud_init"],
        "actual cloud observation",
    )?;
    let (raw, _) = remote(
        directory,
        report,
        "environment",
        "uname -a; id; cat /etc/os-release; cat /proc/self/cgroup",
        &[0],
    )?;
    equal(
        &json!(String::from_utf8(raw)?),
        &report["environment"],
        "actual guest environment",
    )?;
    remote(
        directory,
        report,
        "privileged-probe",
        "mkdir -p /mnt/rubix-probe && mount -t tmpfs -o size=1m tmpfs /mnt/rubix-probe && umount /mnt/rubix-probe && rmdir /mnt/rubix-probe",
        &[0],
    )?;
    remote(directory, report, "guest-poweroff", "poweroff", &[0, 255])?;
    if reboot {
        remote(
            directory,
            report,
            "before-reboot-id",
            "cat /proc/sys/kernel/random/boot_id",
            &[0],
        )?;
        remote(directory, report, "guest-reboot", "reboot", &[0, 255])?;
        readiness(
            directory,
            report,
            "reboot-readiness",
            "reboot_readiness_label",
            "cat /proc/sys/kernel/random/boot_id",
            true,
        )?;
        remote(
            directory,
            report,
            "reboot-cloud-init",
            "cloud-init status --wait --long --format json",
            &[report["reboot_cloud_init_exit"]
                .as_i64()
                .ok_or("reboot cloud exit")?],
        )?;
    }
    Ok(())
}
fn readiness(
    directory: &Path,
    report: &Value,
    prefix: &str,
    key: &str,
    command: &str,
    reboot: bool,
) -> Result<()> {
    let label = report[key].as_str().ok_or("readiness label")?;
    let attempt = label
        .strip_prefix(&format!("{prefix}-"))
        .ok_or("readiness prefix")?
        .parse::<usize>()?;
    require(
        (1..=180).contains(&attempt) && label == format!("{prefix}-{attempt:03}"),
        "bounded readiness attempt sequence",
    )?;
    for index in 1..=attempt {
        let candidate = format!("{prefix}-{index:03}");
        let (raw, _) = remote(
            directory,
            report,
            &candidate,
            command,
            if index == attempt { &[0] } else { &[0, 255] },
        )?;
        if index == attempt && reboot {
            equal(
                &json!(rubix_dev::sha256(&raw)),
                &json!(rubix_dev::sha256(&read(
                    &directory.join("reboot-readiness.stdout"),
                    256 * 1024
                )?)),
                "selected reboot readiness raw bytes",
            )?;
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "Exact ordered family input and command contract"
)]
pub(super) fn alpine(
    directory: &Path,
    report: &Value,
    root: &Path,
    profile: &str,
    inputs: &Value,
) -> Result<()> {
    let here = super::fixture(
        root,
        if profile == "alpine" {
            "alpine-preparation"
        } else {
            "alpine-rust-preparation"
        },
    );
    let binary = if profile == "alpine" {
        "preflight.test"
    } else {
        "rubixctl"
    };
    let mut expected = super::BTreeMap::new();
    for (name, pin) in inputs["packages"]["selected"]
        .as_object()
        .ok_or("package pins")?
    {
        expected.insert(
            format!("repo/aarch64/{name}"),
            pin["sha256"].as_str().ok_or("package digest")?.to_owned(),
        );
    }
    expected.insert(
        "repo/aarch64/APKINDEX.tar.gz".into(),
        inputs["packages"]["index_sha256"]
            .as_str()
            .ok_or("index hash")?
            .to_owned(),
    );
    expected.insert(
        binary.into(),
        inputs[if profile == "alpine" {
            "oracle"
        } else {
            "artifact"
        }][if profile == "alpine" {
            "binary_sha256"
        } else {
            "sha256"
        }]
        .as_str()
        .ok_or("binary hash")?
        .to_owned(),
    );
    if profile != "alpine" {
        expected.insert(
            "service-double.sh".into(),
            super::digest(&here.join("service-double.sh"))?,
        );
    }
    let mut checks = String::new();
    let mut checked = String::new();
    for (name, hash) in expected {
        use std::fmt::Write as _;
        writeln!(&mut checks, "{hash}  {name}")?;
        writeln!(&mut checked, "{name}: OK")?;
    }
    for (label, command, input) in [
        (
            "verify-guest-inputs",
            format!("cd /tmp/rubix-bundle && sha256sum -c - && chmod 0755 {binary}"),
            checks.into_bytes(),
        ),
        (
            "baseline-inventory",
            "RUBIX_RUN_PREPARATION=1 sh -s".into(),
            read(&here.join("guest.sh"), 256 * 1024)?,
        ),
        (
            "reboot-verification",
            "sh -s".into(),
            read(&here.join("reboot.sh"), 256 * 1024)?,
        ),
    ] {
        let (stdout, _) = remote(directory, report, label, &command, &[0])?;
        if label == "verify-guest-inputs" {
            require(
                stdout == checked.as_bytes(),
                "all uploaded guest inputs verified",
            )?;
        }
        require(
            read(&directory.join(format!("{label}.stdin")), 256 * 1024)? == input,
            format!("{label} exact script input"),
        )?;
    }
    let mut argv = ssh(report)?;
    argv[0] = "scp".into();
    argv.truncate(argv.len() - 3);
    let private = report["owned_temporary_directory"]
        .as_str()
        .ok_or("private")?;
    let port = report["ssh_forward"]
        .as_str()
        .ok_or("port")?
        .strip_prefix("127.0.0.1:")
        .ok_or("loopback")?;
    argv.extend([
        "-P".into(),
        port.into(),
        "-r".into(),
        format!("{private}/bundle"),
        "root@127.0.0.1:/tmp/rubix-bundle".into(),
    ]);
    verify_command(directory, "copy-inputs", &argv, &[0])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rehashed_command_fabrications_fail_semantic_checks() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let argv = vec!["ssh".to_owned(), "true".to_owned()];
        std::fs::write(dir.path().join("probe.stdout"), b"observed\n")?;
        std::fs::write(dir.path().join("probe.stderr"), b"")?;
        let valid = json!({"spawned":true,"owned_process_group_absent":true,"cleanup_complete":true,"output_eof":true,"cleanup_errors":[],"timeout":false,"cancelled":false,"merged_output":false,"output_limit":false,"owned_pid":1234,"owner_directory":"/tmp/test-owner","argv":argv,"exit_code":0,"stdout_sha256":rubix_dev::sha256(b"observed\n"),"stderr_sha256":rubix_dev::sha256(b"")});
        crate::parity::write_json(&dir.path().join("probe.command.json"), &valid, true)?;
        verify_command(dir.path(), "probe", &argv, &[0])?;
        for (key, value) in [
            ("argv", json!(["ssh", "false"])),
            ("exit_code", json!(1)),
            ("owned_process_group_absent", json!(false)),
            ("cleanup_complete", json!(false)),
            ("output_eof", json!(false)),
            ("cancelled", json!(true)),
            ("timeout", json!(true)),
            ("output_limit", json!(true)),
        ] {
            let mut bad = valid.clone();
            bad[key] = value;
            std::fs::write(dir.path().join("probe.stdout"), b"fabricated success\n")?;
            bad["stdout_sha256"] = json!(rubix_dev::sha256(b"fabricated success\n"));
            crate::parity::write_json(&dir.path().join("probe.command.json"), &bad, false)?;
            let inventory = super::super::guest::evidence_inventory(dir.path())?;
            crate::parity::write_json(
                &dir.path().join("result.json"),
                &json!({"evidence_sha256":inventory}),
                false,
            )?;
            assert!(
                verify_command(dir.path(), "probe", &argv, &[0]).is_err(),
                "{key}"
            );
        }
        Ok(())
    }
}
