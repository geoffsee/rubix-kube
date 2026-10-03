//! Automated release qualification verification binary (Issue #126 / Gate C16/C17).
//!
//! Enforces:
//! - Cryptographic artifact digest bindings.
//! - License and attribution completeness.
//! - Link integrity and completeness across all documentation artifacts.
//! - Formal check that all 11 completion criteria are satisfied with verifiable evidence.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = match rubix_dev::repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("failed to resolve repository root: {err}");
            return ExitCode::from(2);
        },
    };

    match rubix_dev::release_qualification::run_release_qualification(&root) {
        Ok(report) => {
            report.print_summary();
            ExitCode::SUCCESS
        },
        Err(error) => {
            eprintln!("RELEASE QUALIFICATION VERIFICATION FAILED:\n{error}");
            ExitCode::FAILURE
        },
    }
}
