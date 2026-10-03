#[path = "common/archive.rs"]
mod vectors;

use rubix_assets::{
    AssetId, DeclaredInventory, Encoding, InventoryRequest, Kind, Limits, Manifest,
    MaterializationError, Materializer, Scope, Variant,
};
use rubix_platform::{Architecture, Libc, NodeTarget};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
use vectors::{entry, gzip, hex};

const ONLINE_ARM64_JSON: &str = include_str!("fixtures/online-arm64.json");
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Restore write permissions in case test made it read-only
            let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o755));
        }
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn arm64_request(variant: Variant) -> InventoryRequest {
    InventoryRequest {
        target: NodeTarget {
            architecture: Architecture::Arm64,
            libc: Libc::Glibc,
        },
        variant,
        scope: Scope::SupervisedBundle,
    }
}

fn create_synthetic_payload(
    id: AssetId,
    encoding: Encoding,
) -> (Vec<u8>, Vec<u8>, u64, String, [u8; 32]) {
    // Returns: (uncompressed_content, encoded_bytes, encoded_length, encoded_hex, encoded_hash)
    let content = match id {
        AssetId::ImageCoredns
        | AssetId::ImagePause
        | AssetId::ImageLocalPath
        | AssetId::ImageLocalPathHelper
        | AssetId::ImagePortainerAgent
        | AssetId::ImageD2k => {
            // Valid minimal tar.gz image payload
            let tar_data = entry("layer.tar", b"synthetic image layer");
            gzip(&tar_data)
        },
        _ => format!("synthetic-arm64-binary-{id:?}").into_bytes(),
    };

    let encoded_bytes = match encoding {
        Encoding::Identity | Encoding::Gzip => content.clone(),
        Encoding::Zstd => zstd::encode_all(content.as_slice(), 0).expect("zstd encode"),
    };

    let encoded_len = encoded_bytes.len() as u64;
    let digest: [u8; 32] = Sha256::digest(&encoded_bytes).into();
    let digest_hex = hex(&encoded_bytes);

    (content, encoded_bytes, encoded_len, digest_hex, digest)
}

fn build_synthetic_manifest_and_archive(
    source_json: &str,
    variant: Variant,
) -> (DeclaredInventory, Vec<u8>) {
    let mut manifest_val: Value = serde_json::from_str(source_json).expect("valid fixture");
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
            let encoding = match encoding_str {
                "identity" => Encoding::Identity,
                "gzip" => Encoding::Gzip,
                "zstd" => Encoding::Zstd,
                other => panic!("unknown encoding {other}"),
            };

            let (_content, encoded_bytes, encoded_len, digest_hex, _digest) =
                create_synthetic_payload(id, encoding);

            row["delivery"]["encoded_bytes"] = json!(encoded_len);
            row["delivery"]["sha256"] = json!(digest_hex);

            let path = row["delivery"]["path"].as_str().expect("path");
            archive_bytes.extend(entry(path, &encoded_bytes));
        }
    }
    archive_bytes.extend([0u8; 1024]); // tar EOF blocks

    let manifest_bytes = serde_json::to_vec(&manifest_val).expect("serialized json");
    let inventory = Manifest::decode(&manifest_bytes, Limits::default())
        .expect("decode manifest")
        .validate_inventory(arm64_request(variant), Limits::default())
        .expect("validate inventory");

    (inventory, archive_bytes)
}

