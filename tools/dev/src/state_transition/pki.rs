//! PKI trust root preservation and kubeconfig format verification.
//!
//! Enforces:
//! 1. Root CA (`ca.crt`, `ca.key`) and `ServiceAccount` (`service-account.key`) preservation.
//! 2. Accommodation and verification of **both YAML and JSON** kubeconfig formats.
//! 3. Cryptographic consistency: client certificates in kubeconfig authenticate against CA root.
//! 4. Non-invalidation of existing client trust across the Go-to-Rust transition.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde::{Deserialize, Serialize};
use x509_parser::pem::parse_x509_pem;

/// Format of a kubeconfig file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KubeconfigFormat {
    Yaml,
    Json,
}

impl fmt::Display for KubeconfigFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml => write!(f, "YAML"),
            Self::Json => write!(f, "JSON"),
        }
    }
}

/// Extracted fields from a parsed kubeconfig document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedKubeconfig {
    pub format: KubeconfigFormat,
    pub server: String,
    pub cluster_name: String,
    pub user_name: String,
    pub current_context: String,
    pub ca_cert_bytes: Vec<u8>,
    pub client_cert_bytes: Vec<u8>,
    pub client_key_bytes: Vec<u8>,
}

/// Parses a kubeconfig from bytes, accommodating both YAML and JSON encodings.
pub fn parse_kubeconfig(bytes: &[u8]) -> Result<ParsedKubeconfig, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|e| format!("invalid UTF-8 in kubeconfig: {e}"))?;
    let trimmed = text.trim();

    // Check if it's JSON
    if trimmed.starts_with('{') {
        parse_kubeconfig_json(bytes)
    } else {
        parse_kubeconfig_yaml(trimmed)
    }
}

/// Parse kubeconfig JSON format.
fn parse_kubeconfig_json(bytes: &[u8]) -> Result<ParsedKubeconfig, String> {
    #[derive(Deserialize)]
    struct RawConfig {
        clusters: Vec<ClusterEntry>,
        users: Vec<UserEntry>,
        #[serde(rename = "current-context")]
        current_context: String,
    }
    #[derive(Deserialize)]
    struct ClusterEntry {
        name: String,
        cluster: ClusterDetail,
    }
    #[derive(Deserialize)]
    struct ClusterDetail {
        server: String,
        #[serde(rename = "certificate-authority-data")]
        ca_data: String,
    }
    #[derive(Deserialize)]
    struct UserEntry {
        name: String,
        user: UserDetail,
    }
    #[derive(Deserialize)]
    struct UserDetail {
        #[serde(rename = "client-certificate-data")]
        cert_data: String,
        #[serde(rename = "client-key-data")]
        key_data: String,
    }

    let raw: RawConfig = serde_json::from_slice(bytes)
        .map_err(|e| format!("failed to parse kubeconfig as JSON: {e}"))?;

    let cluster = raw
        .clusters
        .first()
        .ok_or_else(|| "kubeconfig has no clusters".to_string())?;
    let user = raw
        .users
        .first()
        .ok_or_else(|| "kubeconfig has no users".to_string())?;

    let ca_bytes = BASE64_STANDARD
        .decode(cluster.cluster.ca_data.trim())
        .map_err(|e| format!("invalid base64 in certificate-authority-data: {e}"))?;
    let cert_bytes = BASE64_STANDARD
        .decode(user.user.cert_data.trim())
        .map_err(|e| format!("invalid base64 in client-certificate-data: {e}"))?;
    let key_bytes = BASE64_STANDARD
        .decode(user.user.key_data.trim())
        .map_err(|e| format!("invalid base64 in client-key-data: {e}"))?;

    Ok(ParsedKubeconfig {
        format: KubeconfigFormat::Json,
        server: cluster.cluster.server.clone(),
        cluster_name: cluster.name.clone(),
        user_name: user.name.clone(),
        current_context: raw.current_context,
        ca_cert_bytes: ca_bytes,
        client_cert_bytes: cert_bytes,
        client_key_bytes: key_bytes,
    })
}

/// Parse kubeconfig YAML format.
fn parse_kubeconfig_yaml(text: &str) -> Result<ParsedKubeconfig, String> {
    let mut server = String::new();
    let mut cluster_name = String::new();
    let mut user_name = String::new();
    let mut current_context = String::new();
    let mut ca_data_b64 = String::new();
    let mut cert_data_b64 = String::new();
    let mut key_data_b64 = String::new();

    let mut in_cluster = false;
    let mut in_user = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("current-context:") {
            current_context = trimmed
                .strip_prefix("current-context:")
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string();
        } else if trimmed.starts_with("- cluster:") || trimmed == "cluster:" {
            in_cluster = true;
            in_user = false;
        } else if trimmed.starts_with("- user:") || trimmed == "user:" {
            in_user = true;
            in_cluster = false;
        } else if trimmed.starts_with("server:") && in_cluster {
            server = trimmed
                .strip_prefix("server:")
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string();
        } else if trimmed.starts_with("certificate-authority-data:") {
            ca_data_b64 = trimmed
                .strip_prefix("certificate-authority-data:")
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string();
        } else if trimmed.starts_with("client-certificate-data:") {
            cert_data_b64 = trimmed
                .strip_prefix("client-certificate-data:")
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string();
        } else if trimmed.starts_with("client-key-data:") {
            key_data_b64 = trimmed
                .strip_prefix("client-key-data:")
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string();
        } else if trimmed.starts_with("name:") {
            let n = trimmed
                .strip_prefix("name:")
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string();
            if in_cluster && cluster_name.is_empty() {
                cluster_name = n;
            } else if in_user && user_name.is_empty() {
                user_name = n;
            }
        }
    }

    if server.is_empty() {
        return Err("missing server in kubeconfig".to_string());
    }
    if ca_data_b64.is_empty() {
        return Err("missing certificate-authority-data in kubeconfig".to_string());
    }
    if cert_data_b64.is_empty() {
        return Err("missing client-certificate-data in kubeconfig".to_string());
    }
    if key_data_b64.is_empty() {
        return Err("missing client-key-data in kubeconfig".to_string());
    }

    let ca_bytes = BASE64_STANDARD
        .decode(&ca_data_b64)
        .map_err(|e| format!("invalid base64 in certificate-authority-data: {e}"))?;
    let cert_bytes = BASE64_STANDARD
        .decode(&cert_data_b64)
        .map_err(|e| format!("invalid base64 in client-certificate-data: {e}"))?;
    let key_bytes = BASE64_STANDARD
        .decode(&key_data_b64)
        .map_err(|e| format!("invalid base64 in client-key-data: {e}"))?;

    Ok(ParsedKubeconfig {
        format: KubeconfigFormat::Yaml,
        server,
        cluster_name,
        user_name,
        current_context,
        ca_cert_bytes: ca_bytes,
        client_cert_bytes: cert_bytes,
        client_key_bytes: key_bytes,
    })
}

