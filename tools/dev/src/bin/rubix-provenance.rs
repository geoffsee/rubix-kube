//! Generate publication checksums, provenance, license inventory and release manifest
//! for a candidate artifact directory, then verify the checksums.
//!
//! Usage: `rubix-provenance <dist-dir> <version> [repository-root]`

use rubix_dev::provenance::{
    CHECKSUM_FILE, ChecksumManifest, generate_license_inventory, generate_manifest,
    generate_source_provenance,
};
use rubix_dev::{Result, repository_root};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::{env, fs};

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let dist = PathBuf::from(
        args.next()
            .ok_or("usage: rubix-provenance <dist-dir> <version>")?,
    );
    let version = args
        .next()
        .ok_or("usage: rubix-provenance <dist-dir> <version>")?;
    let root = match args.next() {
        Some(path) => PathBuf::from(path),
        None => repository_root(&env::current_dir()?)?,
    };

    let checksums = ChecksumManifest::generate(&dist)?;
    let provenance = generate_source_provenance(
        &fs::read_to_string(root.join("tools/upstream/inputs.json"))?,
        &fs::read_to_string(root.join("docs/architecture/upstream-inputs.json"))?,
        &version,
        &checksums,
    )?;
    let metadata = Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1"])
        .current_dir(&root)
        .output()?;
    if !metadata.status.success() {
        return Err("cargo metadata failed".into());
    }
    let licenses = generate_license_inventory(std::str::from_utf8(&metadata.stdout)?)?;
    let manifest = generate_manifest(&dist, &version, "rubix-kube", "rubixctl")?;

    write(
        &dist,
        "provenance.json",
        &serde_json::to_string_pretty(&provenance)?,
    )?;
    write(
        &dist,
        "licenses.json",
        &serde_json::to_string_pretty(&licenses)?,
    )?;
    write(
        &dist,
        "release-manifest.json",
        &serde_json::to_string_pretty(&manifest)?,
    )?;
    write(&dist, CHECKSUM_FILE, &checksums.render())?;
    checksums.verify(&dist)?;
    Ok(())
}

fn write(dir: &Path, name: &str, content: &str) -> Result<()> {
    fs::write(dir.join(name), format!("{}\n", content.trim_end()))?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rubix-provenance: {error}");
            ExitCode::FAILURE
        },
    }
}
