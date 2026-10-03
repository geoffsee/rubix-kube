use rubix_assets::{
    AssetId, DeclaredInventory, Delivery, FeatureSupport, InventoryError, InventoryRequest, Limits,
    Manifest, OptionalFeature, Scope, Variant, feature_support,
};
use rubix_platform::{Architecture, Libc, NodeTarget};
use serde_json::{Value, json};
const SOURCE: &str = include_str!("fixtures/online-amd64.json");
const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
fn request(architecture: Architecture, libc: Libc, variant: Variant) -> InventoryRequest {
    InventoryRequest {
        target: NodeTarget { architecture, libc },
        variant,
        scope: Scope::SupervisedBundle,
    }
}
fn decode(
    value: &Value,
    request: InventoryRequest,
    limits: Limits,
) -> Result<DeclaredInventory, InventoryError> {
    Manifest::decode(&serde_json::to_vec(value).expect("test JSON"), limits)?
        .validate_inventory(request, limits)
}
fn standard() -> InventoryRequest {
    request(Architecture::Amd64, Libc::Glibc, Variant::Online)
}
fn fixture() -> Value {
    serde_json::from_str(SOURCE).expect("independent fixture")
}
fn configure_images(records: &mut [Value], variant: Variant, unavailable: usize) {
    for row in records {
        let id = row["id"].as_str().expect("id").to_owned();
        let disabled = id == "image-d2k" && unavailable > 0
            || id == "image-portainer-agent" && unavailable == 2;
        if disabled {
            row["delivery"] = json!({"kind":"unavailable"});
        } else if variant == Variant::Offline && id.starts_with("image-") {
            row["delivery"] = json!({"kind":"bundled","path":format!("fixtures/{id}"),"encoding":"gzip","encoded_bytes":3,"sha256":HASH});
        }
    }
}
fn assert_delivery(inventory: &DeclaredInventory, id: AssetId, bundled: bool) {
    let delivery = inventory
        .assets()
        .find(|(asset, _)| *asset == id)
        .expect("catalog role")
        .1;
    assert_eq!(
        matches!(delivery, Delivery::Bundled { .. }),
        bundled,
        "{id:?}"
    );
}
fn assert_optional_support(inventory: &DeclaredInventory, architecture: Architecture) {
    assert_eq!(
        inventory.optional_feature_support().collect::<Vec<_>>(),
        [
            (
                OptionalFeature::LocalPathStorage,
                FeatureSupport::SupportedTarget,
            ),
            (
                OptionalFeature::PortainerAgent,
                if architecture == Architecture::Riscv64 {
                    FeatureSupport::UnsupportedTarget
                } else {
                    FeatureSupport::SupportedTarget
                },
            ),
            (
                OptionalFeature::D2k,
                if matches!(architecture, Architecture::Amd64 | Architecture::Arm64) {
                    FeatureSupport::SupportedTarget
                } else {
                    FeatureSupport::UnsupportedTarget
                },
            ),
        ]
    );
}
fn assert_image_contract(
    inventory: &DeclaredInventory,
    architecture: Architecture,
    variant: Variant,
) {
    assert_delivery(inventory, AssetId::ImageCoredns, true);
    assert_delivery(inventory, AssetId::ImagePause, true);
    if variant == Variant::Offline {
        assert_delivery(inventory, AssetId::ImageLocalPath, true);
        assert_delivery(inventory, AssetId::ImageLocalPathHelper, true);
        assert_delivery(
            inventory,
            AssetId::ImagePortainerAgent,
            architecture != Architecture::Riscv64,
        );
        assert_delivery(
            inventory,
            AssetId::ImageD2k,
            matches!(architecture, Architecture::Amd64 | Architecture::Arm64),
        );
        assert_delivery(inventory, AssetId::ImageKubesolo, true);
    }
}
#[test]
fn all_sixteen_target_variant_cells_have_exact_declared_roles() {
    let expected = [
        "kube-apiserver",
        "kube-controller-manager",
        "kubelet",
        "kube-proxy",
        "kine",
        "containerd",
        "fuse-overlayfs-snapshotter",
        "containerd-shim-runc-v2",
        "crun",
        "cni-bridge",
        "cni-host-local",
        "cni-portmap",
        "cni-loopback",
        "image-coredns",
        "image-pause",
        "image-local-path",
        "image-local-path-helper",
        "image-portainer-agent",
        "image-d2k",
        "image-kubesolo",
    ];
    let mut cells = 0;
    for (architecture, name, unavailable) in [
        (Architecture::Amd64, "amd64", 0),
        (Architecture::Arm64, "arm64", 0),
        (Architecture::ArmV7, "armv7", 1),
        (Architecture::Riscv64, "riscv64", 2),
    ] {
        for (libc, libc_name) in [(Libc::Glibc, "glibc"), (Libc::Musl, "musl")] {
            for variant in [Variant::Online, Variant::Offline] {
                let mut value = fixture();
                value["target"]["architecture"] = json!(name);
                value["target"]["libc"] = json!(libc_name);
                value["variant"] = json!(if variant == Variant::Online {
                    "online"
                } else {
                    "offline"
                });
                let records = value["assets"].as_array_mut().expect("array");
                assert_eq!(
                    records
                        .iter()
                        .map(|r| r["id"].as_str().expect("id"))
                        .collect::<Vec<_>>(),
                    expected
                );
                configure_images(records, variant, unavailable);
                let inventory = decode(
                    &value,
                    request(architecture, libc, variant),
                    Limits::default(),
                )
                .expect("complete cell");
                assert_eq!(inventory.assets().len(), 20);
                assert_eq!(
                    inventory.bundled_assets().len(),
                    if variant == Variant::Online {
                        15
                    } else {
                        20 - unavailable
                    }
                );
                assert_eq!(
                    inventory
                        .assets()
                        .filter(|(_, d)| matches!(d, Delivery::Unavailable {}))
                        .count(),
                    unavailable
                );
                assert_optional_support(&inventory, architecture);
                assert_image_contract(&inventory, architecture, variant);
                cells += 1;
            }
        }
    }
    assert_eq!(cells, 16);
}
#[test]
fn feature_support_matches_expected_architecture_policy() {
    for arch in [
        Architecture::Amd64,
        Architecture::Arm64,
        Architecture::ArmV7,
        Architecture::Riscv64,
    ] {
        assert_eq!(
            feature_support(arch, OptionalFeature::LocalPathStorage),
            FeatureSupport::SupportedTarget
        );
    }
    assert_eq!(
        feature_support(Architecture::Amd64, OptionalFeature::PortainerAgent),
        FeatureSupport::SupportedTarget
    );
    assert_eq!(
        feature_support(Architecture::Arm64, OptionalFeature::PortainerAgent),
        FeatureSupport::SupportedTarget
    );
    assert_eq!(
        feature_support(Architecture::ArmV7, OptionalFeature::PortainerAgent),
        FeatureSupport::SupportedTarget
    );
    assert_eq!(
        feature_support(Architecture::Riscv64, OptionalFeature::PortainerAgent),
        FeatureSupport::UnsupportedTarget
    );

    assert_eq!(
        feature_support(Architecture::Amd64, OptionalFeature::D2k),
        FeatureSupport::SupportedTarget
    );
    assert_eq!(
        feature_support(Architecture::Arm64, OptionalFeature::D2k),
        FeatureSupport::SupportedTarget
    );
    assert_eq!(
        feature_support(Architecture::ArmV7, OptionalFeature::D2k),
        FeatureSupport::UnsupportedTarget
    );
    assert_eq!(
        feature_support(Architecture::Riscv64, OptionalFeature::D2k),
        FeatureSupport::UnsupportedTarget
    );
}
#[test]
fn structural_linux_arm64_glibc_manifests_validate_both_variants() {
    let online_bytes = include_bytes!("fixtures/online-arm64.json");
    let online = Manifest::decode(online_bytes, Limits::default())
        .expect("valid online manifest")
        .validate_inventory(
            request(Architecture::Arm64, Libc::Glibc, Variant::Online),
            Limits::default(),
        )
        .expect("valid online arm64 glibc inventory");
    assert_eq!(online.assets().len(), 20);
    assert_eq!(online.bundled_assets().len(), 15);
    assert_image_contract(&online, Architecture::Arm64, Variant::Online);
    assert_optional_support(&online, Architecture::Arm64);

    let offline_bytes = include_bytes!("fixtures/offline-arm64.json");
    let offline = Manifest::decode(offline_bytes, Limits::default())
        .expect("valid offline manifest")
        .validate_inventory(
            request(Architecture::Arm64, Libc::Glibc, Variant::Offline),
            Limits::default(),
        )
        .expect("valid offline arm64 glibc inventory");
    assert_eq!(offline.assets().len(), 20);
    assert_eq!(offline.bundled_assets().len(), 20);
    assert_image_contract(&offline, Architecture::Arm64, Variant::Offline);
    assert_optional_support(&offline, Architecture::Arm64);
    assert_structural_placeholders(online_bytes, &online);
    assert_structural_placeholders(offline_bytes, &offline);
}

