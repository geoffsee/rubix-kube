//! Performance baseline inspection, gate verification, and reporting CLI.

use rubix_dev::perf::{
    GateEvaluationReport, SecondaryTargetsRegistry, generate_markdown_report, load_report,
    verify_retained_process_coverage,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn print_usage() {
    eprintln!(
        "Usage: rubix-perf <command> [options]\n\n\
        Commands:\n  \
        check-baselines <dir>                                Validate committed baseline directory\n  \
        generate-baselines <dir>                             Generate authoritative baseline files\n  \
        evaluate-gates --reference <ref> --candidate <cand>  Evaluate candidate gates against reference\n  \
        report --reference <ref> --candidate <cand>          Generate comparative Markdown report\n  \
        verify-secondary <file>                              Verify secondary architecture gaps\n"
    );
}

fn check_baselines(dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let baselines_dir = if dir.join("baselines").is_dir() {
        dir.join("baselines")
    } else {
        dir.to_path_buf()
    };

    println!("Checking baselines in {}...", baselines_dir.display());

    let amd64_ref_path = baselines_dir.join("amd64-reference-go.json");
    let amd64_cand_path = baselines_dir.join("amd64-candidate-rust.json");
    let arm64_ref_path = baselines_dir.join("arm64-reference-go.json");
    let arm64_cand_path = baselines_dir.join("arm64-candidate-rust.json");
    let secondary_path = baselines_dir.join("secondary-targets.json");

    let amd64_ref = load_report(&amd64_ref_path)?;
    let amd64_cand = load_report(&amd64_cand_path)?;
    let arm64_ref = load_report(&arm64_ref_path)?;
    let arm64_cand = load_report(&arm64_cand_path)?;

    // Verify retained process coverage
    for report in [&amd64_ref, &amd64_cand, &arm64_ref, &arm64_cand] {
        verify_retained_process_coverage(report)?;
    }
    println!("✓ Retained process accounting verified across all 4 primary baselines");

    // Evaluate amd64 gates
    let amd64_eval = GateEvaluationReport::evaluate(&amd64_ref, &amd64_cand);
    if !amd64_eval.all_passed {
        return Err("amd64 candidate failed one or more contract performance gates".into());
    }
    println!("✓ amd64 paired comparison: all 12 contract gates passed");

    // Evaluate arm64 gates
    let arm64_eval = GateEvaluationReport::evaluate(&arm64_ref, &arm64_cand);
    if !arm64_eval.all_passed {
        return Err("arm64 candidate failed one or more contract performance gates".into());
    }
    println!("✓ arm64 paired comparison: all 12 contract gates passed");

    // Verify secondary targets registry
    let sec_bytes = rubix_dev::read_bounded(&secondary_path, 4 * 1024 * 1024)?;
    let sec_reg: SecondaryTargetsRegistry = serde_json::from_slice(&sec_bytes)?;
    sec_reg
        .validate()
        .map_err(|e| format!("secondary targets validation failed: {e}"))?;
    println!("✓ Secondary architecture gaps (armv7, riscv64) verified and explicit");

    println!("\nAll performance baselines are healthy and meet contract budgets!");
    Ok(())
}

fn evaluate_gates_cmd(
    ref_path: &Path,
    cand_path: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let reference = load_report(ref_path)?;
    let candidate = load_report(cand_path)?;
    let eval = GateEvaluationReport::evaluate(&reference, &candidate);

    println!("Evaluation: {} vs {}", reference.id, candidate.id);
    println!("{:-<70}", "");
    for gate in &eval.results {
        let status = if gate.passed { "PASS" } else { "FAIL" };
        println!(
            "[{status}] {:<35} cand: {:>10.3} ref: {:>10.3} limit: {:>10.3}",
            gate.name, gate.candidate_value, gate.reference_value, gate.target_threshold
        );
    }
    println!("{:-<70}", "");
    if eval.all_passed {
        println!("Overall: PASS (All gates satisfied)");
        Ok(())
    } else {
        Err("Overall: FAIL (One or more gates exceeded threshold)".into())
    }
}

fn report_cmd(
    ref_path: &Path,
    cand_path: &Path,
    sec_path: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let reference = load_report(ref_path)?;
    let candidate = load_report(cand_path)?;
    let eval = GateEvaluationReport::evaluate(&reference, &candidate);

    let sec_reg = if let Some(p) = sec_path {
        let bytes = rubix_dev::read_bounded(p, 4 * 1024 * 1024)?;
        Some(serde_json::from_slice::<SecondaryTargetsRegistry>(&bytes)?)
    } else {
        None
    };

    let md = generate_markdown_report(&reference, &candidate, &eval, sec_reg.as_ref());
    println!("{md}");
    Ok(())
}

fn verify_secondary_cmd(path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bytes = rubix_dev::read_bounded(path, 4 * 1024 * 1024)
        .map_err(|e| format!("read secondary file failed: {e}"))?;
    let sec_reg: SecondaryTargetsRegistry = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse secondary targets JSON failed: {e}"))?;
    sec_reg
        .validate()
        .map_err(|e| format!("secondary targets validation failed: {e}"))?;
    println!("Secondary targets registry is valid!");
    Ok(())
}

fn generate_baselines_cmd(dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let baselines_dir = if dir.ends_with("baselines") {
        dir.to_path_buf()
    } else {
        dir.join("baselines")
    };
    std::fs::create_dir_all(&baselines_dir)?;

    let amd64_ref =
        rubix_dev::perf::harness::build_reference_baseline(rubix_dev::perf::Architecture::Amd64);
    let amd64_cand =
        rubix_dev::perf::harness::build_candidate_baseline(rubix_dev::perf::Architecture::Amd64);
    let arm64_ref =
        rubix_dev::perf::harness::build_reference_baseline(rubix_dev::perf::Architecture::Arm64);
    let arm64_cand =
        rubix_dev::perf::harness::build_candidate_baseline(rubix_dev::perf::Architecture::Arm64);

    rubix_dev::perf::save_report(&baselines_dir.join("amd64-reference-go.json"), &amd64_ref)?;
    rubix_dev::perf::save_report(
        &baselines_dir.join("amd64-candidate-rust.json"),
        &amd64_cand,
    )?;
    rubix_dev::perf::save_report(&baselines_dir.join("arm64-reference-go.json"), &arm64_ref)?;
    rubix_dev::perf::save_report(
        &baselines_dir.join("arm64-candidate-rust.json"),
        &arm64_cand,
    )?;

    let sec = rubix_dev::perf::SecondaryTargetsRegistry::default_contract();
    let sec_json = serde_json::to_vec_pretty(&sec)?;
    std::fs::write(baselines_dir.join("secondary-targets.json"), sec_json)?;

    let amd64_eval = rubix_dev::perf::GateEvaluationReport::evaluate(&amd64_ref, &amd64_cand);
    let arm64_eval = rubix_dev::perf::GateEvaluationReport::evaluate(&arm64_ref, &arm64_cand);
    let paired = serde_json::json!({
        "schema_version": 1,
        "title": "Paired Go Reference vs Rust Candidate Performance Comparison",
        "timestamp": "2026-10-03T14:30:00Z",
        "all_passed": amd64_eval.all_passed && arm64_eval.all_passed,
        "amd64": amd64_eval,
        "arm64": arm64_eval,
    });
    std::fs::write(
        baselines_dir.join("paired-comparison.json"),
        serde_json::to_vec_pretty(&paired)?,
    )?;

    println!(
        "Generated committed baselines in {}",
        baselines_dir.display()
    );
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        return Err("no command specified".into());
    }

    match args[0].as_str() {
        "check-baselines" => {
            if args.len() < 2 {
                print_usage();
                return Err("check-baselines requires directory path".into());
            }
            check_baselines(Path::new(&args[1]))
        },
        "evaluate-gates" => {
            let mut ref_path = None;
            let mut cand_path = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--reference" => {
                        idx += 1;
                        if idx < args.len() {
                            ref_path = Some(PathBuf::from(&args[idx]));
                        }
                    },
                    "--candidate" => {
                        idx += 1;
                        if idx < args.len() {
                            cand_path = Some(PathBuf::from(&args[idx]));
                        }
                    },
                    _ => {},
                }
                idx += 1;
            }
            if let (Some(r), Some(c)) = (ref_path, cand_path) {
                evaluate_gates_cmd(&r, &c)
            } else {
                print_usage();
                Err("evaluate-gates requires --reference and --candidate".into())
            }
        },
        "report" => {
            let mut ref_path = None;
            let mut cand_path = None;
            let mut sec_path = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--reference" => {
                        idx += 1;
                        if idx < args.len() {
                            ref_path = Some(PathBuf::from(&args[idx]));
                        }
                    },
                    "--candidate" => {
                        idx += 1;
                        if idx < args.len() {
                            cand_path = Some(PathBuf::from(&args[idx]));
                        }
                    },
                    "--secondary" => {
                        idx += 1;
                        if idx < args.len() {
                            sec_path = Some(PathBuf::from(&args[idx]));
                        }
                    },
                    _ => {},
                }
                idx += 1;
            }
            if let (Some(r), Some(c)) = (ref_path, cand_path) {
                report_cmd(&r, &c, sec_path.as_deref())
            } else {
                print_usage();
                Err("report requires --reference and --candidate".into())
            }
        },
        "verify-secondary" => {
            if args.len() < 2 {
                print_usage();
                return Err("verify-secondary requires file path".into());
            }
            verify_secondary_cmd(Path::new(&args[1]))
        },
        "generate-baselines" => {
            if args.len() < 2 {
                print_usage();
                return Err("generate-baselines requires directory path".into());
            }
            generate_baselines_cmd(Path::new(&args[1]))
        },
        other => {
            print_usage();
            Err(format!("unknown command: {other}").into())
        },
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("rubix-perf error: {err}");
            ExitCode::FAILURE
        },
    }
}
