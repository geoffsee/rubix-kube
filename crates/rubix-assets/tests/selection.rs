#[path = "common/archive.rs"]
mod vectors;

use rubix_assets::{
    AssetId, AssetSelector, Materializer, NodeTarget, OptionalFeature, Scope, SelectedDelivery,
    SelectionError, Variant,
};
use rubix_platform::{Architecture, Libc};
use serde_json::{Value, json};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
use vectors::{entry, gzip, hex};

const OFFLINE_ARM64_JSON: &str = include_str!("fixtures/offline-arm64.json");

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(prefix: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).expect("create test dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn build_offline_archive() -> (rubix_assets::DeclaredInventory, Vec<u8>) {
    let mut manifest_val: Value =
        serde_json::from_str(OFFLINE_ARM64_JSON).expect("valid offline fixture");
    let mut archive_bytes = Vec::new();

    let assets_arr = manifest_val["assets"].as_array_mut().expect("assets array");
    for row in assets_arr.iter_mut() {
        let id_str = row["id"].as_str().expect("asset id");
        let delivery_kind = row["delivery"]["kind"].as_str().unwrap_or("");
        if delivery_kind == "bundled" {
            let id = match id_str {
                "kube-apiserver" => AssetId::KubeApiserver,
                "kube-controller-manager" => AssetId::KubeControllerManager,
                "kubelet" => AssetId::Kubelet,
                "kube-proxy" => AssetId::KubeProxy,
                "kine" => AssetId::Kine,
                "containerd" => AssetId::Containerd,
                "fuse-overlayfs-snapshotter" => AssetId::FuseOverlayfsSnapshotter,
                "containerd-shim-runc-v2" => AssetId::ContainerdShim,
                "crun" => AssetId::Crun,
                "cni-bridge" => AssetId::CniBridge,
                "cni-host-local" => AssetId::CniHostLocal,
                "cni-portmap" => AssetId::CniPortmap,
                "cni-loopback" => AssetId::CniLoopback,
                "image-coredns" => AssetId::ImageCoredns,
                "image-pause" => AssetId::ImagePause,
                "image-local-path" => AssetId::ImageLocalPath,
                "image-local-path-helper" => AssetId::ImageLocalPathHelper,
                "image-portainer-agent" => AssetId::ImagePortainerAgent,
                "image-d2k" => AssetId::ImageD2k,
                other => panic!("unknown asset {other}"),
            };

            let encoding_str = row["delivery"]["encoding"].as_str().unwrap_or("identity");
            let content = match id {
                AssetId::ImageCoredns
                | AssetId::ImagePause
                | AssetId::ImageLocalPath
                | AssetId::ImageLocalPathHelper
                | AssetId::ImagePortainerAgent
                | AssetId::ImageD2k => gzip(&entry("layer.tar", b"test layer")),
                _ => format!("test-bin-{id:?}").into_bytes(),
            };

            let encoded = match encoding_str {
                "identity" | "gzip" => content,
                "zstd" => zstd::encode_all(content.as_slice(), 0).expect("zstd"),
                other => panic!("unknown encoding {other}"),
            };

            row["delivery"]["encoded_bytes"] = json!(encoded.len());
            row["delivery"]["sha256"] = json!(hex(&encoded));

            let path = row["delivery"]["path"].as_str().expect("path");
            archive_bytes.extend(entry(path, &encoded));
        }
    }
    archive_bytes.extend([0u8; 1024]);

    let manifest_bytes = serde_json::to_vec(&manifest_val).expect("serialized json");
    let inventory =
        rubix_assets::Manifest::decode(&manifest_bytes, rubix_assets::Limits::default())
            .expect("decode")
            .validate_inventory(
                rubix_assets::InventoryRequest {
                    target: NodeTarget {
                        architecture: Architecture::Arm64,
                        libc: Libc::Glibc,
                    },
                    variant: Variant::Offline,
                    scope: Scope::SupervisedBundle,
                },
                rubix_assets::Limits::default(),
            )
            .expect("validate");

    (inventory, archive_bytes)
}

#[test]
fn external_dependency_builds_compile_without_embedded_payloads() {
    let target = NodeTarget {
        architecture: Architecture::Arm64,
        libc: Libc::Glibc,
    };
    let selector = AssetSelector::new(target, Variant::Online, Scope::LegacyExternalDeps)
        .with_local_storage(true)
        .with_portainer_agent(true, None)
        .with_d2k(true);

    assert_eq!(selector.scope(), Scope::LegacyExternalDeps);

    // Executables are host-supplied
    for id in [
        AssetId::KubeApiserver,
        AssetId::KubeControllerManager,
        AssetId::Kubelet,
        AssetId::KubeProxy,
        AssetId::Kine,
        AssetId::Containerd,
        AssetId::ContainerdShim,
        AssetId::Crun,
        AssetId::CniBridge,
        AssetId::CniHostLocal,
        AssetId::CniPortmap,
        AssetId::CniLoopback,
        AssetId::FuseOverlayfsSnapshotter,
    ] {
        assert_eq!(
            selector.select_delivery(id),
            SelectedDelivery::HostSupplied,
            "{id:?} must be host-supplied in LegacyExternalDeps"
        );
        assert!(!selector.is_bundled(id));
    }

    // Zero embedded/bundled payloads
    assert_eq!(selector.selected_bundled_assets().count(), 0);

    // Enabled images are pull-only in external builds
    for id in [
        AssetId::ImageCoredns,
        AssetId::ImagePause,
        AssetId::ImageLocalPath,
        AssetId::ImageLocalPathHelper,
        AssetId::ImagePortainerAgent,
        AssetId::ImageD2k,
    ] {
        assert!(
            selector.select_delivery(id).is_registry_pull(),
            "{id:?} must be registry pull in LegacyExternalDeps"
        );
    }
}

