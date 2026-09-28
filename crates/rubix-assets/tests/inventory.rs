use rubix_assets::{
    AssetId, DeclaredInventory, Delivery, InventoryError, InventoryRequest, Limits, Manifest,
    Scope, Variant,
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
                assert_eq!(inventory.assets().len(), 19);
                assert_eq!(
                    inventory.bundled_assets().len(),
                    if variant == Variant::Online {
                        15
                    } else {
                        19 - unavailable
                    }
                );
                assert_eq!(
                    inventory
                        .assets()
                        .filter(|(_, d)| matches!(d, Delivery::Unavailable {}))
                        .count(),
                    unavailable
                );
                cells += 1;
            }
        }
    }
    assert_eq!(cells, 16);
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
