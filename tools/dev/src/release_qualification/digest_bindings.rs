//! Cryptographic artifact digest bindings and checksum verification.
//!
//! Enforces:
//! - Valid SHA256 hex digests for all upstream generator inputs (`tools/upstream/inputs.json`).
//! - Valid Git commit hashes and tags for all upstream source repositories (`docs/architecture/upstream-inputs.json`).
//! - Valid `AssetId` and references across the asset catalog (`rubix_assets::catalog()`).
//! - Cryptographic integrity and tamper detection for candidate distribution artifacts and checksum manifests (`SHA256SUMS`).

use crate::Result;
use rubix_assets::{ReleasePackager, catalog};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const EXPECTED_BASELINE: &str = "2ef1c4787989f11f868f81bb84ae2afd4a49a81d";

/// Validate a 64-character lowercase hexadecimal SHA-256 string.
pub fn is_valid_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// Validate a 40-character lowercase hexadecimal Git commit hash.
pub fn is_valid_git_commit_hex(value: &str) -> bool {
    value.len() == 40
        && value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[derive(Deserialize)]
struct UpstreamInputsDocument {
    sources: Vec<UpstreamInputEntry>,
}

#[derive(Deserialize)]
struct UpstreamInputEntry {
    id: String,
    #[serde(default)]
    path: Option<String>,
    url: String,
    sha256: String,
    bytes: u64,
}

/// Verifies that all upstream generator inputs in `tools/upstream/inputs.json`
/// have valid cryptographic SHA-256 bindings and positive sizes.
pub fn verify_upstream_inputs(root: &Path) -> Result<usize> {
    let path = root.join("tools/upstream/inputs.json");
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read upstream inputs at {}: {e}", path.display()))?;

    let doc: UpstreamInputsDocument = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;

    if doc.sources.is_empty() {
        return Err("upstream inputs source list cannot be empty".into());
    }

    for source in &doc.sources {
        if source.id.is_empty() {
            return Err(format!("empty id in upstream input source at {}", path.display()).into());
        }
        if let Some(p) = &source.path
            && p.is_empty()
        {
            return Err(format!("empty path in upstream input source {}", source.id).into());
        }
        if source.url.is_empty() {
            return Err(format!("empty url in upstream input source {}", source.id).into());
        }
        if !is_valid_sha256_hex(&source.sha256) {
            return Err(format!(
                "invalid sha256 hex digest for upstream input {}: {}",
                source.id, source.sha256
            )
            .into());
        }
        if source.bytes == 0 {
            return Err(
                format!("zero byte length declared for upstream input {}", source.id).into(),
            );
        }
    }

    Ok(doc.sources.len())
}

#[derive(Deserialize)]
struct ProvenanceDocument {
    baseline: String,
    sources: Vec<ProvenanceSourceEntry>,
}

#[derive(Deserialize)]
struct ProvenanceSourceEntry {
    repository: String,
    tag: String,
    commit: String,
}

/// Verifies that `docs/architecture/upstream-inputs.json` records authoritative
/// baseline commit and valid Git commit hashes for all upstream repositories.
pub fn verify_upstream_provenance(root: &Path) -> Result<usize> {
    let path = root.join("docs/architecture/upstream-inputs.json");
    let content = fs::read_to_string(&path).map_err(|e| {
        format!(
            "failed to read upstream provenance at {}: {e}",
            path.display()
        )
    })?;

    let doc: ProvenanceDocument = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;

    if doc.baseline != EXPECTED_BASELINE {
        return Err(format!(
            "baseline commit mismatch in {}: expected {EXPECTED_BASELINE}, got {}",
            path.display(),
            doc.baseline
        )
        .into());
    }

    if doc.sources.is_empty() {
        return Err("upstream provenance source list cannot be empty".into());
    }

    for source in &doc.sources {
        if source.repository.is_empty() {
            return Err("empty repository name in upstream provenance".into());
        }
        if source.tag.is_empty() {
            return Err(format!(
                "empty tag for repository {} in upstream provenance",
                source.repository
            )
            .into());
        }
        if !is_valid_git_commit_hex(&source.commit) {
            return Err(format!(
                "invalid git commit hash for repository {} ({}): {}",
                source.repository, source.tag, source.commit
            )
            .into());
        }
    }

    Ok(doc.sources.len())
}

/// Verifies that all entries in the asset catalog have valid identities and references.
pub fn verify_catalog_bindings() -> Result<usize> {
    let items = catalog();
    if items.is_empty() {
        return Err("asset catalog is empty".into());
    }

    for item in items {
        if item.reference.is_empty() {
            return Err(format!("empty reference for catalog asset {:?}", item.id).into());
        }
    }

    Ok(items.len())
}

/// Verifies candidate release artifacts against an expected checksum map or `SHA256SUMS`.
/// Strictly fails closed upon missing files, unlisted files, traversal keys, or digest mismatches.
pub fn verify_artifacts_integrity(
    dist_dir: &Path,
    expected_checksums: &BTreeMap<String, String>,
) -> Result<usize> {
    if expected_checksums.is_empty() {
        return Err("expected checksums map cannot be empty".into());
    }

    if !dist_dir.is_dir() {
        return Err(format!(
            "distribution directory does not exist or is not a directory: {}",
            dist_dir.display()
        )
        .into());
    }

    // 1. Validate keys are relative, non-traversing, root-contained filenames
    for (filename, expected_digest) in expected_checksums {
        let p = Path::new(filename);
        if filename.is_empty() || p.is_absolute() {
            return Err(
                format!("invalid absolute or empty artifact path key: '{filename}'").into(),
            );
        }
        for component in p.components() {
            match component {
                std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_) => {
                    return Err(format!(
                        "traversal or non-relative component in artifact key: '{filename}'"
                    )
                    .into());
                },
                _ => {},
            }
        }
        if !is_valid_sha256_hex(expected_digest) {
            return Err(format!(
                "invalid expected sha256 format for {filename}: {expected_digest}"
            )
            .into());
        }
    }

    // 2. Discover actual files in dist_dir and ensure no symlinks or unlisted files
    let mut actual_files = std::collections::BTreeSet::new();
    for entry in fs::read_dir(dist_dir).map_err(|e| {
        format!(
            "failed to read distribution directory {}: {e}",
            dist_dir.display()
        )
    })? {
        let entry =
            entry.map_err(|e| format!("failed to read entry in {}: {e}", dist_dir.display()))?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| format!("cannot read symlink metadata for {}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "symlinks are not permitted in release distribution: {}",
                path.display()
            )
            .into());
        }
        if meta.is_file() {
            let fname = entry.file_name().to_string_lossy().to_string();
            actual_files.insert(fname);
        } else if meta.is_dir() {
            return Err(format!(
                "subdirectories not expected in flat release distribution: {}",
                path.display()
            )
            .into());
        }
    }

    // 3. Reconcile expected files vs actual files
    for filename in expected_checksums.keys() {
        if !actual_files.contains(filename) {
            return Err(format!(
                "missing expected release artifact: {}",
                dist_dir.join(filename).display()
            )
            .into());
        }
    }

    for actual in &actual_files {
        if !expected_checksums.contains_key(actual) {
            return Err(format!(
                "unlisted file present in distribution directory: {}",
                dist_dir.join(actual).display()
            )
            .into());
        }
    }

    // 4. Check each file matches observed SHA-256
    for (filename, expected_digest) in expected_checksums {
        let file_path = dist_dir.join(filename);
        let content = fs::read(&file_path)
            .map_err(|e| format!("failed to read artifact {}: {e}", file_path.display()))?;
        let observed_digest = ReleasePackager::sha256_hex(&content);

        if &observed_digest != expected_digest {
            return Err(format!(
                "cryptographic digest mismatch for {filename}: expected {expected_digest}, observed {observed_digest}"
            )
            .into());
        }
    }

    Ok(expected_checksums.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_sha256_validation() {
        assert!(is_valid_sha256_hex(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ));
        // uppercase rejected
        assert!(!is_valid_sha256_hex(
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855"
        ));
        // wrong length
        assert!(!is_valid_sha256_hex(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85"
        ));
        // non-hex
        assert!(!is_valid_sha256_hex(
            "z3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ));
    }

    #[test]
    fn test_valid_git_commit_validation() {
        assert!(is_valid_git_commit_hex(
            "2ef1c4787989f11f868f81bb84ae2afd4a49a81d"
        ));
        assert!(!is_valid_git_commit_hex(
            "2EF1C4787989F11F868F81BB84AE2AFD4A49A81D"
        ));
        assert!(!is_valid_git_commit_hex("short"));
    }

    #[test]
    fn test_tamper_detection_fails_closed() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let file1 = temp.path().join("artifact1.tar.gz");
        let file2 = temp.path().join("artifact2.tar.gz");

        fs::write(&file1, b"original content 1")?;
        fs::write(&file2, b"original content 2")?;

        let mut checksums = BTreeMap::new();
        checksums.insert(
            "artifact1.tar.gz".into(),
            ReleasePackager::sha256_hex(b"original content 1"),
        );
        checksums.insert(
            "artifact2.tar.gz".into(),
            ReleasePackager::sha256_hex(b"original content 2"),
        );

        // Valid should pass
        let verified = verify_artifacts_integrity(temp.path(), &checksums)?;
        assert_eq!(verified, 2);

        // Tampered content must fail
        fs::write(&file1, b"tampered content 1")?;
        let err = verify_artifacts_integrity(temp.path(), &checksums).unwrap_err();
        assert!(err.to_string().contains("digest mismatch"));

        Ok(())
    }

    #[test]
    fn test_traversal_and_unlisted_files_rejected() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let file1 = temp.path().join("artifact1.tar.gz");
        fs::write(&file1, b"original content 1")?;

        let mut checksums = BTreeMap::new();
        checksums.insert(
            "../outside.tar.gz".into(),
            ReleasePackager::sha256_hex(b"original content 1"),
        );

        // Traversal key must fail closed
        let err = verify_artifacts_integrity(temp.path(), &checksums).unwrap_err();
        assert!(err.to_string().contains("traversal"));

        // Unlisted file must fail closed
        let mut valid_checksums = BTreeMap::new();
        valid_checksums.insert(
            "artifact1.tar.gz".into(),
            ReleasePackager::sha256_hex(b"original content 1"),
        );

        // Add extra unlisted file
        let unlisted = temp.path().join("unlisted.tar.gz");
        fs::write(&unlisted, b"rogue artifact")?;
        let err = verify_artifacts_integrity(temp.path(), &valid_checksums).unwrap_err();
        assert!(err.to_string().contains("unlisted file"));

        Ok(())
    }
}
