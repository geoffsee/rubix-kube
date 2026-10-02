//! Validate and inspect the supported variant matrix and clean-checkout artifact naming.
use rubix_assets::{Architecture, ArtifactNaming, Libc, Matrix, OptionalFeature, Variant};
use std::process::ExitCode;

fn validate_all() -> Result<(), String> {
    let variants = Matrix::all_node_variants();
    if variants.len() != 16 {
        return Err(format!("expected 16 node variants, got {}", variants.len()));
    }

    for (index, variant) in variants.iter().enumerate() {
        let expected_cell =
            u8::try_from(index + 1).map_err(|e| format!("cell index conversion failed: {e}"))?;
        if variant.cell != expected_cell {
            return Err(format!(
                "cell mismatch: expected {expected_cell}, got {}",
                variant.cell
            ));
        }

        let canonical_name =
            ArtifactNaming::validate_clean_checkout_inputs("rubix-kube", "0.1.0", *variant)
                .map_err(|e| {
                    format!("clean-checkout validation failed for cell {expected_cell}: {e}")
                })?;

        let parsed = ArtifactNaming::parse_node_archive(&canonical_name)
            .map_err(|e| format!("parse failed for {canonical_name}: {e}"))?;

        if parsed.variant.cell != expected_cell {
            return Err(format!(
                "parsed cell mismatch for {canonical_name}: expected {expected_cell}, got {}",
                parsed.variant.cell
            ));
        }
    }

    let management_targets = Matrix::all_management_targets();
    if management_targets.len() != 4 {
        return Err(format!(
            "expected 4 management targets, got {}",
            management_targets.len()
        ));
    }

    for target in management_targets {
        let binary_name = target.binary_filename("rubixctl");
        let parsed = ArtifactNaming::parse_management_binary(&binary_name)
            .map_err(|e| format!("management binary parse failed for {binary_name}: {e}"))?;

        if parsed.target != *target {
            return Err(format!(
                "management target mismatch for {binary_name}: expected {target:?}, got {:?}",
                parsed.target
            ));
        }
    }

    Ok(())
}

fn print_cells() {
    println!("Supported Node Variant Matrix (16 cells):");
    println!(
        "{:<5} {:<18} {:<8} {:<10} {:<16} {:<30}",
        "Cell", "Architecture", "Libc", "Variant", "OCI Platform", "Optional Image Limits"
    );
    println!("{:-<95}", "");

    for variant in Matrix::all_node_variants() {
        let limits = match variant.architecture {
            Architecture::Amd64 | Architecture::Arm64 => "Portainer & D2K supported",
            Architecture::ArmV7 => "Portainer supported; D2K disabled",
            Architecture::Riscv64 => "Portainer unavailable; D2K disabled",
        };

        let arch_str = match variant.architecture {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
            Architecture::ArmV7 => "ARMv7 hard-float",
            Architecture::Riscv64 => "riscv64",
        };

        let libc_str = match variant.libc {
            Libc::Glibc => "glibc",
            Libc::Musl => "musl",
        };

        let variant_str = match variant.variant {
            Variant::Online => "online",
            Variant::Offline => "offline",
        };

        println!(
            "{:<5} {:<18} {:<8} {:<10} {:<16} {:<30}",
            variant.cell,
            arch_str,
            libc_str,
            variant_str,
            variant.oci_platform(),
            limits
        );
    }

    println!("\nManagement Targets (4 targets, Windows excluded by E01):");
    for target in Matrix::all_management_targets() {
        println!("  - {}-{}", target.os, target.architecture);
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => match validate_all() {
            Ok(()) => {
                println!(
                    "all 16 node variant cells and 4 management targets validated successfully"
                );
                ExitCode::SUCCESS
            },
            Err(e) => {
                eprintln!("matrix validation error: {e}");
                ExitCode::FAILURE
            },
        },
        [cmd] if cmd == "validate" => match validate_all() {
            Ok(()) => {
                println!(
                    "all 16 node variant cells and 4 management targets validated successfully"
                );
                ExitCode::SUCCESS
            },
            Err(e) => {
                eprintln!("matrix validation error: {e}");
                ExitCode::FAILURE
            },
        },
        [cmd] if cmd == "list" => {
            print_cells();
            ExitCode::SUCCESS
        },
        [cmd, filename] if cmd == "archive" => match ArtifactNaming::parse_node_archive(filename) {
            Ok(parsed) => {
                println!("Parsed node archive:");
                println!("  Prefix:       {}", parsed.prefix);
                println!("  Version:      {}", parsed.version);
                println!("  Cell:         {}", parsed.variant.cell);
                println!("  OCI Platform: {}", parsed.variant.oci_platform());
                println!(
                    "  Portainer:    {:?}",
                    parsed
                        .variant
                        .optional_feature_support(OptionalFeature::PortainerAgent)
                );
                println!(
                    "  D2K:          {:?}",
                    parsed
                        .variant
                        .optional_feature_support(OptionalFeature::D2k)
                );
                println!(
                    "  LocalPath:    {:?}",
                    parsed
                        .variant
                        .optional_feature_support(OptionalFeature::LocalPathStorage)
                );
                ExitCode::SUCCESS
            },
            Err(e) => {
                eprintln!("invalid archive filename: {e}");
                ExitCode::FAILURE
            },
        },
        [cmd, filename] if cmd == "binary" => {
            match ArtifactNaming::parse_management_binary(filename) {
                Ok(parsed) => {
                    println!("Parsed management binary:");
                    println!("  Prefix:       {}", parsed.prefix);
                    println!("  OS:           {}", parsed.target.os);
                    println!("  Architecture: {}", parsed.target.architecture);
                    ExitCode::SUCCESS
                },
                Err(e) => {
                    eprintln!("invalid binary filename: {e}");
                    ExitCode::FAILURE
                },
            }
        },
        _ => {
            eprintln!(
                "Usage: rubix-matrix [validate | list | archive <filename> | binary <filename>]"
            );
            ExitCode::FAILURE
        },
    }
}
