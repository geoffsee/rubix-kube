//! Synthetic C14 fixture consistency and independently read candidate byte verification.
use rubix_dev::platform_soak::{
    CandidateVerificationSummary, EnvironmentMapping, PlatformSoakReport, PlatformSoakRunner,
    RegressionSuite, RestartSummary, SustainedSoakSummary,
};
use std::{path::PathBuf, process::ExitCode};

fn version_args(
    args: &[String],
    output_allowed: bool,
) -> Result<(String, Option<PathBuf>), String> {
    let mut version = env!("CARGO_PKG_VERSION").to_string();
    let mut directory = None;
    let mut remaining = args;
    while !remaining.is_empty() {
        match remaining {
            [flag, value, rest @ ..] if flag == "--version" || flag == "--expected-version" => {
                version.clone_from(value);
                remaining = rest;
            },
            [flag, value, rest @ ..] if flag == "--output" && output_allowed => {
                directory = Some(PathBuf::from(value));
                remaining = rest;
            },
            _ => return Err("unknown option or missing value".into()),
        }
    }
    Ok((version, directory))
}

fn handle_capture(args: &[String]) -> Result<(), String> {
    let mut output_dir: Option<PathBuf> = None;
    let mut duration_secs: u64 = 10;
    let mut cycles: u32 = 10;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                if i >= args.len() {
                    return Err("--output requires a directory path".into());
                }
                output_dir = Some(PathBuf::from(&args[i]));
            },
            "--duration" => {
                i += 1;
                if i >= args.len() {
                    return Err("--duration requires a number of seconds".into());
                }
                duration_secs = args[i]
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --duration: {e}"))?;
            },
            "--cycles" => {
                i += 1;
                if i >= args.len() {
                    return Err("--cycles requires a number of cycles".into());
                }
                cycles = args[i]
                    .parse::<u32>()
                    .map_err(|e| format!("invalid --cycles: {e}"))?;
            },
            other if !other.starts_with('-') && output_dir.is_none() => {
                output_dir = Some(PathBuf::from(other));
            },
            other => return Err(format!("unexpected argument for capture: {other}")),
        }
        i += 1;
    }
    let output_dir =
        output_dir.unwrap_or_else(|| PathBuf::from("target/platform-soak-qualification"));
    let root = rubix_dev::repository_root(&std::env::current_dir().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let (receipt_path, report_json_path, report_md_path) = rt
        .block_on(rubix_dev::platform_soak::capture_soak(
            &output_dir,
            duration_secs,
            cycles,
            &root,
        ))
        .map_err(|e| e.to_string())?;

    println!("Platform soak qualification capture completed successfully:");
    println!("  Receipt:     {}", receipt_path.display());
    println!("  Report JSON: {}", report_json_path.display());
    println!("  Report MD:   {}", report_md_path.display());
    Ok(())
}

