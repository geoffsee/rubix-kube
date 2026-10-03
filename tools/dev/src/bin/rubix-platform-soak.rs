//! CLI qualification tool for platform coverage and sustained soak (Epic E28 / Issue #120).
//!
//! Validates the exhaustive platform/runtime/variant/container matrix, asserts candidate
//! digests match tested releases, confirms sustained 24-hour soak memory stability within
//! contractual bounds, verifies restart/crash recovery timing bounds, and tracks all
//! historical (REG-01..REG-10) and per-epic (E01..E30) regressions.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::similar_names
)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rubix_dev::platform_soak::{
    DimensionCategory, EnvironmentMapping, PlatformSoakReport, PlatformSoakRunner, RegressionSuite,
    RestartSummary, SustainedSoakSummary,
};

fn print_matrix() {
    println!("Accepted Platform & Environment Matrix (Exhaustive Coverage):\n");
    let records = EnvironmentMapping::canonical_matrix();

    let categories = [
        DimensionCategory::NodeVariantCell,
        DimensionCategory::ManagementTarget,
        DimensionCategory::OciContainerImage,
        DimensionCategory::RuntimeProvider,
        DimensionCategory::RuntimeBuildMode,
        DimensionCategory::InitSystem,
        DimensionCategory::ContainerRunMode,
        DimensionCategory::HostCapability,
        DimensionCategory::DeliveryMode,
        DimensionCategory::RuntimeOwnership,
        DimensionCategory::AddonComponent,
    ];

    for cat in categories {
        println!("=== {} ===", cat.display_name());
        for rec in records.iter().filter(|r| r.dimension == cat) {
            let status = match &rec.support_status {
                rubix_dev::platform_soak::SupportStatus::Supported => "Supported".to_string(),
                rubix_dev::platform_soak::SupportStatus::Unsupported { rationale } => {
                    format!("Unsupported ({rationale})")
                },
            };
            println!("  - [{}] {}: {}", rec.id, rec.name, status);
        }
        println!();
    }
}

fn print_soak() {
    println!("Sustained 24-Hour Soak Contractual Bounds:\n");
    let records = SustainedSoakSummary::canonical_soak_records();
    for rec in records {
        println!("Target Architecture: {}", rec.target_architecture);
        println!(
            "  Duration: {}h ({}s)",
            rec.declared_duration_hours, rec.actual_duration_seconds
        );
        println!("  Workload Cycles: {}", rec.workload_cycles_completed);
        println!(
            "  Memory: {:.2} MiB -> {:.2} MiB (derived growth: {:.3}x, limit: 1.10x)",
            rec.initial_settled_idle_rss_bytes as f64 / (1024.0 * 1024.0),
            rec.final_settled_idle_rss_bytes as f64 / (1024.0 * 1024.0),
            rec.derived_growth_ratio
        );
        println!(
            "  Failures: {} OOMs, {} crashes, {} probe failures",
            rec.oom_count, rec.crash_count, rec.unexplained_probe_failures
        );
        println!("  Status: {}", if rec.passed { "PASS" } else { "FAIL" });
        println!();
    }
}

fn print_restarts() {
    println!("Restart & Recovery Timing Bounds:\n");
    let cases = RestartSummary::canonical_cases();
    for case in cases {
        println!(
            "  [{}] {}: bound='{}', observed={}ms, state_preserved={}, status={}",
            case.id,
            case.name,
            case.declared_bound,
            case.observed_duration_ms,
            case.state_preserved,
            if case.passed { "PASS" } else { "FAIL" }
        );
    }
    println!();
}

fn print_regressions() {
    println!("Historical Regressions (REG-01 through REG-10):\n");
    let regs = RegressionSuite::historical_regressions();
    for r in regs {
        println!(
            "  [{}] {} (upstream: {}): bound='{}', status={}",
            r.id,
            r.description,
            r.upstream_ref,
            r.declared_bound,
            if r.passed { "PASS" } else { "FAIL" }
        );
    }
    println!();
}

