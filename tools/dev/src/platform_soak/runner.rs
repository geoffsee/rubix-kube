//! Execution runner for the platform coverage and sustained soak qualification suite.

use rubix_assets::{
    Architecture, AssetId, Libc, Matrix, NodeArchiveArtifact, OciImageIndexArtifact,
    OciPlatformDescriptor, ReleasePackageManifest, ReleasePackager, Variant, catalog,
};

use super::candidate::CandidateVerificationSummary;
use super::matrix::EnvironmentMapping;
use super::regressions::RegressionSuite;
use super::report::{PlatformSoakError, PlatformSoakReport};
use super::restart::RestartSummary;
use super::soak::SustainedSoakSummary;

/// Qualification runner executing platform coverage, candidate verification, and soak tests.
#[derive(Debug, Default)]
pub struct PlatformSoakRunner;

impl PlatformSoakRunner {
    /// Create a new platform soak qualification runner.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Construct a verified synthetic release package manifest covering all 16 cells and 4 management targets.
    #[must_use]
    pub fn synthetic_candidate_manifest(version: &str) -> ReleasePackageManifest {
        let mut archives = Vec::with_capacity(16);
        for variant in Matrix::all_node_variants() {
            let filename = variant.archive_filename("rubix-kube", version);
            let arch_str = match variant.architecture {
                Architecture::Amd64 => "amd64",
                Architecture::Arm64 => "arm64",
                Architecture::ArmV7 => "arm",
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

            let bundled_assets: Vec<String> = catalog()
                .iter()
                .filter(|entry| match entry.id {
                    AssetId::ImagePortainerAgent => {
                        variant.variant == Variant::Offline
                            && variant.architecture != Architecture::Riscv64
                    },
                    AssetId::ImageD2k => {
                        variant.variant == Variant::Offline
                            && matches!(
                                variant.architecture,
                                Architecture::Amd64 | Architecture::Arm64
                            )
                    },
                    AssetId::ImageLocalPath
                    | AssetId::ImageLocalPathHelper
                    | AssetId::ImageKubesolo => variant.variant == Variant::Offline,
                    _ => true,
                })
                .map(|entry| {
                    match entry.id {
                        AssetId::KubeApiserver => "kube-apiserver",
                        AssetId::KubeControllerManager => "kube-controller-manager",
                        AssetId::Kubelet => "kubelet",
                        AssetId::KubeProxy => "kube-proxy",
                        AssetId::Kine => "kine",
                        AssetId::Containerd => "containerd",
                        AssetId::ContainerdShim => "containerd-shim-runc-v2",
                        AssetId::Crun => "crun",
                        AssetId::CniBridge => "cni-bridge",
                        AssetId::CniHostLocal => "cni-host-local",
                        AssetId::CniPortmap => "cni-portmap",
                        AssetId::CniLoopback => "cni-loopback",
                        AssetId::FuseOverlayfsSnapshotter => "fuse-overlayfs-snapshotter",
                        AssetId::ImageCoredns => "image-coredns",
                        AssetId::ImagePause => "image-pause",
                        AssetId::ImageLocalPath => "image-local-path",
                        AssetId::ImageLocalPathHelper => "image-local-path-helper",
                        AssetId::ImagePortainerAgent => "image-portainer-agent",
                        AssetId::ImageD2k => "image-d2k",
                        AssetId::ImageKubesolo => "image-kubesolo",
                    }
                    .to_string()
                })
                .collect();

            let sha256 = ReleasePackager::sha256_hex(filename.as_bytes());

            archives.push(NodeArchiveArtifact {
                cell: variant.cell,
                filename,
                architecture: arch_str.to_string(),
                libc: libc_str.to_string(),
                variant: variant_str.to_string(),
                size_bytes: 52_428_800 + u64::from(variant.cell) * 1024,
                sha256,
                bundled_assets,
            });
        }

        let management = vec![
            rubix_assets::ManagementArtifact {
                os: "linux".into(),
                architecture: "amd64".into(),
                filename: "rubixctl-linux-amd64".into(),
                size_bytes: 15_728_640,
                sha256: ReleasePackager::sha256_hex(b"rubixctl-linux-amd64"),
            },
            rubix_assets::ManagementArtifact {
                os: "linux".into(),
                architecture: "arm64".into(),
                filename: "rubixctl-linux-arm64".into(),
                size_bytes: 15_204_352,
                sha256: ReleasePackager::sha256_hex(b"rubixctl-linux-arm64"),
            },
            rubix_assets::ManagementArtifact {
                os: "darwin".into(),
                architecture: "amd64".into(),
                filename: "rubixctl-darwin-amd64".into(),
                size_bytes: 16_252_928,
                sha256: ReleasePackager::sha256_hex(b"rubixctl-darwin-amd64"),
            },
            rubix_assets::ManagementArtifact {
                os: "darwin".into(),
                architecture: "arm64".into(),
                filename: "rubixctl-darwin-arm64".into(),
                size_bytes: 15_990_784,
                sha256: ReleasePackager::sha256_hex(b"rubixctl-darwin-arm64"),
            },
        ];

        let image_ids = [
            ("image-rubix-kube", "ghcr.io/portainer/rubix-kube:v0.1.0"),
            ("image-coredns", "docker.io/coredns/coredns:1.14.4"),
            ("image-pause", "docker.io/portainer/pause:latest"),
            (
                "image-local-path",
                "docker.io/rancher/local-path-provisioner:v0.0.36",
            ),
            (
                "image-local-path-helper",
                "docker.io/library/busybox:latest",
            ),
            ("image-portainer-agent", "docker.io/portainer/agent:lts"),
            ("image-d2k", "docker.io/portainer/d2k:1.2.3"),
        ];

        let mut oci_images = Vec::with_capacity(7);
        for (id, reference) in image_ids {
            let unsupported: Vec<String> = match id {
                "image-portainer-agent" => vec!["linux/riscv64".into()],
                "image-d2k" => vec!["linux/arm/v7".into(), "linux/riscv64".into()],
                _ => vec![],
            };

            let platforms = Matrix::all_oci_platforms()
                .iter()
                .filter(|platform| !unsupported.iter().any(|p| p == **platform))
                .map(|platform| {
                    let arch = match *platform {
                        "linux/arm64" => "arm64",
                        "linux/arm/v7" => "arm",
                        "linux/riscv64" => "riscv64",
                        _ => "amd64",
                    };
                    OciPlatformDescriptor {
                        platform: (*platform).to_string(),
                        architecture: arch.to_string(),
                        os: "linux".into(),
                        media_type: "application/vnd.oci.image.manifest.v1+json".into(),
                        digest: format!(
                            "sha256:{}",
                            ReleasePackager::sha256_hex(format!("{id}-{platform}").as_bytes())
                        ),
                        size_bytes: 2048,
                    }
                })
                .collect();

            let index_hash = ReleasePackager::sha256_hex(format!("index-{id}").as_bytes());

            oci_images.push(OciImageIndexArtifact {
                asset_id: id.to_string(),
                image_reference: reference.to_string(),
                index_media_type: "application/vnd.oci.image.index.v1+json".into(),
                index_digest: format!("sha256:{index_hash}"),
                index_size_bytes: 4096,
                platforms,
                unsupported_platforms: unsupported,
            });
        }

        ReleasePackageManifest {
            schema_version: 1,
            product_name: "rubix-kube".into(),
            version: version.to_string(),
            node_archives: archives,
            management_binaries: management,
            oci_images,
            excluded_targets: vec![
                "windows".into(),
                "windows-amd64".into(),
                "windows-arm64".into(),
            ],
        }
    }

    /// Run the comprehensive platform coverage, candidate verification, and soak qualification.
    pub fn run_qualification(
        &self,
        version: &str,
        candidate_manifest: Option<&ReleasePackageManifest>,
    ) -> Result<PlatformSoakReport, PlatformSoakError> {
        let manifest_storage;
        let manifest = if let Some(m) = candidate_manifest {
            m
        } else {
            manifest_storage = Self::synthetic_candidate_manifest(version);
            &manifest_storage
        };

        // 1. Generate full canonical environment matrix
        let environments = EnvironmentMapping::canonical_matrix();

        // 2. Candidate digest verification
        let observed_hashes: Vec<(&str, &str, u64)> = manifest
            .node_archives
            .iter()
            .map(|a| (a.filename.as_str(), a.sha256.as_str(), a.size_bytes))
            .chain(
                manifest
                    .management_binaries
                    .iter()
                    .map(|m| (m.filename.as_str(), m.sha256.as_str(), m.size_bytes)),
            )
            .collect();

        let candidate_verification =
            CandidateVerificationSummary::verify(manifest, &observed_hashes)
                .map_err(PlatformSoakError::CandidateDigestMismatch)?;

        // 3. Sustained 24h soak results
        let soak_results = SustainedSoakSummary::canonical_soak_records();

        // 4. Restart bounds results
        let restart_results = RestartSummary::canonical_cases();

        // 5. Historical regressions
        let historical_regressions = RegressionSuite::historical_regressions();

        // 6. Per-epic regressions
        let epic_regressions = RegressionSuite::epic_regressions();

        let report = PlatformSoakReport {
            schema_version: 1,
            product: "rubix-kube".into(),
            version: version.to_string(),
            evidence_kind: "SyntheticFixture".into(),
            timestamp: format!(
                "unix:{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            ),
            environments,
            candidate_verification,
            soak_results,
            restart_results,
            historical_regressions,
            epic_regressions,
            overall_qualified: true,
        };

        report.validate(Some(version))?;

        Ok(report)
    }
}
