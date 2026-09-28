//! Read-only prerequisite artifact verification shared by Alpine qualification.
use super::{
    BTreeMap, Path, Result, Value, digest, equal, fixture, json, load, require, source_inventory,
    text,
};
use std::collections::BTreeSet;
#[allow(
    clippy::too_many_lines,
    reason = "Independent exact prerequisite behavior oracle"
)]
fn verify_run(raw: &str) -> Result<Value> {
    require(
        raw.matches("test result: ok. 6 passed;").count() == 1
            && raw.matches("test result: ok. 7 passed;").count() == 2,
        "Rust prerequisite test completion",
    )?;
    let regex = regex::Regex::new(r"(?m)^([a-f0-9]{64})  /out/([a-z-]+)$")?;
    let binaries = regex
        .captures_iter(raw)
        .map(|c| (c[2].to_owned(), c[1].to_owned()))
        .collect::<Vec<_>>();
    require(
        binaries.len() == 5
            && binaries
                .iter()
                .map(|(_, hash)| hash)
                .collect::<BTreeSet<_>>()
                .len()
                == 5,
        "five distinct compiled identities",
    )?;
    equal(
        &json!(
            binaries
                .iter()
                .map(|(name, _)| name)
                .collect::<BTreeSet<_>>()
        ),
        &json!([
            "check-tests",
            "fixture-command",
            "preparation-tests",
            "rubixctl",
            "rubixctl-tests"
        ]),
        "binary inventory",
    )?;
    let records = raw
        .lines()
        .filter_map(|line| line.strip_prefix("RUBIX_PREPARATION "))
        .map(|line| rubix_dev::json::parse(line.as_bytes()))
        .collect::<Result<Vec<_>>>()?;
    require(records.len() == 1, "one preparation observation")?;
    let record = &records[0];
    equal(
        &json!(
            record
                .as_object()
                .ok_or("record object")?
                .keys()
                .collect::<Vec<_>>()
        ),
        &json!(["cases"]),
        "record keys",
    )?;
    let rows = record["cases"].as_array().ok_or("case array")?;
    let mut names = [
        "opt_out",
        "prepare",
        "repeat",
        "no_effect_success",
        "register_failure",
        "nonroot",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    names.extend((0..20).map(|i| format!("signal_{i}")));
    require(
        rows.len() == names.len(),
        "exact preparation case inventory",
    )?;
    let apk = "/sbin/apk add --no-cache nftables iptables";
    let update = "/sbin/rc-update add cgroups boot";
    let start = "/sbin/rc-service cgroups start";
    for (row, name) in rows.iter().zip(names) {
        equal(
            &json!(
                row.as_object()
                    .ok_or("case object")?
                    .keys()
                    .collect::<Vec<_>>()
            ),
            &json!([
                "actions",
                "exit",
                "name",
                "owned_child_absent",
                "signal_sent",
                "stderr",
                "stdout"
            ]),
            "case schema",
        )?;
        equal(&row["name"], &json!(name), "case name")?;
        let success = ["prepare", "repeat"].contains(&name.as_str());
        equal(&row["exit"], &json!(i32::from(!success)), "case exit")?;
        equal(&row["stdout"], &json!(""), "stdout")?;
        let stderr = row["stderr"].as_str().ok_or("stderr string")?;
        let actions = match name.as_str() {
            "opt_out" | "nonroot" => vec![],
            "prepare" | "repeat" => vec![apk, update, start],
            "register_failure" => vec![apk, update],
            _ => vec![apk],
        };
        equal(&row["actions"], &json!(actions), "exact command order")?;
        let signal = name
            .strip_prefix("signal_")
            .map(str::parse::<u32>)
            .transpose()?;
        equal(
            &row["owned_child_absent"],
            &json!(signal.is_some()),
            "owned child absence",
        )?;
        equal(
            &row["signal_sent"],
            &json!(signal.map(|number| if number % 2 == 0 { 2 } else { 15 })),
            "delivered signal",
        )?;
        require(
            stderr.contains("All 7 checks passed") == success,
            "no false success",
        )?;
        if ["register_failure", "no_effect_success"].contains(&name.as_str()) || signal.is_some() {
            require(
                stderr.contains("host state may have changed"),
                "partial effects warning",
            )?;
        }
    }
    Ok(json!({"record":record,"binaries":binaries.into_iter().collect::<BTreeMap<_,_>>()}))
}
#[allow(
    clippy::too_many_lines,
    clippy::items_after_statements,
    reason = "Audit exact compiled inventory and artifact provenance together"
)]
pub(crate) fn verify(root: &Path, directory: &Path) -> Result<Value> {
    let receipt = load(&directory.join("receipt.json"))?;
    equal(
        &receipt["schema_version"],
        &json!(2),
        "current Rust prerequisite capture schema",
    )?;
    let revision = receipt["revision"].as_str().ok_or("source revision")?;
    require(
        regex::Regex::new(r"^[a-f0-9]{40}$")?.is_match(revision),
        "exact source revision",
    )?;
    require(
        regex::Regex::new(r"^sha256:[a-f0-9]{64}$")?
            .is_match(receipt["image_id"].as_str().ok_or("builder image")?),
        "exact builder image",
    )?;
    equal(
        &receipt["source_sha256"],
        &source_inventory(root, "prerequisite")?,
        "current prerequisite harness sources",
    )?;
    let containers = receipt["containers"]
        .as_array()
        .ok_or("container inventory")?;
    require(
        containers.len() == 1,
        "single owned qualification container",
    )?;
    let container = containers[0].as_str().ok_or("container name")?;
    require(
        regex::Regex::new(r"^rubix-preparation-linux-[a-f0-9]{32}-test$")?.is_match(container),
        "owned container identity",
    )?;
    let tag = container.strip_suffix("-test").ok_or("container suffix")?;
    equal(
        &receipt["command"],
        &json!([
            "docker",
            "run",
            "--name",
            container,
            "--hostname",
            "fixture",
            "--init",
            "--read-only",
            "--network",
            "none",
            "--cap-drop",
            "ALL",
            "--cap-add",
            "SYS_CHROOT",
            "--cap-add",
            "SETUID",
            "--security-opt",
            "no-new-privileges",
            "--memory",
            "256m",
            "--cpus",
            "2",
            "--pids-limit",
            "64",
            "--tmpfs",
            "/tmp:rw,exec,nosuid,nodev,size=32m",
            tag
        ]),
        "exact isolated qualification command",
    )?;
    for key in [
        "errors",
        "cleanup_errors",
        "remaining_containers",
        "remaining_images",
    ] {
        equal(&receipt[key], &json!([]), key)?;
    }
    equal(&receipt["platform"], &json!("linux/arm64"), "platform")?;
    equal(
        &receipt["target"],
        &json!("aarch64-unknown-linux-musl"),
        "target",
    )?;
    equal(
        &receipt["uncommitted_implementation"],
        &json!(true),
        "working tree qualification",
    )?;
    let raw = text(&directory.join("run.log"))?;
    equal(
        &receipt["run_sha256"],
        &json!(rubix_dev::sha256(raw.as_bytes())),
        "raw run digest",
    )?;
    let result = verify_run(&raw)?;
    equal(
        &receipt["artifact_sha256"],
        &result["binaries"]["rubixctl"],
        "observed artifact identity",
    )?;
    let inventory = load(&directory.join("source-hashes.json"))?;
    equal(
        &receipt["inventory_sha256"],
        &json!(digest(&directory.join("source-hashes.json"))?),
        "source inventory digest",
    )?;
    let here = fixture(root, "prerequisite-preparation");
    equal(
        &inventory["Dockerfile"],
        &json!(digest(&here.join("Linux.Dockerfile"))?),
        "builder source",
    )?;
    let mut current = BTreeSet::from([
        "Cargo.toml".to_owned(),
        "Cargo.lock".into(),
        "rust-toolchain.toml".into(),
    ]);
    fn files(root: &Path, path: &Path, current: &mut BTreeSet<String>, all: bool) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            require(!kind.is_symlink(), "source inventory symlink")?;
            if entry.file_name() == "target" {
                continue;
            }
            if kind.is_dir() {
                files(root, &entry.path(), current, all)?;
            } else if kind.is_file() && (all || entry.file_name() == "Cargo.toml") {
                current.insert(
                    entry
                        .path()
                        .strip_prefix(root)?
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        Ok(())
    }
    for directory in ["crates", "third_party", "tools/upstream", "tools/dev"] {
        files(root, &root.join(directory), &mut current, false)?;
    }
    for directory in [
        ".cargo",
        "tools/dev/src",
        "crates/rubixctl/src",
        "crates/rubixctl/tests",
        "crates/rubix-platform/src",
        "crates/rubix-platform/tests",
        "crates/rubix-supervisor/src",
        "crates/rubix-supervisor/tests",
    ] {
        let path = root.join(directory);
        if path.exists() {
            files(root, &path, &mut current, true)?;
        }
    }
    for name in [
        "management-check/cases.json",
        "management-check/expected.json",
        "prerequisite-preparation/fixture-command.rs",
    ] {
        current.insert(format!("tools/parity/fixtures/{name}"));
    }
    let relevant = |name: &str| {
        name == "Cargo.toml"
            || name == "Cargo.lock"
            || name == "rust-toolchain.toml"
            || name.ends_with("/Cargo.toml")
            || name.starts_with(".cargo/")
            || name.starts_with("tools/dev/src/")
            || [
                "crates/rubixctl/",
                "crates/rubix-platform/",
                "crates/rubix-supervisor/",
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
                && Path::new(name).extension() == Some(std::ffi::OsStr::new("rs"))
            || [
                "tools/parity/fixtures/management-check/cases.json",
                "tools/parity/fixtures/management-check/expected.json",
                "tools/parity/fixtures/prerequisite-preparation/fixture-command.rs",
            ]
            .contains(&name)
    };
    current.retain(|name| relevant(name));
    equal(
        &json!(
            inventory
                .as_object()
                .ok_or("compiled inventory")?
                .keys()
                .filter(|name| relevant(name))
                .collect::<BTreeSet<_>>()
        ),
        &json!(current),
        "exact compiled source namespace",
    )?;
    for name in current {
        equal(
            &inventory[&name],
            &json!(digest(&root.join(&name))?),
            &format!("compiled source {name}"),
        )?;
    }
    let artifact = load(&directory.join("artifact.json"))?;
    equal(
        &artifact["sha256"],
        &receipt["artifact_sha256"],
        "artifact metadata",
    )?;
    equal(
        &artifact["revision"],
        &receipt["revision"],
        "artifact revision",
    )?;
    let binary = directory.join("rubixctl");
    if binary.exists() {
        equal(
            &json!(digest(&binary)?),
            &artifact["sha256"],
            "exported binary",
        )?;
        equal(
            &json!(std::fs::metadata(binary)?.len()),
            &artifact["size"],
            "exported size",
        )?;
    }
    Ok(receipt)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_prerequisite_observations_reject_exit_action_and_signal_mutations() -> Result<()> {
        let root = rubix_dev::repository_root(Path::new(env!("CARGO_MANIFEST_DIR")))?;
        let raw = text(&fixture(&root, "prerequisite-preparation").join("evidence-linux/run.log"))?;
        verify_run(&raw)?;
        let line = raw
            .lines()
            .find(|line| line.starts_with("RUBIX_PREPARATION "))
            .ok_or("actual observation")?;
        let original = rubix_dev::json::parse(
            line.strip_prefix("RUBIX_PREPARATION ")
                .ok_or("marker")?
                .as_bytes(),
        )?;
        for key in ["exit", "actions", "owned_child_absent", "signal_sent"] {
            let mut record = original.clone();
            record["cases"][6][key] = match key {
                "exit" => json!(0),
                "actions" => json!([]),
                "owned_child_absent" => json!(false),
                _ => json!(15),
            };
            let changed = raw.replace(
                line,
                &format!("RUBIX_PREPARATION {}", serde_json::to_string(&record)?),
            );
            assert!(verify_run(&changed).is_err(), "{key}");
        }
        assert!(
            verify_run(&raw.replace("test result: ok. 6 passed;", "test result: ok. 5 passed;"))
                .is_err()
        );
        Ok(())
    }
}