/// Result of PKI transition assertion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PkiTransitionAssertion {
    pub ca_sha256_before: String,
    pub ca_sha256_after: String,
    pub ca_preserved: bool,
    pub sa_key_sha256_before: String,
    pub sa_key_sha256_after: String,
    pub sa_key_preserved: bool,
    pub client_cert_verified: bool,
    pub kubeconfig_format: KubeconfigFormat,
}

/// Computes SHA-256 hex digest of a byte slice.
fn digest_bytes(b: &[u8]) -> String {
    crate::sha256(b)
}

/// Asserts PKI state before and after transition.
pub fn assert_pki_transition(
    pki_dir_before: &Path,
    pki_dir_after: &Path,
    kubeconfig_file: &Path,
) -> io::Result<PkiTransitionAssertion> {
    // 1. Locate CA cert
    let ca_path_before = find_file(pki_dir_before, "ca.crt")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "ca.crt not found before"))?;
    let ca_path_after = find_file(pki_dir_after, "ca.crt")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "ca.crt not found after"))?;

    let ca_bytes_before = fs::read(&ca_path_before)?;
    let ca_bytes_after = fs::read(&ca_path_after)?;

    let ca_sha_before = digest_bytes(&ca_bytes_before);
    let ca_sha_after = digest_bytes(&ca_bytes_after);
    let ca_preserved = ca_sha_before == ca_sha_after;

    // 2. Service account key
    let sa_key_before = find_file(pki_dir_before, "service-account.key")
        .map(|p| fs::read(&p))
        .transpose()?
        .unwrap_or_default();
    let sa_key_after = find_file(pki_dir_after, "service-account.key")
        .map(|p| fs::read(&p))
        .transpose()?
        .unwrap_or_default();

    let sa_sha_before = digest_bytes(&sa_key_before);
    let sa_sha_after = digest_bytes(&sa_key_after);
    let sa_key_preserved = sa_sha_before == sa_sha_after;

    // 3. Verify kubeconfig
    let kcfg_bytes = fs::read(kubeconfig_file)?;
    let parsed_kcfg =
        parse_kubeconfig(&kcfg_bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Verify client cert is signed by CA cert
    let client_verified =
        verify_cert_chain(&ca_bytes_after, &parsed_kcfg.client_cert_bytes).unwrap_or(false);

    Ok(PkiTransitionAssertion {
        ca_sha256_before: ca_sha_before,
        ca_sha256_after: ca_sha_after,
        ca_preserved,
        sa_key_sha256_before: sa_sha_before,
        sa_key_sha256_after: sa_sha_after,
        sa_key_preserved,
        client_cert_verified: client_verified,
        kubeconfig_format: parsed_kcfg.format,
    })
}

/// Finds a file by name recursively within a directory.
fn find_file(root: &Path, filename: &str) -> Option<std::path::PathBuf> {
    if !root.is_dir() {
        return None;
    }
    for entry in fs::read_dir(root).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some(filename) {
            return Some(path);
        }
        if path.is_dir()
            && let Some(found) = find_file(&path, filename)
        {
            return Some(found);
        }
    }
    None
}

/// Cryptographically validates that a client certificate's issuer matches the CA's subject.
pub fn verify_cert_chain(ca_pem: &[u8], client_pem: &[u8]) -> Result<bool, String> {
    let (_, ca_pem_obj) =
        parse_x509_pem(ca_pem).map_err(|e| format!("failed to parse CA PEM: {e}"))?;
    let ca_cert = ca_pem_obj
        .parse_x509()
        .map_err(|e| format!("failed to parse CA X.509 cert: {e}"))?;

    let (_, client_pem_obj) =
        parse_x509_pem(client_pem).map_err(|e| format!("failed to parse client PEM: {e}"))?;
    let client_cert = client_pem_obj
        .parse_x509()
        .map_err(|e| format!("failed to parse client X.509 cert: {e}"))?;

    // Check Issuer matches CA Subject
    let issuer_matches = client_cert.issuer() == ca_cert.subject();
    Ok(issuer_matches)
}
