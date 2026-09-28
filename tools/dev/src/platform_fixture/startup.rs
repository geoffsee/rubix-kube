use super::{
    BTreeMap, Options, Path, Result, Value, digest, equal, fixture, json, load, read, require,
    source_inventory, text,
};
fn baseline_provenance(root: &Path) -> Result<()> {
    let here = fixture(root, "cli-startup");
    let provenance = load(&here.join("provenance.json"))?;
    equal(
        &json!(
            provenance["source_sha256"]
                .as_object()
                .ok_or("baseline source map")?
                .keys()
                .collect::<Vec<_>>()
        ),
        &json!([
            "cmd/kubesolo/main.go",
            "go.mod",
            "go.sum",
            "internal/config/file.go",
            "internal/config/flags/flags.go",
            "internal/config/loader.go"
        ]),
        "baseline source inventory",
    )?;
    equal(
        &json!(
            provenance["kingpin_source_sha256"]
                .as_object()
                .ok_or("kingpin source map")?
                .keys()
                .collect::<Vec<_>>()
        ),
        &json!(["app.go", "envar.go", "flags.go", "global.go", "parser.go"]),
        "kingpin source inventory",
    )?;
    equal(
        &provenance["observations_sha256"],
        &json!(digest(&here.join("observations.json"))?),
        "reviewed observation digest",
    )?;
    Ok(())
}
pub(super) const BINARY: &str = "67348b2560d0f831de20ef722c53cc317e385ab7cfbf406633fbbb6a7b73f81e";
pub(super) const REVISION: &str = "2ef1c4787989f11f868f81bb84ae2afd4a49a81d";
const CASE_IDS: &[&str] = &[
    "version-long",
    "version-short",
    "version-invalid-bool-env",
    "version-invalid-int-env",
    "version-invalid-map-env",
    "version-directory-file",
    "version-before-full",
    "version-invalid-explicit-int",
    "version-unknown-flag",
    "unknown-before-version",
    "help-long-name",
    "help-invalid-env",
    "help-invalid-explicit-int",
    "help-unknown-flag",
    "short-h",
    "duplicate-bool",
    "duplicate-positive-negative",
    "duplicate-version-alias",
    "duplicate-string",
    "bool-equals-false",
    "bool-equals-true",
    "bool-following-false",
    "bool-empty-equals",
    "string-empty-argv",
    "string-empty-equals",
    "integer-empty-equals",
    "empty-positional",
    "delimiter-empty-tail",
    "delimiter-flag-is-positional",
    "missing-argument",
    "invalid-env-overridden-boolean",
    "invalid-env-overridden-integer",
    "directory-before-string-map-env",
    "primitive-env-before-directory",
    "empty-full-env",
    "empty-config-env",
    "empty-config-equals",
    "no-version-print",
    "no-help",
];
fn normalize(stderr: &str) -> Result<Value> {
    let mut result = vec![];
    for line in stderr.split_inclusive('\n') {
        match rubix_dev::json::parse(line.as_bytes()) {
            Ok(Value::Object(mut value)) => {
                value.remove("time");
                result.push(Value::Object(value));
            },
            Ok(_) => result.push(json!(line)),
            Err(error) => {
                // Ordinary prose is expected; structured JSON lines must remain strict.
                if line.trim_start().starts_with('{') {
                    return Err(error);
                }
                result.push(json!(line));
            },
        }
    }
    Ok(json!(result))
}
#[allow(
    clippy::too_many_lines,
    reason = "Source-specified startup case inventory and stream verification"
)]
fn observations(root: &Path, directory: &Path) -> Result<Value> {
    let here = fixture(root, "cli-startup");
    let suite = load(&here.join("suite.json"))?;
    let record = load(&directory.join("result.json"))?;
    let runner = load(&directory.join("runner-result.json"))?;
    let wanted = suite["cases"].as_array().ok_or("suite cases")?;
    let cases = record["cases"].as_array().ok_or("observed cases")?;
    equal(
        &json!(wanted.iter().map(|c| &c["id"]).collect::<Vec<_>>()),
        &json!(CASE_IDS),
        "source-specified case inventory",
    )?;
    equal(
        &json!(cases.iter().map(|c| &c["id"]).collect::<Vec<_>>()),
        &json!(CASE_IDS),
        "observed case inventory",
    )?;
    equal(&record["status"], &json!("passed"), "driver status")?;
    equal(&runner["exit_code"], &json!(0), "runner exit")?;
    equal(&runner["errors"], &json!([]), "runner errors")?;
    equal(
        &runner["cleanup_uncertain"],
        &json!(false),
        "settled runner",
    )?;
    equal(
        &runner["owned_context_removed"],
        &json!(true),
        "context cleanup",
    )?;
    equal(
        &runner["retained_commands"],
        &json!([]),
        "retained commands",
    )?;
    let suite_hash = json!(digest(&here.join("suite.json"))?);
    equal(&record["suite_sha256"], &suite_hash, "driver suite hash")?;
    equal(
        &runner["source_sha256"]["suite"],
        &suite_hash,
        "runner suite hash",
    )?;
    equal(
        &record["artifact"]["source"]["revision"],
        &json!(REVISION),
        "baseline revision",
    )?;
    equal(
        &record["artifact"]["sha256"],
        &json!(BINARY),
        "baseline binary",
    )?;
    for kind in ["container", "image", "volume"] {
        require(
            text(&directory.join(format!("{kind}-verify-removal.stdout")))?
                .trim()
                .is_empty(),
            "owned resource still present",
        )?;
        let receipt = load(&directory.join(format!("{kind}-verify-removal.command.json")))?;
        require(
            receipt["exit_code"] == 0,
            "cleanup inventory command failed",
        )?;
    }
    let mut output = serde_json::Map::new();
    for (index, (wanted, actual)) in wanted.iter().zip(cases).enumerate() {
        for key in ["id", "argv"] {
            equal(&actual[key], &wanted[key], key)?;
        }
        for (key, value) in [
            ("status", json!("passed")),
            ("failures", json!([])),
            ("owned_process_group_absent", json!(true)),
            ("artifact_uid_inventory_confirmed", json!(true)),
            ("timeout", json!(false)),
            ("cancelled", json!(false)),
        ] {
            equal(&actual[key], &value, key)?;
        }
        require(
            actual.get("remaining_artifact_pids").is_none() && actual.get("errors").is_none(),
            "artifact cleanup errors",
        )?;
        equal(
            &actual["exit_code"],
            &wanted["expect"]["exit_code"],
            "case exit",
        )?;
        let mut streams = BTreeMap::new();
        for kind in ["stdout", "stderr"] {
            let name = format!("{index:03}.{kind}");
            equal(
                &actual[format!("{kind}_file")],
                &json!(name),
                "stream filename",
            )?;
            let raw = read(&directory.join(name), 256 * 1024)?;
            equal(
                &actual[format!("{kind}_sha256")],
                &json!(rubix_dev::sha256(&raw)),
                "stream identity",
            )?;
            streams.insert(kind, String::from_utf8(raw)?);
        }
        streams.insert(
            "combined",
            format!("{}{}", streams["stdout"], streams["stderr"]),
        );
        for (kind, stream) in &streams {
            if let Some(exact) = wanted["expect"].get(format!("{kind}_equals")) {
                equal(&json!(stream), exact, "exact expectation")?;
            }
            if let Some(markers) = wanted["expect"].get(format!("{kind}_contains")) {
                for marker in markers.as_array().ok_or("marker array")? {
                    require(
                        stream.contains(marker.as_str().ok_or("string marker")?),
                        "missing independently specified marker",
                    )?;
                }
            }
        }
        output.insert(CASE_IDS[index].into(),json!({"exit_code":actual["exit_code"],"stdout":streams["stdout"],"stderr":normalize(&streams["stderr"])?}));
    }
    require(
        !output["version-before-full"]["stderr"]
            .to_string()
            .contains("deprecated"),
        "full warning preceded version",
    )?;
    Ok(Value::Object(output))
}
pub(super) fn verify(root: &Path, directory: &Path) -> Result<Value> {
    baseline_provenance(root)?;
    let capture = load(&directory.join("capture.json"))?;
    equal(
        &capture["schema_version"],
        &json!(2),
        "current Rust capture schema",
    )?;
    equal(
        &capture["source_sha256"],
        &source_inventory(root, "startup")?,
        "current Rust source inventory",
    )?;
    equal(&capture["status"], &json!("passed"), "capture status")?;
    let mut inventory = super::guest::evidence_inventory(directory)?;
    inventory
        .as_object_mut()
        .ok_or("inventory object")?
        .remove("capture.json");
    equal(
        &capture["evidence_sha256"],
        &inventory,
        "exact raw startup evidence inventory",
    )?;
    let runner = load(&directory.join("runner-result.json"))?;
    equal(
        &capture["runner_binary_sha256"],
        &runner["source_sha256"]["runner"],
        "actual runner binary",
    )?;
    let record = load(&directory.join("result.json"))?;
    equal(
        &capture["driver_binary_sha256"],
        &record["driver_sha256"],
        "executed Linux driver binary",
    )?;
    equal(
        &capture["result_sha256"],
        &json!(digest(&directory.join("result.json"))?),
        "raw driver result",
    )?;
    equal(
        &capture["runner_result_sha256"],
        &json!(digest(&directory.join("runner-result.json"))?),
        "raw runner result",
    )?;
    let output = observations(root, directory)?;
    equal(
        &output,
        &load(&fixture(root, "cli-startup").join("observations.json"))?,
        "reviewed independent observations",
    )?;
    Ok(output)
}
pub(super) fn capture(root: &Path, options: &Options) -> Result<u8> {
    baseline_provenance(root)?;
    require(
        !options.values.contains_key("--runner"),
        "runner is this source-bound Rust executable",
    )?;
    let descriptor = options.path("--artifact")?;
    let artifact = load(descriptor)?;
    equal(
        &artifact["sha256"],
        &json!(BINARY),
        "pinned startup artifact",
    )?;
    equal(
        &artifact["source"]["revision"],
        &json!(REVISION),
        "pinned baseline revision",
    )?;
    let binary = descriptor
        .parent()
        .ok_or("artifact parent")?
        .join(artifact["binary"].as_str().ok_or("artifact binary")?);
    equal(
        &json!(digest(&binary)?),
        &json!(BINARY),
        "actual baseline binary",
    )?;
    let driver = options.path("--driver")?;
    let driver_hash = digest(driver)?;
    let sources = source_inventory(root, "startup")?;
    let output = options.path("--output")?;
    let argv = vec![
        "run".into(),
        "--repo".into(),
        root.into(),
        "--artifact".into(),
        descriptor.into(),
        "--suite".into(),
        fixture(root, "cli-startup").join("suite.json").into(),
        "--output".into(),
        output.into(),
        "--driver".into(),
        driver.into(),
    ];
    let execution = crate::parity::main(&argv);
    let status = execution.as_ref().copied().unwrap_or(1);
    let mut capture = json!({"schema_version":2,"source_sha256":sources,"runner_binary_sha256":digest(&std::env::current_exe()?)?,"driver_binary_sha256":driver_hash,"status":"failed"});
    let verified = (|| -> Result<()> {
        require(status == 0, "parity runner failed")?;
        equal(
            &source_inventory(root, "startup")?,
            &sources,
            "source stable through capture",
        )?;
        equal(
            &observations(root, output)?,
            &load(&fixture(root, "cli-startup").join("observations.json"))?,
            "independent startup observations",
        )?;
        let record = load(&output.join("result.json"))?;
        equal(
            &record["driver_sha256"],
            &json!(driver_hash),
            "executed driver",
        )?;
        capture["result_sha256"] = json!(digest(&output.join("result.json"))?);
        capture["runner_result_sha256"] = json!(digest(&output.join("runner-result.json"))?);
        capture["status"] = json!("passed");
        capture["evidence_sha256"] = super::guest::evidence_inventory(output)?;
        Ok(())
    })();
    if let Err(error) = &verified {
        capture["error"] = json!(error.to_string());
    }
    let publication = crate::parity::write_json(&output.join("capture.json"), &capture, true);
    execution?;
    publication?;
    verified?;
    Ok(0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_preserves_all_fields_except_wall_clock() -> Result<()> {
        equal(
            &normalize("{\"time\":\"one\",\"level\":\"warn\",\"message\":\"a\"}\nplain\n")?,
            &json!([{"level":"warn","message":"a"},"plain\n"]),
            "normalization",
        )?;
        assert!(normalize("{\"level\":1,\"level\":2}\n").is_err());
        Ok(())
    }
    #[test]
    fn published_startup_requires_current_rust_captures() -> Result<()> {
        let root = rubix_dev::repository_root(Path::new(env!("CARGO_MANIFEST_DIR")))?;
        for run in ["first", "repeat"] {
            verify(
                &root,
                &fixture(&root, "cli-startup")
                    .join("evidence-rust")
                    .join(run),
            )?;
        }
        Ok(())
    }
}