#[test]
fn local_storage_disabled_selection_requests_no_provisioner_image() {
    let target = NodeTarget {
        architecture: Architecture::Amd64,
        libc: Libc::Glibc,
    };

    // When local storage is disabled:
    let disabled_selector = AssetSelector::new(target, Variant::Offline, Scope::SupervisedBundle)
        .with_local_storage(false);

    assert_eq!(
        disabled_selector.select_delivery(AssetId::ImageLocalPath),
        SelectedDelivery::Disabled
    );
    assert_eq!(
        disabled_selector.select_delivery(AssetId::ImageLocalPathHelper),
        SelectedDelivery::Disabled
    );
    assert!(!disabled_selector.is_bundled(AssetId::ImageLocalPath));
    assert!(!disabled_selector.is_bundled(AssetId::ImageLocalPathHelper));

    // When local storage is enabled in Offline variant:
    let enabled_offline = AssetSelector::new(target, Variant::Offline, Scope::SupervisedBundle)
        .with_local_storage(true);

    assert_eq!(
        enabled_offline.select_delivery(AssetId::ImageLocalPath),
        SelectedDelivery::Bundled
    );
    assert_eq!(
        enabled_offline.select_delivery(AssetId::ImageLocalPathHelper),
        SelectedDelivery::Bundled
    );
    assert!(enabled_offline.is_bundled(AssetId::ImageLocalPath));
    assert!(enabled_offline.is_bundled(AssetId::ImageLocalPathHelper));

    // When local storage is enabled in Online variant:
    let enabled_online = AssetSelector::new(target, Variant::Online, Scope::SupervisedBundle)
        .with_local_storage(true);

    assert!(
        enabled_online
            .select_delivery(AssetId::ImageLocalPath)
            .is_registry_pull()
    );
    assert!(
        enabled_online
            .select_delivery(AssetId::ImageLocalPathHelper)
            .is_registry_pull()
    );
    assert!(!enabled_online.is_bundled(AssetId::ImageLocalPath));
    assert!(!enabled_online.is_bundled(AssetId::ImageLocalPathHelper));
}

#[test]
fn egress_denied_offline_fixtures_provide_supported_bundled_images() {
    let target = NodeTarget {
        architecture: Architecture::Arm64,
        libc: Libc::Glibc,
    };

    // Valid offline build with supported features and egress denied succeeds
    let offline_selector = AssetSelector::new(target, Variant::Offline, Scope::SupervisedBundle)
        .with_local_storage(true)
        .with_portainer_agent(true, None)
        .with_d2k(true)
        .with_egress_denied(true);

    assert!(
        offline_selector.validate().is_ok(),
        "offline variant bundles all supported images, satisfying egress denial"
    );

    // Online variant with egress denied fails because optional images require registry pull
    let online_selector = AssetSelector::new(target, Variant::Online, Scope::SupervisedBundle)
        .with_local_storage(true)
        .with_egress_denied(true);

    let err = online_selector.validate().unwrap_err();
    assert!(
        matches!(err, SelectionError::EgressDeniedRegistryRequired { .. }),
        "online variant requires registry pull, which fails with egress denied"
    );
}

#[test]
fn custom_portainer_images_remain_explicit_registry_pulls() {
    let target = NodeTarget {
        architecture: Architecture::Arm64,
        libc: Libc::Glibc,
    };

    let custom_image = "corp-registry.internal/portainer/agent:2.20.0".to_string();

    // Even in offline variant, custom portainer image is a registry pull, not bundled
    let selector = AssetSelector::new(target, Variant::Offline, Scope::SupervisedBundle)
        .with_portainer_agent(true, Some(custom_image.clone()));

    let delivery = selector.select_delivery(AssetId::ImagePortainerAgent);
    assert_eq!(
        delivery,
        SelectedDelivery::RegistryPull {
            reference: custom_image.clone(),
            custom: true,
        }
    );
    assert!(
        !selector.is_bundled(AssetId::ImagePortainerAgent),
        "custom portainer images must not be bundled"
    );

    // When egress is denied, a custom portainer image cannot be satisfied
    let egress_denied_selector = selector.with_egress_denied(true);
    let err = egress_denied_selector.validate().unwrap_err();
    match err {
        SelectionError::EgressDeniedRegistryRequired {
            asset,
            reference,
            custom,
        } => {
            assert_eq!(asset, AssetId::ImagePortainerAgent);
            assert_eq!(reference, custom_image);
            assert!(custom);
        },
        other => panic!(
            "expected EgressDeniedRegistryRequired for custom portainer image, got {other:?}"
        ),
    }
}

