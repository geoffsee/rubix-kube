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

fn execute(args: &[String]) -> Result<(), String> {
    match args {
        [] => return execute(&["help".into()]),
        [s] if s == "help" || s == "--help" || s == "-h" => {
            println!(
                "rubix-platform-soak: C14 NOT QUALIFIED\nrun [--version VERSION] (unavailable)\nverify REPORT [--expected-version VERSION] (unavailable)\nfixture [--version VERSION] [--output DIR]\nverify-fixture REPORT [--expected-version VERSION]\nverify-candidate MANIFEST ARTIFACT_DIR --expected-version VERSION\nmatrix | soak | restarts | regressions (synthetic inventories)"
            );
        },
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
