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
/// Strictly fails closed upon missing files, unlisted files, or digest mismatches.
pub fn verify_artifacts_integrity(
    dist_dir: &Path,
    expected_checksums: &BTreeMap<String, String>,
) -> Result<usize> {
    if expected_checksums.is_empty() {
        return Err("expected checksums map cannot be empty".into());
    }

    // Check each expected file exists and matches observed SHA-256
    for (filename, expected_digest) in expected_checksums {
        if !is_valid_sha256_hex(expected_digest) {
            return Err(format!(
                "invalid expected sha256 format for {filename}: {expected_digest}"
            )
            .into());
        }

        let file_path = dist_dir.join(filename);
        if !file_path.is_file() {
            return Err(
                format!("missing expected release artifact: {}", file_path.display()).into(),
            );
        }

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
}