fn handle_verify_receipt(args: &[String]) -> Result<(), String> {
    let path = match args {
        [p] => p,
        [flag, p] if flag == "--input" || flag == "-i" => p,
        _ => return Err("verify-receipt requires a receipt file or directory path".into()),
    };
    let root = rubix_dev::repository_root(&std::env::current_dir().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let receipt = rubix_dev::platform_soak::verify_soak_receipt(std::path::Path::new(path), &root)
        .map_err(|e| e.to_string())?;
    println!(
        "Platform soak receipt verified successfully:\n\
         - Criterion: {}\n\
         - Status: qualified\n\
         - Candidate revision: {}\n\
         - Assertions passed: {}\n\
         - Skips documented: {}",
        receipt.criterion,
        receipt.candidate.source_revision,
        receipt.assertions.len(),
        receipt.skips.len()
    );
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn execute(args: &[String]) -> Result<(), String> {
    match args {
        [] => return execute(&["help".into()]),
        [s] if s == "help" || s == "--help" || s == "-h" => {
            println!(
                "rubix-platform-soak: Platform coverage, soak and Criterion 6 qualification\n\
                 capture [--output DIR] [--duration SECS] [--cycles N] (Criterion 6 qualification)\n\
                 verify-receipt RECEIPT_OR_DIR (Criterion 6 verification)\n\
                 run [--version VERSION] (unavailable)\n\
                 verify REPORT [--expected-version VERSION] (unavailable)\n\
                 fixture [--version VERSION] [--output DIR]\n\
                 verify-fixture REPORT [--expected-version VERSION]\n\
                 verify-candidate MANIFEST ARTIFACT_DIR --expected-version VERSION\n\
                 matrix | soak | restarts | regressions (synthetic inventories)"
            );
        },
        [command, rest @ ..] if command == "capture" => handle_capture(rest)?,
        [command, rest @ ..] if command == "verify-receipt" => handle_verify_receipt(rest)?,
        [command, rest @ ..] if command == "run" || command == "fixture" => {
            let (version, directory) = version_args(rest, command == "fixture")?;
            let runner = PlatformSoakRunner::new();
            let report = if command == "fixture" {
                runner.run_fixture(&version)
            } else {
                runner.run_qualification(&version, None)
            }
            .map_err(|e| e.to_string())?;
            if let Some(directory) = directory {
                let json = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
                let markdown = report.to_markdown();
                std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
                std::fs::write(directory.join("platform-soak-report.json"), json)
                    .map_err(|e| e.to_string())?;
                std::fs::write(directory.join("platform-soak-report.md"), markdown)
                    .map_err(|e| e.to_string())?;
            }
            println!(
                "Synthetic fixture consistency passed; C14 NOT QUALIFIED; no workload/soak/restart execution or candidate byte observations."
            );
        },
        [command, file, rest @ ..] if command == "verify" || command == "verify-fixture" => {
            let (version, _) = version_args(rest, false)?;
            let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
            let report: PlatformSoakReport =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if command == "verify-fixture" {
                report.validate_fixture(Some(&version))
            } else {
                report.validate(Some(&version))
            }
            .map_err(|e| e.to_string())?;
            println!("Synthetic fixture consistency passed; C14 NOT QUALIFIED.");
        },
        [command, manifest, directory, flag, version]
            if command == "verify-candidate" && flag == "--expected-version" =>
        {
            let manifest =
                serde_json::from_slice(&std::fs::read(manifest).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let summary = CandidateVerificationSummary::verify_files(
                &manifest,
                &PathBuf::from(directory),
                version,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
            );
        },
        [command] if command == "matrix" => {
            println!(
                "Synthetic matrix plan only; C14 NOT QUALIFIED.\n{}",
                serde_json::to_string_pretty(&EnvironmentMapping::canonical_matrix())
                    .map_err(|e| e.to_string())?
            );
        },
        [command] if command == "soak" => {
            println!(
                "Synthetic arithmetic only; no 24-hour run observed; C14 NOT QUALIFIED.\n{}",
                serde_json::to_string_pretty(&SustainedSoakSummary::canonical_soak_records())
                    .map_err(|e| e.to_string())?
            );
        },
        [command] if command == "restarts" => {
            println!(
                "Synthetic arithmetic only; no restart run observed; C14 NOT QUALIFIED.\n{}",
                serde_json::to_string_pretty(&RestartSummary::canonical_cases())
                    .map_err(|e| e.to_string())?
            );
        },
        [command] if command == "regressions" => {
            println!(
                "Synthetic regression inventory only; no execution receipts; C14 NOT QUALIFIED.\n{}",
                serde_json::to_string_pretty(&(
                    RegressionSuite::historical_regressions(),
                    RegressionSuite::epic_regressions()
                ))
                .map_err(|e| e.to_string())?
            );
        },
        _ => return Err("invalid arguments; use help".into()),
    }
    Ok(())
}
fn main() -> ExitCode {
    match execute(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("platform-soak: {error}");
            ExitCode::FAILURE
        },
    }
}
