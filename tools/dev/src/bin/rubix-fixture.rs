//! Verify public distribution fixture behavior against independent source oracles.
use rubix_dev::{Result, fixture_oracles as oracle};
use std::{ffi::OsString, path::PathBuf, process::ExitCode};

fn run(args: &[OsString]) -> Result<()> {
    if args.first().is_some_and(|arg| arg == "capture") {
        let code = rubix_dev::fixture_capture::cli(&args[1..])?;
        if code != 0 {
            return Err("fixture capture failed; inspect its receipt".into());
        }
        return Ok(());
    }
    if args.len() == 3 && args[0] == "verify-evidence" {
        let root = rubix_dev::repository_root(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))?;
        return rubix_dev::fixture_capture::verify_evidence(
            &root,
            args[1].to_str().ok_or("non UTF-8 fixture family")?,
            std::path::Path::new(&args[2]),
        );
    }
    if args.len() != 3 || args[0] != "verify" {
        return Err(
            "usage: rubix-fixture verify <credentials|runtime-mapping|webhooks|node-config|pki|resources|config-api|config-write-links> PATH".into(),
        );
    }
    let path = PathBuf::from(&args[2]);
    match args[1].to_str() {
        Some("config-api") => {
            oracle::config_api::verify_file(&oracle::load(&path.join("config.json"))?)?;
            oracle::config_api::verify_api(&oracle::load(&path.join("configapi.json"))?)?;
        },
        Some("config-write-links") => oracle::config_links::verify(&oracle::load(&path)?)?,
        Some("resources") => {
            let mut records = Vec::new();
            for component in ["coredns", "localpath", "portainer", "d2k"] {
                let value = oracle::load(&path.join(format!("{component}.json")))?;
                records.extend(
                    value
                        .as_array()
                        .ok_or("resource array missing")?
                        .iter()
                        .cloned(),
                );
            }
            oracle::resources::verify(&serde_json::Value::Array(records))?;
        },
        Some("pki") => oracle::pki::verify(&oracle::load(&path)?)?,
        Some("credentials") => {
            for component in ["apiserver", "kubelet"] {
                oracle::equal(
                    &oracle::load(&path.join(format!("{component}.json")))?,
                    &oracle::credentials::expected(component)?,
                )?;
            }
        },
        Some("node-config") => {
            for component in ["kubelet", "containerd"] {
                oracle::equal(
                    &oracle::load(&path.join(format!("{component}.json")))?,
                    &oracle::node_config::expected(component)?,
                )?;
            }
        },
        Some("runtime-mapping") => {
            oracle::equal(&oracle::load(&path)?, &oracle::mapping::expected())?;
        },
        Some("webhooks") => oracle::equal(
            &oracle::load(&path.join("webhook.json"))?,
            &oracle::webhook::expected()?,
        )?,
        _ => return Err("unknown fixture family".into()),
    }
    println!("public fixture behavior matches independent expectations");
    Ok(())
}

fn main() -> ExitCode {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|arg| arg == "__exec") {
        return match rubix_dev::process::child_exec(&args) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(2)
            },
        };
    }
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        },
    }
}