fn assert_structural_placeholders(raw: &[u8], inventory: &DeclaredInventory) {
    let text = std::str::from_utf8(raw).expect("fixture utf-8");
    for reused in [
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "35b5ba6e08cdd034b8d1920f9c7203e5d66f258e1915f164a01de0af14f34710",
        "114dde0fefebbca13165d0da9c500a66190e497a82a53dcaabc3172d630be1e9",
        "ba5b1d81f9993f384b8f032914e5491306f056088821c2dba1c366bcb0893169",
        "33b37ab0b0901175c2699d81c10b0c887f0dff944de74763cca00c155904d6fb",
        "546ffb720afb37c1c537c5113c366b9cd3f7a714a4f80e5c3ca11843eedc6124",
    ] {
        assert!(!text.contains(reused), "reused digest {reused}");
    }
    for (id, delivery) in inventory.assets() {
        let Delivery::Bundled {
            sha256,
            encoded_bytes,
            ..
        } = delivery
        else {
            continue;
        };
        if matches!(
            id,
            AssetId::KubeApiserver
                | AssetId::KubeControllerManager
                | AssetId::Kubelet
                | AssetId::KubeProxy
                | AssetId::Kine
        ) {
            continue;
        }
        assert_eq!(*encoded_bytes, 64, "{id:?}");
        assert_eq!(sha256, structural_digest(id), "{id:?}");
    }
    let apiserver = inventory
        .assets()
        .find(|(id, _)| *id == AssetId::KubeApiserver);
    let Some((_, Delivery::Bundled { sha256, .. })) = apiserver else {
        panic!("kube-apiserver pin missing");
    };
    assert_eq!(
        sha256,
        "4e5fe160e7b90e84faab827e71a101f0472a920385abfb7f6bba36ee783529e1"
    );
}

