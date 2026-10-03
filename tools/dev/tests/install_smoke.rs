use rubix_assets::{Architecture, Libc, Matrix};
use rubix_dev::provenance::{ChecksumManifest, generate_manifest, generate_source_provenance};
use rubix_platform::preflight::PortAvailability;
use rubix_platform::preflight_probe::SupplementalFacts;
use rubix_platform::{ExecutableAbi, HostEvidence, Observation, PlatformError, ProbeFailure};
use rubixctl::CheckInputs;
use rubixctl::bundle::{BUNDLE_PREFIX, BundleInput, BundleSpec, build_offline_bundle};
use rubixctl::contract::{CommandHandler, DefaultCommandHandler, InstallOptions};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use tempfile::TempDir;

const GENERATOR: &str = r#"{"sources":[{"id":"openapi","path":"a","url":"https://x/a","sha256":"483500149ee52ce5753d75f5639101d985bb4f5e902cc05b1ba7627465d62446","bytes":3}]}"#;
const INVENTORY: &str = r#"{"baseline":"2ef1c47","sources":[{"repository":"https://github.com/k/k","tag":"v1","commit":"abc"}]}"#;

struct Mock {
    arch: String,
    libc: String,
}

impl CheckInputs for Mock {
    fn discover(&mut self) -> Result<HostEvidence, PlatformError> {
        fn denied<T>() -> Observation<T> {
            Observation::Unknown(ProbeFailure::PermissionDenied)
        }
        Ok(HostEvidence {
            executable: ExecutableAbi {
                os: "linux".to_string(),
                architecture: self.arch.clone(),
                environment: self.libc.clone(),
            },
            kernel: denied(),
            privileges: denied(),
            hostname: denied(),
            container_environment_set: false,
            landmarks: BTreeMap::new(),
            musl_linkers: denied(),
            files: BTreeMap::new(),
            requested_paths: vec![],
        })
    }
    fn supplemental(&mut self) -> Result<SupplementalFacts, PlatformError> {
        Err(PlatformError::UnsupportedHost)
    }
    fn ports(&mut self, _p: bool) -> Result<[Observation<PortAvailability>; 4], PlatformError> {
        Err(PlatformError::UnsupportedHost)
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

fn elf(machine: u16) -> Vec<u8> {
    let mut b = vec![0u8; 64];
    b[..4].copy_from_slice(b"\x7fELF");
    b[4] = 2;
    b[5] = 1;
    b[18..20].copy_from_slice(&machine.to_le_bytes());
    b
}

#[test]
fn layout_smoke_verification_all_cells() {
    let dist = TempDir::new().unwrap();
    let version = "0.1.0";

    // 1. Build all 16 candidate archive cells using bundle builder
    for variant in Matrix::all_node_variants() {
        let src = dist.path().join(format!("src_{}", variant.cell));
        fs::create_dir_all(&src).unwrap();

        let machine = match variant.architecture {
            Architecture::Amd64 => 0x3e,
            Architecture::Arm64 => 0xb7,
            Architecture::ArmV7 => 0x28,
            Architecture::Riscv64 => 0xf3,
        };

        fs::write(src.join("rubix-kube"), elf(machine)).unwrap();
        fs::write(src.join("data.txt"), b"payload").unwrap();

        let spec = BundleSpec {
            version: version.to_string(),
            architecture: variant.architecture,
            libc: variant.libc,
            variant: variant.variant,
            inputs: vec![
                BundleInput {
                    source: src.join("rubix-kube"),
                    relative: "bin/rubix-kube".to_string(),
                    executable: true,
                },
                BundleInput {
                    source: src.join("data.txt"),
                    relative: "data.txt".to_string(),
                    executable: false,
                },
            ],
            output_dir: dist.path().to_path_buf(),
        };

        let archive = build_offline_bundle(&spec).unwrap();

        // Ensure the expected canonical name
        let expected_name = variant.archive_filename(BUNDLE_PREFIX, version);
        assert_eq!(
            archive.file_name().unwrap().to_str().unwrap(),
            expected_name
        );

        // 2. Exercise round-trip extraction against bundle manifests
        let install_dest = dist.path().join(format!("dest_{}", variant.cell));
        let arch_str = match variant.architecture {
            Architecture::Amd64 => "x86_64",
            Architecture::Arm64 => "aarch64",
            Architecture::ArmV7 => "arm",
            Architecture::Riscv64 => "riscv64",
        };
        let libc_str = match variant.libc {
            Libc::Glibc => "gnu",
            Libc::Musl => "musl",
        };
        let mut mock = Mock {
            arch: arch_str.to_string(),
            libc: libc_str.to_string(),
        };
        let code = DefaultCommandHandler
            .execute_install(
                InstallOptions {
                    offline_install: Some(archive),
                    path: install_dest.clone(),
                    ..InstallOptions::default()
                },
                &mut mock,
                &mut std::io::sink(),
                &mut std::io::sink(),
            )
            .unwrap();
        assert_eq!(code, 0);

        assert!(install_dest.join("bin/rubix-kube").exists());
        assert!(install_dest.join("data.txt").exists());
    }

    // 3. Use candidate package outputs and provenance records
    let checksums = ChecksumManifest::generate(dist.path()).unwrap();
    assert_eq!(checksums.entries().len(), 16);

    let provenance = generate_source_provenance(GENERATOR, INVENTORY, version, &checksums).unwrap();
    assert_eq!(provenance.artifacts.len(), 16);

    let manifest = generate_manifest(dist.path(), version, "kubesolo", "rubixctl").unwrap();
    assert_eq!(manifest.node_archives.len(), 16);
    assert_eq!(manifest.management_binaries.len(), 0);
}