fn handle_run(args: &[String]) -> ExitCode {
    let mut output_dir: Option<PathBuf> = None;
    let mut version = "0.1.0".to_string();

    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--output" => {
                if idx + 1 < args.len() {
                    output_dir = Some(PathBuf::from(&args[idx + 1]));
                    idx += 2;
                } else {
                    eprintln!("error: --output requires a directory path");
                    return ExitCode::FAILURE;
                }
            },
            "--version" => {
                if idx + 1 < args.len() {
                    version.clone_from(&args[idx + 1]);
                    idx += 2;
                } else {
                    eprintln!("error: --version requires a version string");
                    return ExitCode::FAILURE;
                }
            },
            _ => {
                idx += 1;
            },
        }
    }

    let runner = PlatformSoakRunner::new();
    let report = match runner.run_qualification(&version, None) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("qualification run failed: {e}");
            return ExitCode::FAILURE;
        },
    };

    println!(
        "Qualification complete: {} environments mapped, {} candidate artifacts verified, {} soak profiles passed, {} restart bounds met, {} regressions verified",
        report.environments.len(),
        report.candidate_verification.total_artifacts,
        report.soak_results.len(),
        report.restart_results.len(),
        report.historical_regressions.len()
    );

    if let Some(dir) = output_dir {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("failed to create output directory '{}': {e}", dir.display());
            return ExitCode::FAILURE;
        }

        let json_path = dir.join("platform-soak-report.json");
        let json_data = match serde_json::to_string_pretty(&report) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("failed to serialize report to JSON: {e}");
                return ExitCode::FAILURE;
            },
        };
        if let Err(e) = std::fs::write(&json_path, json_data) {
            eprintln!(
                "failed to write JSON report to '{}': {e}",
                json_path.display()
            );
            return ExitCode::FAILURE;
        }
        println!("Wrote JSON report to: {}", json_path.display());

        let md_path = dir.join("platform-soak-report.md");
        let md_data = report.to_markdown();
        if let Err(e) = std::fs::write(&md_path, md_data) {
            eprintln!(
                "failed to write Markdown report to '{}': {e}",
                md_path.display()
            );
            return ExitCode::FAILURE;
        }
        println!("Wrote Markdown report to: {}", md_path.display());
    }

    ExitCode::SUCCESS
}

fn handle_verify(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!(
            "Usage: rubix-platform-soak verify <report-json-file> [--expected-version <version>]"
        );
        return ExitCode::FAILURE;
    }

    let file_path = Path::new(&args[0]);
    let mut expected_version: Option<&str> = None;

    let mut idx = 1;
    while idx < args.len() {
        if args[idx] == "--expected-version" && idx + 1 < args.len() {
            expected_version = Some(&args[idx + 1]);
            idx += 2;
        } else {
            idx += 1;
        }
    }

    let data = match std::fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("failed to read report file '{}': {e}", file_path.display());
            return ExitCode::FAILURE;
        },
    };

    let report: PlatformSoakReport = match serde_json::from_slice(&data) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("failed to parse JSON report '{}': {e}", file_path.display());
            return ExitCode::FAILURE;
        },
    };

    match report.validate(expected_version) {
        Ok(()) => {
            println!(
                "Report '{}' (version {}) successfully validated: all {} environments mapped with zero silent omissions, candidate digests matched, soak bounds met, restart bounds met, and all regressions passed.",
                file_path.display(),
                report.version,
                report.environments.len()
            );
            ExitCode::SUCCESS
        },
        Err(e) => {
            eprintln!("report validation failed: {e}");
            ExitCode::FAILURE
        },
    }
}

fn print_help() {
    println!(
        "rubix-platform-soak: Platform matrix coverage and sustained soak qualification tool (Issue #120)\n"
    );
    println!("Usage:");
    println!("  rubix-platform-soak run [--output <dir>] [--version <ver>]");
    println!("  rubix-platform-soak verify <report-json-file> [--expected-version <ver>]");
    println!("  rubix-platform-soak matrix");
    println!("  rubix-platform-soak soak");
    println!("  rubix-platform-soak restarts");
    println!("  rubix-platform-soak regressions");
    println!("  rubix-platform-soak help");
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.as_slice() {
        [] => {
            print_help();
            ExitCode::SUCCESS
        },
        [cmd, rest @ ..] if cmd == "run" => handle_run(rest),
        [cmd, rest @ ..] if cmd == "verify" => handle_verify(rest),
        [cmd] if cmd == "matrix" => {
            print_matrix();
            ExitCode::SUCCESS
        },
        [cmd] if cmd == "soak" => {
            print_soak();
            ExitCode::SUCCESS
        },
        [cmd] if cmd == "restarts" => {
            print_restarts();
            ExitCode::SUCCESS
        },
        [cmd] if cmd == "regressions" => {
            print_regressions();
            ExitCode::SUCCESS
        },
        [cmd] if cmd == "help" || cmd == "--help" || cmd == "-h" => {
            print_help();
            ExitCode::SUCCESS
        },
        _ => {
            eprintln!("unknown command. Run 'rubix-platform-soak help' for usage.");
            ExitCode::FAILURE
        },
    }
}
