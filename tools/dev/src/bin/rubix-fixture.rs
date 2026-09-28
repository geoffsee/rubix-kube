//! Verify public distribution fixture behavior against independent source oracles.
use rubix_dev::{Result, fixture_oracles as oracle};
use std::{ffi::OsString, path::PathBuf, process::ExitCode};

fn run(args: &[OsString]) -> Result<()> {
    if args.len() != 3 || args[0] != "verify" {
        return Err(
            "usage: rubix-fixture verify <credentials|runtime-mapping|webhooks> PATH".into(),
        );
    }
    let path = PathBuf::from(&args[2]);
    match args[1].to_str() {
        Some("credentials") => {
            for component in ["apiserver", "kubelet"] {
                oracle::equal(
                    &oracle::load(&path.join(format!("{component}.json")))?,
                    &oracle::credentials::expected(component)?,
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
