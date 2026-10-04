//! Explicit fixture diagnostics; production release qualification remains unavailable.
use rubix_dev::{
    release::{
        assemble_fixture_evidence, assemble_release_evidence, verify_fixture_evidence,
        verify_release_evidence,
    },
    repository_root,
};
use std::{env, path::PathBuf, process::ExitCode};
#[tokio::main]
async fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        println!(
            "rubix-release assemble|verify [directory]\n  Production commands fail closed: live evidence importer unavailable.\nrubix-release assemble-fixtures|verify-fixtures [directory]\n  Unqualified fixture diagnostics only; default docs/release. Assembly requires empty output."
        );
        return ExitCode::SUCCESS;
    }
    let result = async {
        if args.len() > 2 {
            return Err("unexpected arguments".into());
        }
        let root = repository_root(&env::current_dir()?)?;
        let dir = args
            .get(1)
            .map_or_else(|| root.join("docs/release"), PathBuf::from);
        match args[0].as_str() {
            "assemble" => assemble_release_evidence(&root, &dir).await,
            "verify" => verify_release_evidence(&dir),
            "assemble-fixtures" => assemble_fixture_evidence(&root, &dir).await,
            "verify-fixtures" => verify_fixture_evidence(&root, &dir),
            _ => Err("unknown command; use --help".into()),
        }
    }
    .await;
    match result {
        Ok(()) => {
            let message = match args[0].as_str() {
                "assemble-fixtures" => {
                    "UNQUALIFIED_FIXTURE_ONLY: fixture diagnostics assembled and consistency checked; no production release qualification."
                },
                "verify-fixtures" => {
                    "UNQUALIFIED_FIXTURE_ONLY: fixture consistency checked; no production release qualification."
                },
                "assemble" => "Production release evidence assembled.",
                "verify" => "Production release evidence verified.",
                _ => unreachable!("unknown commands return an error"),
            };
            println!("{message}");
            ExitCode::SUCCESS
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        },
    }
}
