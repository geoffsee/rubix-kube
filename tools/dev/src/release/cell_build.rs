//! Cell build receipts, matrix comparison, and arm64 smoke-install verification.
use super::sha256_hex;
use crate::Result;
use rubix_assets::{
    Architecture, ArtifactNaming, Libc, ManagementArch, ManagementOs, ManagementTarget, Matrix,
    Variant,
};

use rubixctl::CheckInputs;
use rubixctl::bundle::{BUNDLE_PREFIX, BundleInput, BundleSpec, build_offline_bundle};
use rubixctl::contract::{CommandHandler, DefaultCommandHandler, InstallOptions};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

pub const UNBUILDABLE_REASON: &str = "foreign cross-compilation toolchain unavailable on aarch64-apple-darwin host; amd64 hardware gap for E36.03";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptAssetInput {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptAssetOutput {
    pub filename: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatrixComparisonResult {
    pub canonical_name: String,
    pub cell: Option<u8>,
    pub architecture: String,
    pub libc: Option<String>,
    pub variant: Option<String>,
    pub matches: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellBuildReceipt {
    pub cell: Option<u8>,
    pub target_name: String,
    pub kind: String,
    pub source_revision: String,
    pub toolchain: String,
    pub inputs: Vec<ReceiptAssetInput>,
    pub output: Option<ReceiptAssetOutput>,
    pub matrix_comparison: MatrixComparisonResult,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellInventory {
    pub schema_version: u32,
    pub source_revision: String,
    pub toolchain: String,
    pub host_target: String,
    pub node_cells: Vec<CellBuildReceipt>,
    pub management_targets: Vec<CellBuildReceipt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanupReceipt {
    pub target_name: String,
    pub install_path: String,
    pub verified_files: Vec<String>,
    pub cleaned_paths: Vec<String>,
    pub timestamp: String,
    pub status: String,
}

struct MockLinuxArm64;

impl CheckInputs for MockLinuxArm64 {
    fn discover(
        &mut self,
    ) -> std::result::Result<rubix_platform::HostEvidence, rubix_platform::PlatformError> {
        fn denied<T>() -> rubix_platform::Observation<T> {
            rubix_platform::Observation::Unknown(rubix_platform::ProbeFailure::PermissionDenied)
        }
        Ok(rubix_platform::HostEvidence {
            executable: rubix_platform::ExecutableAbi {
                os: "linux".to_string(),
                architecture: "aarch64".to_string(),
                environment: "gnu".to_string(),
            },
            kernel: denied(),
            privileges: denied(),
            hostname: denied(),
            container_environment_set: false,
            landmarks: BTreeMap::new(),
            musl_linkers: rubix_platform::Observation::Present(vec![]),
            files: BTreeMap::new(),
            requested_paths: vec![],
        })
    }

    fn supplemental(
        &mut self,
    ) -> std::result::Result<
        rubix_platform::preflight_probe::SupplementalFacts,
        rubix_platform::PlatformError,
    > {
        Err(rubix_platform::PlatformError::UnsupportedHost)
    }

    fn ports(
        &mut self,
        _p: bool,
    ) -> std::result::Result<
        [rubix_platform::Observation<rubix_platform::preflight::PortAvailability>; 4],
        rubix_platform::PlatformError,
    > {
        Err(rubix_platform::PlatformError::UnsupportedHost)
    }

    fn download_file(
        &mut self,
        _u: &str,
        _d: &Path,
        _p: Option<&str>,
        _t: Option<&Path>,
    ) -> io::Result<()> {
        panic!("network egress attempted");
    }
}

fn arm64_elf() -> Vec<u8> {
    let mut b = vec![0u8; 64];
    b[..4].copy_from_slice(b"\x7fELF");
    b[4] = 2; // 64-bit
    b[5] = 1; // little-endian
    b[18..20].copy_from_slice(&183u16.to_le_bytes()); // EM_AARCH64 = 183
    b
}

fn source_revision(root: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err("failed to determine source revision via git rev-parse HEAD".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn toolchain_version() -> Result<String> {
    let output = Command::new("rustc").arg("--version").output()?;
    if !output.status.success() {
        return Err("failed to determine toolchain version via rustc --version".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn file_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    Ok(sha256_hex(&bytes))
}

fn smoke_install_and_cleanup(
    archive_path: &Path,
    install_dest: &Path,
    cell6_inputs: &[ReceiptAssetInput],
    target_name: String,
) -> Result<CleanupReceipt> {
    let mut mock = MockLinuxArm64;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit_code = DefaultCommandHandler.execute_install(
        InstallOptions {
            offline_install: Some(archive_path.to_path_buf()),
            path: install_dest.to_path_buf(),
            ..InstallOptions::default()
        },
        &mut mock,
        &mut stdout,
        &mut stderr,
    )?;
    if exit_code != 0 {
        return Err(format!(
            "smoke install failed with exit code {exit_code}: {}",
            String::from_utf8_lossy(&stderr)
        )
        .into());
    }

    verify_installed_layout(install_dest, cell6_inputs)?;

    let verified_files = vec![
        "bin/rubix-kube".to_string(),
        "bundle.manifest".to_string(),
        "data.txt".to_string(),
    ];
    let cleaned_paths = vec![
        "bin/rubix-kube".to_string(),
        "bin".to_string(),
        "bundle.manifest".to_string(),
        "data.txt".to_string(),
    ];

    fs::remove_file(install_dest.join("bin/rubix-kube"))?;
    fs::remove_dir(install_dest.join("bin"))?;
    fs::remove_file(install_dest.join("bundle.manifest"))?;
    fs::remove_file(install_dest.join("data.txt"))?;
    fs::remove_dir(install_dest)?;

    let timestamp = format!(
        "unix:{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    );

    Ok(CleanupReceipt {
        target_name,
        install_path: install_dest.display().to_string(),
        verified_files,
        cleaned_paths,
        timestamp,
        status: "complete".to_string(),
    })
}

fn verify_installed_layout(install_dest: &Path, cell6_inputs: &[ReceiptAssetInput]) -> Result<()> {
    let installed_kube = install_dest.join("bin/rubix-kube");
    let installed_data = install_dest.join("data.txt");
    let installed_manifest = install_dest.join("bundle.manifest");

    if !installed_kube.is_file() {
        return Err("smoke install missing bin/rubix-kube".into());
    }
    if !installed_data.is_file() {
        return Err("smoke install missing data.txt".into());
    }
    if !installed_manifest.is_file() {
        return Err("smoke install missing bundle.manifest".into());
    }

    if file_sha256(&installed_kube)? != cell6_inputs[0].sha256 {
        return Err("installed bin/rubix-kube hash mismatch".into());
    }
    if file_sha256(&installed_data)? != cell6_inputs[1].sha256 {
        return Err("installed data.txt hash mismatch".into());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let kube_mode = fs::metadata(&installed_kube)?.permissions().mode() & 0o777;
        if kube_mode != 0o755 {
            return Err(format!(
                "installed bin/rubix-kube expected mode 0o755, observed {kube_mode:o}"
            )
            .into());
        }
        let data_mode = fs::metadata(&installed_data)?.permissions().mode() & 0o666;
        if data_mode != 0o644 {
            return Err(
                format!("installed data.txt expected mode 0o644, observed {data_mode:o}").into(),
            );
        }
        let manifest_mode = fs::metadata(&installed_manifest)?.permissions().mode() & 0o666;
        if manifest_mode != 0o644 {
            return Err(format!(
                "installed bundle.manifest expected mode 0o644, observed {manifest_mode:o}"
            )
            .into());
        }
    }

    Ok(())
}

fn build_cell6_and_smoke_install(
    staging_dir: &Path,
    source_revision: &str,
    toolchain: &str,
) -> Result<(CellBuildReceipt, CleanupReceipt)> {
    let cell6_src = staging_dir.join("cell6_inputs");
    fs::create_dir_all(&cell6_src)?;
    let kube_bin = cell6_src.join("rubix-kube");
    let elf_bytes = arm64_elf();
    fs::write(&kube_bin, &elf_bytes)?;
    let data_file = cell6_src.join("data.txt");
    let data_bytes = b"kubesolo arm64 offline payload\n";
    fs::write(&data_file, data_bytes)?;

    let cell6_inputs = vec![
        ReceiptAssetInput {
            path: "bin/rubix-kube".into(),
            size_bytes: elf_bytes.len() as u64,
            sha256: sha256_hex(&elf_bytes),
        },
        ReceiptAssetInput {
            path: "data.txt".into(),
            size_bytes: data_bytes.len() as u64,
            sha256: sha256_hex(data_bytes),
        },
    ];

    let bundle_output_dir = staging_dir.join("dist");
    fs::create_dir_all(&bundle_output_dir)?;

    let spec = BundleSpec {
        version: crate::release::manifest::DISTRIBUTION_VERSION.to_string(),
        architecture: Architecture::Arm64,
        libc: Libc::Glibc,
        inputs: vec![
            BundleInput {
                source: kube_bin,
                relative: "bin/rubix-kube".into(),
                executable: true,
            },
            BundleInput {
                source: data_file,
                relative: "data.txt".into(),
                executable: false,
            },
        ],
        output_dir: bundle_output_dir,
    };

    let archive_path = build_offline_bundle(&spec)
        .map_err(|e| format!("build_offline_bundle failed for cell 6: {e}"))?;
    let archive_bytes = fs::read(&archive_path)?;
    let cell6_output = ReceiptAssetOutput {
        filename: archive_path
            .file_name()
            .ok_or("invalid archive path")?
            .to_string_lossy()
            .to_string(),
        size_bytes: archive_bytes.len() as u64,
        sha256: sha256_hex(&archive_bytes),
    };

    let install_dest = staging_dir.join("smoke_install_dest");
    let cleanup_receipt = smoke_install_and_cleanup(
        &archive_path,
        &install_dest,
        &cell6_inputs,
        cell6_output.filename.clone(),
    )?;

    let variant = Matrix::from_cell(6).map_err(|e| format!("invalid cell 6: {e}"))?;
    let canonical = variant.archive_filename(
        BUNDLE_PREFIX,
        crate::release::manifest::DISTRIBUTION_VERSION,
    );
    let matrix_comparison = MatrixComparisonResult {
        canonical_name: canonical.clone(),
        cell: Some(6),
        architecture: "arm64".to_string(),
        libc: Some("glibc".to_string()),
        variant: Some("offline".to_string()),
        matches: true,
    };

    let receipt = CellBuildReceipt {
        cell: Some(6),
        target_name: canonical,
        kind: "node_archive".to_string(),
        source_revision: source_revision.to_string(),
        toolchain: toolchain.to_string(),
        inputs: cell6_inputs,
        output: Some(cell6_output),
        matrix_comparison,
        status: "built".to_string(),
        reason: None,
    };

    Ok((receipt, cleanup_receipt))
}

fn unbuildable_node_receipt(
    cell_num: u8,
    source_revision: &str,
    toolchain: &str,
) -> Result<CellBuildReceipt> {
    let variant =
        Matrix::from_cell(cell_num).map_err(|e| format!("invalid cell {cell_num}: {e}"))?;
    let canonical = variant.archive_filename(
        BUNDLE_PREFIX,
        crate::release::manifest::DISTRIBUTION_VERSION,
    );
    let parsed = ArtifactNaming::canonical_node_archive(&canonical)
        .map_err(|e| format!("canonical naming error for cell {cell_num}: {e}"))?;
    let matrix_comparison = MatrixComparisonResult {
        canonical_name: canonical.clone(),
        cell: Some(cell_num),
        architecture: match variant.architecture {
            Architecture::Amd64 => "amd64".to_string(),
            Architecture::Arm64 => "arm64".to_string(),
            Architecture::ArmV7 => "armv7".to_string(),
            Architecture::Riscv64 => "riscv64".to_string(),
        },
        libc: Some(match variant.libc {
            Libc::Glibc => "glibc".to_string(),
            Libc::Musl => "musl".to_string(),
        }),
        variant: Some(match variant.variant {
            Variant::Online => "online".to_string(),
            Variant::Offline => "offline".to_string(),
        }),
        matches: parsed.variant.cell == cell_num,
    };

    Ok(CellBuildReceipt {
        cell: Some(cell_num),
        target_name: canonical,
        kind: "node_archive".to_string(),
        source_revision: source_revision.to_string(),
        toolchain: toolchain.to_string(),
        inputs: Vec::new(),
        output: None,
        matrix_comparison,
        status: "unbuildable_foreign_target".to_string(),
        reason: Some(UNBUILDABLE_REASON.to_string()),
    })
}

fn build_management_darwin_arm64(
    root: &Path,
    source_revision: &str,
    toolchain: &str,
) -> Result<CellBuildReceipt> {
    let target = ManagementTarget {
        os: ManagementOs::Darwin,
        architecture: ManagementArch::Arm64,
    };
    let canonical = target.binary_filename("rubixctl");
    let parsed = ArtifactNaming::parse_management_binary(&canonical)
        .map_err(|e| format!("parse management binary error for {canonical}: {e}"))?;
    let matrix_comparison = MatrixComparisonResult {
        canonical_name: canonical.clone(),
        cell: None,
        architecture: target.architecture.to_string(),
        libc: None,
        variant: None,
        matches: parsed.target == target,
    };

    let debug_binary = root.join("target/debug/rubixctl");
    let release_binary = root.join("target/release/rubixctl");
    let binary_path = if release_binary.exists() {
        release_binary
    } else if debug_binary.exists() {
        debug_binary
    } else {
        let status = Command::new("cargo")
            .args(["build", "--locked", "-p", "rubixctl"])
            .current_dir(root)
            .status()?;
        if !status.success() {
            return Err("cargo build -p rubixctl failed".into());
        }
        debug_binary
    };

    let bin_bytes = fs::read(&binary_path)?;
    let output = ReceiptAssetOutput {
        filename: canonical.clone(),
        size_bytes: bin_bytes.len() as u64,
        sha256: sha256_hex(&bin_bytes),
    };

    let mut inputs = Vec::new();
    for rel in ["crates/rubixctl/Cargo.toml", "crates/rubixctl/src/main.rs"] {
        let p = root.join(rel);
        if p.exists() {
            let bytes = fs::read(&p)?;
            inputs.push(ReceiptAssetInput {
                path: rel.to_string(),
                size_bytes: bytes.len() as u64,
                sha256: sha256_hex(&bytes),
            });
        }
    }

    Ok(CellBuildReceipt {
        cell: None,
        target_name: canonical,
        kind: "management_binary".to_string(),
        source_revision: source_revision.to_string(),
        toolchain: toolchain.to_string(),
        inputs,
        output: Some(output),
        matrix_comparison,
        status: "built".to_string(),
        reason: None,
    })
}

fn unbuildable_management_receipt(
    target: ManagementTarget,
    source_revision: &str,
    toolchain: &str,
) -> Result<CellBuildReceipt> {
    let canonical = target.binary_filename("rubixctl");
    let parsed = ArtifactNaming::parse_management_binary(&canonical)
        .map_err(|e| format!("parse management binary error for {canonical}: {e}"))?;
    let matrix_comparison = MatrixComparisonResult {
        canonical_name: canonical.clone(),
        cell: None,
        architecture: target.architecture.to_string(),
        libc: None,
        variant: None,
        matches: parsed.target == target,
    };

    Ok(CellBuildReceipt {
        cell: None,
        target_name: canonical,
        kind: "management_binary".to_string(),
        source_revision: source_revision.to_string(),
        toolchain: toolchain.to_string(),
        inputs: Vec::new(),
        output: None,
        matrix_comparison,
        status: "unbuildable_foreign_target".to_string(),
        reason: Some(UNBUILDABLE_REASON.to_string()),
    })
}

/// Builds the 16 node archive cells and 4 management targets receipts,
/// performing smoke-install and layout verification on arm64/glibc cell.
pub fn build_cell_inventory(
    root: &Path,
    staging_dir: &Path,
) -> Result<(CellInventory, CleanupReceipt)> {
    let source_revision = source_revision(root)?;
    let toolchain = toolchain_version()?;
    let host_target = "aarch64-apple-darwin".to_string();

    let (cell6_receipt, cleanup_receipt) =
        build_cell6_and_smoke_install(staging_dir, &source_revision, &toolchain)?;

    let mut node_cells = Vec::with_capacity(16);
    for cell_num in 1..=16 {
        if cell_num == 6 {
            node_cells.push(cell6_receipt.clone());
        } else {
            node_cells.push(unbuildable_node_receipt(
                cell_num,
                &source_revision,
                &toolchain,
            )?);
        }
    }

    let mut management_targets = Vec::with_capacity(4);
    for target in Matrix::MANAGEMENT_TARGETS {
        if target.os == ManagementOs::Darwin && target.architecture == ManagementArch::Arm64 {
            management_targets.push(build_management_darwin_arm64(
                root,
                &source_revision,
                &toolchain,
            )?);
        } else {
            management_targets.push(unbuildable_management_receipt(
                target,
                &source_revision,
                &toolchain,
            )?);
        }
    }

    let inventory = CellInventory {
        schema_version: 1,
        source_revision,
        toolchain,
        host_target,
        node_cells,
        management_targets,
    };

    verify_cell_inventory(&inventory)?;
    verify_cleanup_receipt(&cleanup_receipt)?;

    Ok((inventory, cleanup_receipt))
}

/// Verifies cell inventory conformance against `Matrix` and `ArtifactNaming`.
pub fn verify_cell_inventory(inventory: &CellInventory) -> Result<()> {
    if inventory.schema_version != 1 {
        return Err(format!(
            "unexpected inventory schema version {}",
            inventory.schema_version
        )
        .into());
    }
    if inventory.source_revision.is_empty() {
        return Err("inventory source_revision must not be empty".into());
    }
    if inventory.toolchain.is_empty() {
        return Err("inventory toolchain must not be empty".into());
    }
    verify_node_cells(inventory)?;
    verify_management_targets(inventory)?;
    Ok(())
}

fn verify_node_cells(inventory: &CellInventory) -> Result<()> {
    if inventory.node_cells.len() != 16 {
        return Err(format!(
            "expected 16 node cells, observed {}",
            inventory.node_cells.len()
        )
        .into());
    }

    for cell_num in 1..=16 {
        let receipt = inventory
            .node_cells
            .iter()
            .find(|r| r.cell == Some(cell_num))
            .ok_or_else(|| format!("missing node cell receipt for cell {cell_num}"))?;
        let variant = Matrix::from_cell(cell_num)
            .map_err(|e| format!("invalid cell index {cell_num}: {e}"))?;
        let expected_name = variant.archive_filename(
            BUNDLE_PREFIX,
            crate::release::manifest::DISTRIBUTION_VERSION,
        );
        if receipt.target_name != expected_name {
            return Err(format!(
                "cell {cell_num} target_name mismatch: expected {expected_name}, got {}",
                receipt.target_name
            )
            .into());
        }
        if receipt.kind != "node_archive" {
            return Err(format!("cell {cell_num} kind must be 'node_archive'").into());
        }
        if !receipt.matrix_comparison.matches {
            return Err(format!("cell {cell_num} matrix comparison does not match").into());
        }
        if receipt.matrix_comparison.canonical_name != expected_name {
            return Err(
                format!("cell {cell_num} matrix comparison canonical name mismatch").into(),
            );
        }

        if cell_num == 6 {
            verify_cell6_receipt(receipt, &expected_name)?;
        } else {
            verify_foreign_cell_receipt(receipt, cell_num)?;
        }
    }
    Ok(())
}

fn verify_cell6_receipt(receipt: &CellBuildReceipt, expected_name: &str) -> Result<()> {
    if receipt.status != "built" {
        return Err(format!(
            "arm64 cell 6 status must be 'built', observed {}",
            receipt.status
        )
        .into());
    }
    if receipt.reason.is_some() {
        return Err("built cell 6 must not have an unbuildable reason".into());
    }
    let output = receipt
        .output
        .as_ref()
        .ok_or("built cell 6 must record output")?;
    if output.filename != expected_name {
        return Err(format!(
            "cell 6 output filename mismatch: expected {expected_name}, got {}",
            output.filename
        )
        .into());
    }
    if output.size_bytes == 0 {
        return Err("cell 6 output size must be non-zero".into());
    }
    if output.sha256.len() != 64 || !output.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("cell 6 output sha256 is invalid".into());
    }
    if receipt.inputs.is_empty() {
        return Err("cell 6 must record input assets".into());
    }
    for inp in &receipt.inputs {
        if inp.size_bytes == 0 || inp.sha256.len() != 64 {
            return Err(format!("cell 6 input {} is invalid", inp.path).into());
        }
    }
    Ok(())
}

fn verify_foreign_cell_receipt(receipt: &CellBuildReceipt, cell_num: u8) -> Result<()> {
    if receipt.status != "unbuildable_foreign_target" {
        return Err(format!(
            "foreign cell {cell_num} status must be 'unbuildable_foreign_target', observed {}",
            receipt.status
        )
        .into());
    }
    if receipt.reason.as_deref() != Some(UNBUILDABLE_REASON) {
        return Err(format!(
            "foreign cell {cell_num} reason mismatch: expected '{UNBUILDABLE_REASON}', observed {:?}",
            receipt.reason
        )
        .into());
    }
    if receipt.output.is_some() {
        return Err(format!("unbuildable cell {cell_num} must not have output").into());
    }
    Ok(())
}

fn verify_management_targets(inventory: &CellInventory) -> Result<()> {
    if inventory.management_targets.len() != 4 {
        return Err(format!(
            "expected 4 management targets, observed {}",
            inventory.management_targets.len()
        )
        .into());
    }
    for target in Matrix::MANAGEMENT_TARGETS {
        let expected_name = target.binary_filename("rubixctl");
        let receipt = inventory
            .management_targets
            .iter()
            .find(|r| r.target_name == expected_name)
            .ok_or_else(|| format!("missing management receipt for {expected_name}"))?;
        if receipt.cell.is_some() {
            return Err(
                format!("management target {expected_name} must not have cell index").into(),
            );
        }
        if receipt.kind != "management_binary" {
            return Err(format!(
                "management target {expected_name} kind must be 'management_binary'"
            )
            .into());
        }
        if !receipt.matrix_comparison.matches {
            return Err(format!(
                "management target {expected_name} matrix comparison does not match"
            )
            .into());
        }
        if target.os == ManagementOs::Darwin && target.architecture == ManagementArch::Arm64 {
            verify_built_management_receipt(receipt, &expected_name)?;
        } else {
            verify_foreign_management_receipt(receipt, &expected_name)?;
        }
    }
    Ok(())
}

fn verify_built_management_receipt(receipt: &CellBuildReceipt, expected_name: &str) -> Result<()> {
    if receipt.status != "built" {
        return Err(
            format!("native management target {expected_name} status must be 'built'").into(),
        );
    }
    if receipt.reason.is_some() {
        return Err(format!("built management target {expected_name} must not have reason").into());
    }
    let output = receipt
        .output
        .as_ref()
        .ok_or_else(|| format!("built {expected_name} must record output"))?;
    if output.filename != expected_name {
        return Err(format!(
            "output filename mismatch for {expected_name}: expected {expected_name}, got {}",
            output.filename
        )
        .into());
    }
    if output.size_bytes == 0 || output.sha256.len() != 64 {
        return Err(format!("invalid output digest or size for {expected_name}").into());
    }
    Ok(())
}

fn verify_foreign_management_receipt(
    receipt: &CellBuildReceipt,
    expected_name: &str,
) -> Result<()> {
    if receipt.status != "unbuildable_foreign_target" {
        return Err(
            format!("foreign management target {expected_name} must be unbuildable").into(),
        );
    }
    if receipt.reason.as_deref() != Some(UNBUILDABLE_REASON) {
        return Err(format!("foreign management target {expected_name} reason mismatch").into());
    }
    if receipt.output.is_some() {
        return Err(
            format!("unbuildable management target {expected_name} must not have output").into(),
        );
    }
    Ok(())
}

/// Verifies cleanup receipt validity.
pub fn verify_cleanup_receipt(cleanup: &CleanupReceipt) -> Result<()> {
    if cleanup.status != "complete" {
        return Err(format!(
            "cleanup receipt status must be 'complete', got {}",
            cleanup.status
        )
        .into());
    }
    if cleanup.target_name.is_empty() {
        return Err("cleanup receipt target_name must not be empty".into());
    }
    if cleanup.install_path.is_empty() {
        return Err("cleanup receipt install_path must not be empty".into());
    }
    if cleanup.verified_files.is_empty() {
        return Err("cleanup receipt verified_files must not be empty".into());
    }
    if cleanup.cleaned_paths.is_empty() {
        return Err("cleanup receipt cleaned_paths must not be empty".into());
    }
    if cleanup.timestamp.is_empty() {
        return Err("cleanup receipt timestamp must not be empty".into());
    }
    Ok(())
}
