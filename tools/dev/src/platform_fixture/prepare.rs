//! Public pinned package closure and actual Go baseline oracle preparation.
use super::{
    Options, Path, Result, Value, digest, equal, fixture, json, load, read, require,
    source_inventory,
};
use rubix_dev::defaults::capture::{OwnedDocker, Runner, arguments, owned_tag, push_error};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
fn publish(path: &Path, bytes: &[u8], expected: &str) -> Result<()> {
    require(
        rubix_dev::sha256(bytes) == expected,
        "verified cache content",
    )?;
    require(!path.is_symlink(), "cache symlink")?;
    if path.exists() {
        equal(
            &json!(digest(path)?),
            &json!(expected),
            "existing cache content",
        )?;
        return Ok(());
    }
    let mut partial = tempfile::NamedTempFile::new_in(path.parent().ok_or("cache parent")?)?;
    partial.write_all(bytes)?;
    partial.persist_noclobber(path)?;
    Ok(())
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep prepared build context and owned Docker lifecycle in one scope"
)]
pub(super) fn run(root: &Path, options: &Options) -> Result<u8> {
    let output = options.path("--output")?;
    rubix_dev::defaults::capture::create_output(output)?;
    let cache = options.path("--cache")?;
    require(!cache.is_symlink(), "cache directory symlink")?;
    fs::create_dir_all(cache)?;
    let here = fixture(root, "alpine-preparation");
    let inputs = load(&here.join("inputs.json"))?;
    let tag = owned_tag("rubix-alpine-prepare")?;
    let mut owned = OwnedDocker {
        tag,
        image_id: None,
        containers: vec![],
    };
    let mut runner = Runner::with_signals(output)?;
    runner.launcher = Some(std::env::current_exe()?);
    let mut report = json!({"schema_version":2,"containers":[],"errors":[],"cleanup_errors":[],"source_sha256":source_inventory(root,"alpine")?});
    let operation = (|| -> Result<()> {
        let index = cache.join("APKINDEX.tar.gz");
        equal(
            &json!(digest(&index)?),
            &inputs["packages"]["index_sha256"],
            "index pin",
        )?;
        let context = tempfile::Builder::new()
            .prefix("rubix-alpine-prepare-source-")
            .tempdir()?;
        let context_path = context.path().to_owned();
        for name in ["Prepare.Dockerfile", "go.mod", "baseline_test.go"] {
            fs::write(
                context_path.join(name),
                read(&here.join(name), 8 * 1024 * 1024)?,
            )?;
        }
        fs::write(
            context_path.join("APKINDEX.tar.gz"),
            read(&index, 8 * 1024 * 1024)?,
        )?;
        runner.keep_context(context);
        runner.bounded(
            &[
                "docker".into(),
                "build".into(),
                "--platform".into(),
                "linux/arm64".into(),
                "-t".into(),
                owned.tag.clone().into(),
                "-f".into(),
                context_path.join("Prepare.Dockerfile").into(),
                context_path.clone().into(),
            ],
            "build",
            900,
            8 * 1024 * 1024,
        )?;
        report["image"] = rubix_dev::defaults::capture::inspect_image(&mut runner, &mut owned)?;
        let name = format!("{}-resolve", owned.tag);
        owned.containers.push(name.clone());
        report["containers"] = json!(owned.containers);
        let resolution = runner.bounded(
            &arguments(&[
                "docker",
                "run",
                "--name",
                &name,
                "--read-only",
                "--network",
                "none",
                "--cap-drop",
                "ALL",
                "--security-opt",
                "no-new-privileges",
                "--memory",
                "256m",
                "--pids-limit",
                "64",
                "--tmpfs",
                "/tmp:rw,size=16m",
                "--tmpfs",
                "/var/cache/apk:rw,size=16m",
                &owned.tag,
                "apk",
                "fetch",
                "--simulate",
                "--recursive",
                "--url",
                "--no-network",
                "--repositories-file",
                "/repositories",
                "nftables=1.1.6-r1",
                "iptables=1.8.13-r0",
                "openrc=0.63.2-r0",
            ]),
            "resolution",
            60,
            65536,
        )?;
        let names = String::from_utf8(resolution)?
            .lines()
            .filter(|line| {
                line.starts_with("/repo/aarch64/") || line.starts_with("file:///repo/aarch64/")
            })
            .map(|line| line.rsplit('/').next().unwrap_or("").to_owned())
            .collect::<Vec<_>>();
        require(
            !names.is_empty()
                && names.len() <= 64
                && names.iter().collect::<BTreeSet<_>>().len() == names.len(),
            "unique nonempty resolved closure",
        )?;
        let accepted = inputs["packages"]["selected"]
            .as_object()
            .ok_or("package pins")?;
        equal(
            &json!(names.iter().collect::<BTreeSet<_>>()),
            &json!(accepted.keys().collect::<BTreeSet<_>>()),
            "package resolution drift",
        )?;
        for (source, dest) in [
            ("/preflight.test", "preflight.test"),
            ("/source.sha256", "source.sha256"),
        ] {
            let path = context_path.join(dest);
            runner.bounded(
                &[
                    "docker".into(),
                    "cp".into(),
                    format!("{name}:{source}").into(),
                    path.clone().into(),
                ],
                &format!("copy-{}", dest.replace('.', "-")),
                30,
                65536,
            )?;
            let bytes = read(&path, 16 * 1024 * 1024)?;
            let expected = if dest == "preflight.test" {
                inputs["oracle"]["binary_sha256"]
                    .as_str()
                    .ok_or("oracle hash")?
                    .to_owned()
            } else {
                rubix_dev::sha256(&bytes)
            };
            publish(&cache.join(dest), &bytes, &expected)?;
        }
        let source = text_source(&cache.join("source.sha256"))?;
        require(
            source.lines().next()
                == Some(&format!(
                    "{}  preflight.go",
                    inputs["oracle"]["source_sha256"]
                        .as_str()
                        .ok_or("source pin")?
                )),
            "unchanged Go baseline source",
        )?;
        let packages = cache.join("packages");
        require(!packages.is_symlink(), "packages directory symlink")?;
        fs::create_dir_all(&packages)?;
        publish(
            &packages.join("APKINDEX.tar.gz"),
            &read(&index, 8 * 1024 * 1024)?,
            inputs["packages"]["index_sha256"]
                .as_str()
                .ok_or("index hash")?,
        )?;
        let mut pins = serde_json::Map::new();
        for (index, name) in names.into_iter().enumerate() {
            require(
                Path::new(&name).extension() == Some(std::ffi::OsStr::new("apk"))
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"+_.-".contains(&b)),
                "safe package basename",
            )?;
            let pin = &accepted[&name];
            let url = format!("https://dl-cdn.alpinelinux.org/alpine/v3.24/main/aarch64/{name}");
            equal(&pin["url"], &json!(url), "official package URL")?;
            let downloaded = context_path.join(&name);
            runner.bounded(
                &[
                    "curl".into(),
                    "--fail".into(),
                    "--location".into(),
                    "--proto".into(),
                    "=https".into(),
                    "--max-time".into(),
                    "30".into(),
                    "--max-filesize".into(),
                    "16777216".into(),
                    "--output".into(),
                    downloaded.clone().into(),
                    url.clone().into(),
                ],
                &format!("download-{index:03}"),
                40,
                65536,
            )?;
            let bytes = read(&downloaded, 16 * 1024 * 1024)?;
            equal(&json!(bytes.len()), &pin["bytes"], "package size")?;
            publish(
                &packages.join(&name),
                &bytes,
                pin["sha256"].as_str().ok_or("package digest")?,
            )?;
            pins.insert(name, pin.clone());
        }
        report["packages"] = Value::Object(pins);
        report["index_sha256"] = inputs["packages"]["index_sha256"].clone();
        report["oracle_sha256"] = json!(digest(&cache.join("preflight.test"))?);
        report["baseline_source_and_binary"] = json!(source);
        let bytes = serde_json::to_vec_pretty(&report["packages"])?;
        publish(
            &cache.join("package-pins.json"),
            &bytes,
            &rubix_dev::sha256(&bytes),
        )?;
        equal(
            &source_inventory(root, "alpine")?,
            &report["source_sha256"],
            "source stable through preparation",
        )?;
        Ok(())
    })();
    if let Err(error) = operation {
        push_error(&mut report, "errors", error.to_string());
    }
    runner.finish(&mut report, output, &owned)?;
    let mut inventory = super::guest::evidence_inventory(output)?;
    inventory
        .as_object_mut()
        .ok_or("inventory object")?
        .remove("receipt.json");
    report["evidence_sha256"] = inventory;
    crate::parity::write_json(&output.join("receipt.json"), &report, false)?;
    Ok(u8::from(!rubix_dev::defaults::capture::successful(&report)))
}
fn text_source(path: &Path) -> Result<String> {
    Ok(String::from_utf8(read(path, 65536)?)?)
}
pub(super) fn verify(root: &Path, output: &Path) -> Result<()> {
    let report = load(&output.join("receipt.json"))?;
    let mut inventory = super::guest::evidence_inventory(output)?;
    inventory
        .as_object_mut()
        .ok_or("inventory object")?
        .remove("receipt.json");
    equal(
        &report["evidence_sha256"],
        &inventory,
        "exact preparation raw inventory",
    )?;
    equal(
        &report["schema_version"],
        &json!(2),
        "current Rust preparation schema",
    )?;
    require(
        rubix_dev::defaults::capture::successful(&report),
        "package/oracle preparation cleanup",
    )?;
    equal(
        &report["source_sha256"],
        &source_inventory(root, "alpine")?,
        "current preparation sources",
    )?;
    let inputs = load(&fixture(root, "alpine-preparation").join("inputs.json"))?;
    equal(
        &report["packages"],
        &inputs["packages"]["selected"],
        "full pinned package closure",
    )?;
    equal(
        &report["index_sha256"],
        &inputs["packages"]["index_sha256"],
        "index identity",
    )?;
    equal(
        &report["oracle_sha256"],
        &inputs["oracle"]["binary_sha256"],
        "oracle identity",
    )?;
    let source = report["baseline_source_and_binary"]
        .as_str()
        .ok_or("baseline source observation")?;
    require(
        source.lines().next()
            == Some(&format!(
                "{}  preflight.go",
                inputs["oracle"]["source_sha256"]
                    .as_str()
                    .ok_or("source digest")?
            )),
        "unchanged baseline source",
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_publication_rejects_wrong_bytes_and_links_without_overwrite() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("cache");
        let hash = rubix_dev::sha256(b"expected");
        assert!(publish(&path, b"wrong", &hash).is_err());
        assert!(!path.exists());
        publish(&path, b"expected", &hash)?;
        assert!(publish(&path, b"changed", &rubix_dev::sha256(b"changed")).is_err());
        assert_eq!(fs::read(&path)?, b"expected");
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&path, &link)?;
        assert!(publish(&link, b"expected", &hash).is_err());
        Ok(())
    }
}