fn structural_digest(id: AssetId) -> &'static str {
    match id {
        AssetId::ContainerdShim => {
            "78bb7ccefd41ab2d1c83ef2f0f5178bd57a4cdb359cb5298961d3d2fef6bd591"
        },
        AssetId::Crun => "7334035c70464154385408b773496920602ba4012b6a690eb1f635b02dd62c30",
        AssetId::CniBridge => "adde2235bc72cdd111fe7cef9de85d196eb24d6a24c975b8e43d65491eaa811c",
        AssetId::CniHostLocal => "df28e12d46743e1810d54e39fa0288e0285e0d8dae67c755fad6e57c6c5f62c2",
        AssetId::CniPortmap => "2d7720ad0daa2fc1238ee21e305af372b0d0d87c3e4a2754d588fac3172cdeef",
        AssetId::CniLoopback => "d10ed367231b7a5637885ae36ccb1b3162fed4a48fe48ee894339caa1c5d024a",
        AssetId::Containerd => "2397d068a8d1552a4c3f9147ba9c94e086bfd66fb2acb5bbaf47197105127d5f",
        AssetId::FuseOverlayfsSnapshotter => {
            "3935c8f4bf206670ccecc8126fa81f54dc0248576a1be29153c3b1ea6ed8e6f3"
        },
        AssetId::ImageCoredns => "dd0e4c5ec09bd5e73f21e4f297acf5355f9b2d924d976090e22d6098089eb1a3",
        AssetId::ImagePause => "c43544793a1810ab5e7149f17d37833c473d85c8279739ee9d42b0274b5c727c",
        AssetId::ImageLocalPath => {
            "55ec058998391634e19a91a8e7b7393c3c3ee7a4645df0b5221f64ede22d62fe"
        },
        AssetId::ImageLocalPathHelper => {
            "a8e722d3f5a1b9e7aa7db1e77cf03efff286664fb1fe811ef83f52e3def79405"
        },
        AssetId::ImagePortainerAgent => {
            "3d92978a213132043410654949cd06ade2b05cbbefe61ac57b6c667f94ee1bda"
        },
        AssetId::ImageD2k | AssetId::ImageKubesolo => {
            "a9b9a9f9a512e3fe70571c58d4d76862deb09566eca282ea6b1afa2fed287743"
        },
        AssetId::KubeApiserver
        | AssetId::KubeControllerManager
        | AssetId::Kubelet
        | AssetId::KubeProxy
        | AssetId::Kine => unreachable!("upstream pins stay outside the placeholder contract"),
    }
}
#[test]
fn absent_or_truncated_manifest_bytes_fail_before_inventory_validation() {
    for bytes in [&b""[..], &b"{\"schema_version\":1"[..]] {
        assert!(matches!(
            Manifest::decode(bytes, Limits::default()),
            Err(InventoryError::InvalidJson { .. })
        ));
    }
    let complete = SOURCE.trim_end().as_bytes();
    assert!(matches!(
        Manifest::decode(&complete[..complete.len() - 1], Limits::default()),
        Err(InventoryError::InvalidJson { .. })
    ));
}
#[test]
fn unknown_duplicate_fields_ids_and_versions_fail() {
    for raw in [
        SOURCE.replacen(
            "\"schema_version\": 1",
            "\"schema_version\": 1, \"schema_version\": 1",
            1,
        ),
        SOURCE.replacen(
            "\"encoded_bytes\": 3",
            "\"encoded_bytes\": 3, \"encoded_bytes\": 3",
            1,
        ),
        SOURCE.replacen(
            "\"os\": \"linux\"",
            "\"os\": \"linux\", \"os\": \"linux\"",
            1,
        ),
        SOURCE.replacen(
            "\"kind\": \"registry-required\"",
            "\"kind\": \"registry-required\", \"extra\": true",
            1,
        ),
        SOURCE.replacen(
            "\"kind\": \"bundled\"",
            "\"kind\": \"bundled\", \"kind\": \"bundled\"",
            1,
        ),
        SOURCE.replacen(
            "\"schema_version\": 1",
            "\"extra\": 1, \"schema_version\": 1",
            1,
        ),
        SOURCE.replacen("\"schema_version\": 1", "\"schema_version\": true", 1),
        SOURCE.replacen("\"kube-apiserver\"", "\"unknown-component\"", 1),
    ] {
        assert!(
            Manifest::decode(raw.as_bytes(), Limits::default()).is_err(),
            "accepted malformed schema"
        );
    }
    let mut v = fixture();
    v["schema_version"] = json!(2);
    assert_eq!(
        decode(&v, standard(), Limits::default()).unwrap_err(),
        InventoryError::SchemaVersion
    );
    let mut v = fixture();
    let first = v["assets"][0].clone();
    v["assets"].as_array_mut().unwrap().push(first);
    assert_eq!(
        decode(&v, standard(), Limits::default()).unwrap_err(),
        InventoryError::DuplicateAsset(AssetId::KubeApiserver)
    );
}
#[test]
fn missing_mandatory_roles_wrong_target_and_delivery_fail() {
    for index in [0, 13, 14, 16] {
        let mut v = fixture();
        v["assets"].as_array_mut().unwrap().remove(index);
        assert!(matches!(
            decode(&v, standard(), Limits::default()),
            Err(InventoryError::MissingAsset(_))
        ));
    }
    let v = fixture();
    assert_eq!(
        decode(
            &v,
            request(Architecture::Arm64, Libc::Glibc, Variant::Online),
            Limits::default()
        )
        .unwrap_err(),
        InventoryError::TargetMismatch
    );
    let mut v = fixture();
    v["assets"][13]["delivery"] = json!({"kind":"registry-required"});
    assert!(matches!(
        decode(&v, standard(), Limits::default()),
        Err(InventoryError::WrongDelivery(_))
    ));
    let mut v = fixture();
    v["assets"][0]["delivery"]["encoding"] = json!("zstd");
    assert!(matches!(
        decode(&v, standard(), Limits::default()),
        Err(InventoryError::WrongEncoding(_))
    ));
}
#[test]
fn unsafe_conflicting_paths_and_invalid_digests_are_rejected() {
    for path in [
        "", "/abs", "../up", "a/../b", "a//b", "./a", "a/", "a\\b", "C:x", "é", "a\0b",
    ] {
        let mut v = fixture();
        v["assets"][0]["delivery"]["path"] = json!(path);
        assert!(
            matches!(
                decode(&v, standard(), Limits::default()),
                Err(InventoryError::UnsafePath(_))
            ),
            "{path}"
        );
    }
    for path in [
        "fixtures/kube-apiserver",
        "fixtures",
        "fixtures/kube-apiserver/nested",
    ] {
        let mut v = fixture();
        v["assets"][1]["delivery"]["path"] = json!(path);
        assert!(matches!(
            decode(&v, standard(), Limits::default()),
            Err(InventoryError::ConflictingPath(_))
        ));
    }
    for hash in [
        "",
        "0",
        "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD",
        &"g".repeat(64),
    ] {
        let mut v = fixture();
        v["assets"][0]["delivery"]["sha256"] = json!(hash);
        assert!(matches!(
            decode(&v, standard(), Limits::default()),
            Err(InventoryError::InvalidDigest(_))
        ));
    }
}
#[test]
fn limits_include_aggregate_probe_budget_and_checked_size_arithmetic() {
    let v = fixture();
    let limits = Limits {
        encoded_total_bytes: 59,
        ..Limits::default()
    };
    assert_eq!(
        decode(&v, standard(), limits).unwrap_err(),
        InventoryError::LimitExceeded
    );
    assert!(
        decode(
            &v,
            standard(),
            Limits {
                encoded_total_bytes: 60,
                ..limits
            }
        )
        .is_ok()
    );
    for size in [0, u64::MAX] {
        let mut v = fixture();
        v["assets"][0]["delivery"]["encoded_bytes"] = json!(size);
        assert!(matches!(
            decode(&v, standard(), Limits::default()),
            Err(InventoryError::InvalidSize(_))
        ));
    }
    assert!(
        Manifest::decode(
            SOURCE.as_bytes(),
            Limits {
                manifest_bytes: 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        Manifest::decode(
            SOURCE.as_bytes(),
            Limits {
                records: 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        Manifest::decode(
            SOURCE.as_bytes(),
            Limits {
                encoded_asset_bytes: u64::MAX,
                ..Limits::default()
            }
        )
        .is_err()
    );
}
#[test]
fn legacy_external_scope_is_empty_and_does_not_qualify_supervised_roles() {
    let mut v = fixture();
    v["scope"] = json!("legacy-external-deps");
    let r = InventoryRequest {
        scope: Scope::LegacyExternalDeps,
        ..standard()
    };
    assert_eq!(
        decode(&v, r, Limits::default()).unwrap_err(),
        InventoryError::LegacyScopeHasPayloads
    );
    v["assets"] = json!([]);
    let inventory = decode(&v, r, Limits::default()).unwrap();
    assert_eq!(inventory.assets().len(), 0);
    assert_eq!(inventory.request().scope, Scope::LegacyExternalDeps);
    assert!(decode(&v, standard(), Limits::default()).is_err());
}

#[test]
fn validation_rechecks_exact_original_json_length_including_whitespace() {
    let padded = format!("{SOURCE}                 ");
    let exact = Limits {
        manifest_bytes: padded.len(),
        ..Limits::default()
    };
    assert!(
        Manifest::decode(padded.as_bytes(), exact)
            .unwrap()
            .validate_inventory(standard(), exact)
            .is_ok()
    );
    let decoded = Manifest::decode(padded.as_bytes(), Limits::default()).unwrap();
    let stricter = Limits {
        manifest_bytes: padded.len() - 1,
        ..Limits::default()
    };
    assert_eq!(
        decoded
            .validate_inventory(standard(), stricter)
            .unwrap_err(),
        InventoryError::LimitExceeded
    );
    let decoded = Manifest::decode(SOURCE.as_bytes(), Limits::default()).unwrap();
    assert_eq!(
        decoded
            .validate_inventory(
                standard(),
                Limits {
                    manifest_bytes: 1,
                    ..Limits::default()
                }
            )
            .unwrap_err(),
        InventoryError::LimitExceeded
    );
}