#[test]
fn accepted_linux_arm64_glibc_payload_cell_materializes_idempotently() {
    for (fixture_json, variant) in [
        (ONLINE_ARM64_JSON, Variant::Online),
        (OFFLINE_ARM64_JSON, Variant::Offline),
    ] {
        let (inventory, archive_data) = build_synthetic_manifest_and_archive(fixture_json, variant);
        let test_dir = TestDir::new("rubix-test-mat-arm64");
        let materializer = Materializer::new(inventory, test_dir.path());

        // First materialization
        let outcome1 = materializer
            .materialize_from_archive(Cursor::new(&archive_data))
            .expect("first materialization succeeds");

        assert_eq!(
            outcome1.assets.len(),
            materializer.inventory().bundled_assets().len()
        );

        for materialized in &outcome1.assets {
            assert!(
                materialized.path.exists(),
                "path {:?} must exist",
                materialized.path
            );
            let meta = fs::metadata(&materialized.path).expect("file metadata");
            assert!(meta.is_file());

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = meta.permissions().mode() & 0o777;
                match materialized.kind {
                    Kind::Executable => {
                        assert_eq!(mode, 0o755, "executable {:?} mode", materialized.path);
                    },
                    Kind::Image => {
                        assert_eq!(mode, 0o644, "image payload {:?} mode", materialized.path);
                    },
                }
            }
        }

        // Second materialization (idempotency check)
        let outcome2 = materializer
            .materialize_from_archive(Cursor::new(&archive_data))
            .expect("second materialization succeeds idempotently");

        assert_eq!(outcome2.assets.len(), outcome1.assets.len());

        for (a1, a2) in outcome1.assets.iter().zip(outcome2.assets.iter()) {
            assert_eq!(a1.id, a2.id);
            assert_eq!(a1.path, a2.path);
            assert_eq!(a1.mode, a2.mode);
            assert_eq!(a1.bytes_written, a2.bytes_written);
            assert_eq!(a1.sha256, a2.sha256);
        }
    }
}

#[test]
fn digest_verification_enforces_arm64_manifest_hashes() {
    let online_bytes = ONLINE_ARM64_JSON.as_bytes();
    let online = Manifest::decode(online_bytes, Limits::default())
        .expect("valid online manifest")
        .validate_inventory(arm64_request(Variant::Online), Limits::default())
        .expect("valid inventory");

    let test_dir = TestDir::new("rubix-test-digest-enforce");
    let materializer = Materializer::new(online, test_dir.path());

    // Provide bad data for kube-apiserver
    let bad_data = vec![0x99u8; 79_888_568];
    let err = materializer
        .materialize_single_asset(AssetId::KubeApiserver, Cursor::new(&bad_data))
        .unwrap_err();

    match err {
        MaterializationError::DigestMismatch {
            asset,
            expected,
            observed,
        } => {
            assert_eq!(asset, AssetId::KubeApiserver);
            let expected_hex = expected.iter().fold(String::new(), |mut s, b| {
                use std::fmt::Write;
                write!(&mut s, "{b:02x}").unwrap();
                s
            });
            assert_eq!(
                expected_hex, "4e5fe160e7b90e84faab827e71a101f0472a920385abfb7f6bba36ee783529e1",
                "enforces exact ARM64 manifest hash"
            );
            assert_ne!(expected, observed);
        },
        other => panic!("expected DigestMismatch, got {other:?}"),
    }

    // Destination file must NOT exist after digest mismatch
    let final_dest = materializer
        .layout()
        .resolve_destination(test_dir.path(), AssetId::KubeApiserver)
        .unwrap();
    assert!(
        !final_dest.exists(),
        "corrupted asset must not be placed at destination"
    );
}

#[test]
fn repeated_materialization_preserves_content_and_permissions() {
    let (inventory, archive_data) =
        build_synthetic_manifest_and_archive(ONLINE_ARM64_JSON, Variant::Online);
    let test_dir = TestDir::new("rubix-test-perm-preserve");
    let materializer = Materializer::new(inventory, test_dir.path());

    let outcome = materializer
        .materialize_from_archive(Cursor::new(&archive_data))
        .expect("first materialization succeeds");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Intentionally tamper with permissions on disk
        for asset in &outcome.assets {
            fs::set_permissions(&asset.path, fs::Permissions::from_mode(0o600)).unwrap();
            let meta = fs::metadata(&asset.path).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }

        // Repeated materialization should restore correct permissions
        let outcome2 = materializer
            .materialize_from_archive(Cursor::new(&archive_data))
            .expect("second materialization succeeds");

        for asset in &outcome2.assets {
            let meta = fs::metadata(&asset.path).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            match asset.kind {
                Kind::Executable => assert_eq!(mode, 0o755),
                Kind::Image => assert_eq!(mode, 0o644),
            }
        }
    }
}

