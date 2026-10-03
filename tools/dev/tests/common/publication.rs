use rubix_assets::{
    Architecture, AssetId, Matrix, OciImageIndexArtifact, OciPlatformDescriptor,
    ReleasePackageManifest, Variant, catalog,
};
use std::{fs, path::Path};

fn asset_name(id: AssetId) -> &'static str {
    match id {
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
    }
}

// Synthetic descriptor fixture only: these bytes are not runnable release assets.
pub(crate) fn candidate(dir: &Path, version: &str) -> ReleasePackageManifest {
    for variant in Matrix::all_node_variants() {
        fs::write(
            dir.join(variant.archive_filename("rubix-kube", version)),
            b"node",
        )
        .unwrap();
    }
    for target in Matrix::all_management_targets() {
        fs::write(dir.join(target.binary_filename("rubixctl")), b"cli").unwrap();
    }
    let mut manifest =
        rubix_dev::provenance::generate_manifest(dir, version, "rubix-kube", "rubixctl").unwrap();
    for archive in &mut manifest.node_archives {
        let variant = Matrix::from_cell(archive.cell).unwrap();
        archive.bundled_assets = catalog()
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
                AssetId::ImageLocalPath | AssetId::ImageLocalPathHelper => {
                    variant.variant == Variant::Offline
                },
                _ => true,
            })
            .map(|entry| asset_name(entry.id).to_string())
            .collect();
    }
    for id in [
        "image-rubix-kube",
        "image-coredns",
        "image-pause",
        "image-local-path",
        "image-local-path-helper",
        "image-portainer-agent",
        "image-d2k",
    ] {
        let unsupported: Vec<String> = match id {
            "image-portainer-agent" => vec!["linux/riscv64".into()],
            "image-d2k" => vec!["linux/arm/v7".into(), "linux/riscv64".into()],
            _ => vec![],
        };
        let reference = catalog()
            .iter()
            .find(|entry| asset_name(entry.id) == id)
            .map_or("ghcr.io/example/synthetic-node:v1.0.0", |entry| {
                entry.reference
            });
        let platforms = Matrix::all_oci_platforms()
            .iter()
            .filter(|platform| !unsupported.iter().any(|p| p == **platform))
            .map(|platform| OciPlatformDescriptor {
                platform: platform.to_string(),
                architecture: platform.split('/').nth(1).unwrap().to_string(),
                os: "linux".into(),
                media_type: "application/vnd.oci.image.manifest.v1+json".into(),
                digest: format!("sha256:{}", "0".repeat(64)),
                size_bytes: 1,
            })
            .collect();
        manifest.oci_images.push(OciImageIndexArtifact {
            asset_id: id.into(),
            image_reference: reference.into(),
            index_media_type: "application/vnd.oci.image.index.v1+json".into(),
            index_digest: format!("sha256:{}", "0".repeat(64)),
            index_size_bytes: 1,
            platforms,
            unsupported_platforms: unsupported,
        });
    }
    manifest
}