#[test]
fn unsupported_target_features_are_rejected_or_unsupported() {
    let riscv = NodeTarget {
        architecture: Architecture::Riscv64,
        libc: Libc::Glibc,
    };

    // Portainer agent is unsupported on RISC-V
    let selector_riscv_portainer =
        AssetSelector::new(riscv, Variant::Offline, Scope::SupervisedBundle)
            .with_portainer_agent(true, None);

    assert_eq!(
        selector_riscv_portainer.select_delivery(AssetId::ImagePortainerAgent),
        SelectedDelivery::UnsupportedTarget
    );
    let err = selector_riscv_portainer.validate().unwrap_err();
    assert_eq!(
        err,
        SelectionError::UnsupportedTargetFeature {
            feature: OptionalFeature::PortainerAgent,
            architecture: Architecture::Riscv64,
        }
    );

    // D2K is unsupported on ARMv7
    let armv7 = NodeTarget {
        architecture: Architecture::ArmV7,
        libc: Libc::Glibc,
    };
    let selector_armv7_d2k =
        AssetSelector::new(armv7, Variant::Offline, Scope::SupervisedBundle).with_d2k(true);

    assert_eq!(
        selector_armv7_d2k.select_delivery(AssetId::ImageD2k),
        SelectedDelivery::UnsupportedTarget
    );
    let err = selector_armv7_d2k.validate().unwrap_err();
    assert_eq!(
        err,
        SelectionError::UnsupportedTargetFeature {
            feature: OptionalFeature::D2k,
            architecture: Architecture::ArmV7,
        }
    );
}

#[test]
fn materializer_honors_asset_selection_filter() {
    let (inventory, archive_data) = build_offline_archive();
    let target = NodeTarget {
        architecture: Architecture::Arm64,
        libc: Libc::Glibc,
    };

    // Materialize with local storage disabled
    let test_dir_no_storage = TestDir::new("rubix-test-no-storage");
    let selector_no_storage = AssetSelector::new(target, Variant::Offline, Scope::SupervisedBundle)
        .with_local_storage(false);

    let mat_no_storage =
        Materializer::new(inventory, test_dir_no_storage.path()).with_selector(selector_no_storage);

    let outcome_no_storage = mat_no_storage
        .materialize_from_archive(Cursor::new(&archive_data))
        .expect("materialize without storage succeeds");

    // Provisioner and helper image must NOT exist
    let local_path_dest = mat_no_storage
        .layout()
        .resolve_destination(test_dir_no_storage.path(), AssetId::ImageLocalPath)
        .unwrap();
    let helper_dest = mat_no_storage
        .layout()
        .resolve_destination(test_dir_no_storage.path(), AssetId::ImageLocalPathHelper)
        .unwrap();

    assert!(
        !local_path_dest.exists(),
        "local-path image must not be extracted when local storage is disabled"
    );
    assert!(
        !helper_dest.exists(),
        "helper image must not be extracted when local storage is disabled"
    );
    assert!(outcome_no_storage.get(AssetId::ImageLocalPath).is_none());
    assert!(
        outcome_no_storage
            .get(AssetId::ImageLocalPathHelper)
            .is_none()
    );

    // Executables must still exist
    let apiserver_dest = mat_no_storage
        .layout()
        .resolve_destination(test_dir_no_storage.path(), AssetId::KubeApiserver)
        .unwrap();
    assert!(apiserver_dest.exists(), "kube-apiserver must exist");

    // Now repeat with local storage enabled
    let (inventory2, _) = build_offline_archive();
    let test_dir_with_storage = TestDir::new("rubix-test-with-storage");
    let selector_with_storage =
        AssetSelector::new(target, Variant::Offline, Scope::SupervisedBundle)
            .with_local_storage(true);

    let mat_with_storage = Materializer::new(inventory2, test_dir_with_storage.path())
        .with_selector(selector_with_storage);

    let outcome_with_storage = mat_with_storage
        .materialize_from_archive(Cursor::new(&archive_data))
        .expect("materialize with storage succeeds");

    let local_path_dest2 = mat_with_storage
        .layout()
        .resolve_destination(test_dir_with_storage.path(), AssetId::ImageLocalPath)
        .unwrap();
    let helper_dest2 = mat_with_storage
        .layout()
        .resolve_destination(test_dir_with_storage.path(), AssetId::ImageLocalPathHelper)
        .unwrap();

    assert!(
        local_path_dest2.exists(),
        "local-path image must be extracted when local storage is enabled"
    );
    assert!(
        helper_dest2.exists(),
        "helper image must be extracted when local storage is enabled"
    );
    assert!(outcome_with_storage.get(AssetId::ImageLocalPath).is_some());
    assert!(
        outcome_with_storage
            .get(AssetId::ImageLocalPathHelper)
            .is_some()
    );
}