#[test]
fn failed_extraction_leaves_no_usable_looking_partial_install() {
    let (inventory, archive_data) =
        build_synthetic_manifest_and_archive(ONLINE_ARM64_JSON, Variant::Online);
    let test_dir = TestDir::new("rubix-test-partial-install");
    let materializer = Materializer::new(inventory, test_dir.path());

    // Truncate archive data right in the middle
    let truncated_len = archive_data.len() / 2;
    let corrupted_archive = &archive_data[..truncated_len];

    let result = materializer.materialize_from_archive(Cursor::new(corrupted_archive));
    assert!(
        result.is_err(),
        "materialization must fail on truncated archive"
    );

    // Verify destination has NO partial files
    for (id, _, _, _) in materializer.inventory().bundled_assets() {
        let dest = materializer
            .layout()
            .resolve_destination(test_dir.path(), id)
            .unwrap();
        assert!(
            !dest.exists(),
            "destination file {dest:?} must not exist after failed extraction"
        );
    }

    // Verify staging directory does not remain
    let entries: Vec<_> = fs::read_dir(test_dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            !name.starts_with(".staging-"),
            "staging directory {name} should be cleaned up on failure"
        );
    }
}

#[test]
fn corrupt_archive_fails_precisely_without_writing_outside_owned_paths() {
    let (inventory, mut archive_data) =
        build_synthetic_manifest_and_archive(ONLINE_ARM64_JSON, Variant::Online);
    let test_dir = TestDir::new("rubix-test-corrupt-archive");
    let materializer = Materializer::new(inventory, test_dir.path());

    // Test 1: corrupt tar header checksum
    if archive_data.len() >= 150 {
        archive_data[148..156].copy_from_slice(b"999999\0 ");
    }
    let res = materializer.materialize_from_archive(Cursor::new(&archive_data));
    assert!(
        matches!(res, Err(MaterializationError::CorruptArchive(_))),
        "must report CorruptArchive on header checksum failure"
    );

    // Test 2: completely random bytes
    let random_data = vec![0xde, 0xad, 0xbe, 0xef, 0x12, 0x34];
    let res2 = materializer.materialize_from_archive(Cursor::new(&random_data));
    assert!(res2.is_err());

    // Destination root must have no leaked files
    for (id, _, _, _) in materializer.inventory().bundled_assets() {
        let dest = materializer
            .layout()
            .resolve_destination(test_dir.path(), id)
            .unwrap();
        assert!(!dest.exists());
    }
}

#[test]
#[cfg(unix)]
fn read_only_destination_reports_precise_failure() {
    use std::os::unix::fs::PermissionsExt;

    let (inventory, archive_data) =
        build_synthetic_manifest_and_archive(ONLINE_ARM64_JSON, Variant::Online);
    let test_dir = TestDir::new("rubix-test-read-only");

    // Make the test dir read-only
    fs::set_permissions(test_dir.path(), fs::Permissions::from_mode(0o555)).unwrap();

    let materializer = Materializer::new(inventory, test_dir.path());
    let res = materializer.materialize_from_archive(Cursor::new(&archive_data));

    // Restore permissions so drop cleanup can succeed
    let _ = fs::set_permissions(test_dir.path(), fs::Permissions::from_mode(0o755));

    assert!(
        matches!(
            res,
            Err(MaterializationError::ReadOnlyDestination { .. } | MaterializationError::Io { .. })
        ),
        "must report ReadOnlyDestination or PermissionDenied Io error, got {res:?}"
    );
}

#[test]
fn custom_root_fixture_stays_strictly_within_configured_root() {
    let (_inventory, _archive_data) =
        build_synthetic_manifest_and_archive(ONLINE_ARM64_JSON, Variant::Online);
    let test_dir = TestDir::new("rubix-test-path-traversal");

    let layout = materializer_layout_with_traversal();
    let err = layout
        .resolve_destination(test_dir.path(), AssetId::KubeApiserver)
        .unwrap_err();

    assert!(
        matches!(err, MaterializationError::PathEscapesRoot(_)),
        "path traversal must be rejected with PathEscapesRoot, got {err:?}"
    );

    // Absolute path escaping
    let layout_abs =
        rubix_assets::AssetLayout::canonical().with_path(AssetId::Kubelet, "/etc/shadow");
    let err_abs = layout_abs
        .resolve_destination(test_dir.path(), AssetId::Kubelet)
        .unwrap_err();
    assert!(
        matches!(err_abs, MaterializationError::PathEscapesRoot(_)),
        "absolute escape must be rejected"
    );
}

fn materializer_layout_with_traversal() -> rubix_assets::AssetLayout {
    rubix_assets::AssetLayout::canonical().with_path(AssetId::KubeApiserver, "../../../etc/passwd")
}
