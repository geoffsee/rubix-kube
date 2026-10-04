//! Repository metadata audit; release qualification fails closed until trusted
//! current candidate-bound completion receipt verification is implemented.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let metadata_only = args.as_slice() == [std::ffi::OsString::from("--metadata-only")];
    if !args.is_empty() && !metadata_only {
        eprintln!("usage: rubix-qualification [--metadata-only]");
        return ExitCode::from(2);
    }
    let root = match rubix_dev::repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("failed to resolve repository root: {err}");
            return ExitCode::from(2);
        },
    };
    if !metadata_only {
        return match rubix_dev::release_qualification::run_release_qualification(&root) {
            Ok(_) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            },
        };
    }
    match rubix_dev::release_qualification::audit_repository_metadata(&root) {
        Ok(report) => {
            report.print_summary();
            println!("METADATA AUDIT PASSED; RELEASE REMAINS UNQUALIFIED");
            ExitCode::SUCCESS
        },
        Err(error) => {
            eprintln!("REPOSITORY METADATA AUDIT FAILED:\n{error}");
            ExitCode::FAILURE
        },
    }
}
