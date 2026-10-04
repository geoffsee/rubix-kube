//! Release evidence assembly and verification CLI.
//!
//! Complies with Gate C16/C17 requirements (Epic E30 / Issue #126).
//!
//! Usage:
//!   rubix-release assemble [output-dir]
//!   rubix-release verify [dir]
//!   rubix-release --help

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use rubix_dev::release::{assemble_release_evidence, verify_release_evidence};
use rubix_dev::repository_root;

fn print_help() {
    eprintln!(
        r"rubix-release: Gate C16/C17 release evidence and attribution tool

Usage:
  rubix-release assemble [output-dir]   Assemble checksums, manifests, attribution, and reports
  rubix-release verify [release-dir]    Validate all release evidence and checksum bindings
  rubix-release --help                  Print this help message

Default directory: docs/release
"
    );
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return ExitCode::SUCCESS;
    }

    let current_dir = match env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot get current directory: {e}");
            return ExitCode::FAILURE;
        },
    };

    let root = match repository_root(&current_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot find repository root: {e}");
            return ExitCode::FAILURE;
        },
    };

    let default_release_dir = root.join("docs/release");

    match args[0].as_str() {
        "assemble" => {
            let target_dir = if args.len() > 1 {
                PathBuf::from(&args[1])
            } else {
                default_release_dir
            };

            println!(
                "Assembling release evidence into {}...",
                target_dir.display()
            );
            match assemble_release_evidence(&root, &target_dir).await {
                Ok(()) => {
                    println!(
                        "Release evidence successfully assembled at {}",
                        target_dir.display()
                    );
                    ExitCode::SUCCESS
                },
                Err(e) => {
                    eprintln!("error: release assembly failed: {e}");
                    ExitCode::FAILURE
                },
            }
        },
        "verify" => {
            let target_dir = if args.len() > 1 {
                PathBuf::from(&args[1])
            } else {
                default_release_dir
            };

            println!("Verifying release evidence at {}...", target_dir.display());
            match verify_release_evidence(&target_dir) {
                Ok(()) => {
                    println!(
                        "✓ All release evidence verified successfully: checksums, manifests, attribution, notes, and dual-format kubeconfig valid."
                    );
                    ExitCode::SUCCESS
                },
                Err(e) => {
                    eprintln!("error: release evidence verification failed: {e}");
                    ExitCode::FAILURE
                },
            }
        },
        other => {
            eprintln!("error: unrecognized command: {other}");
            print_help();
            ExitCode::FAILURE
        },
    }
}
