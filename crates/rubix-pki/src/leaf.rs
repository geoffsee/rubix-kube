//! Leaf certificate generation for Rubix PKI

use super::{PkiError, to_pkcs1_pem, atomic_write};
use rcgen::{
    CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose,
    PKCS_RSA_SHA256, RsaKeySize,
};
use std::fs;
use std::path::Path;

/// Generate (or reuse) a leaf certificate signed by the CA.
///
/// * `ca_cert_path` – path to the CA certificate (not needed for signing but kept for API compatibility).
/// * `ca_key_path` – path to the CA private key.
/// * `leaf_cert_path` – where the leaf certificate will be written.
/// * `leaf_key_path` – where the leaf private key will be written.
/// * `common_name` – the leaf's common name (e.g., "kubelet").
/// * `san_dns` – DNS Subject Alternative Names for the leaf.
pub fn ensure_leaf_certificate(
    ca_cert_path: &Path,
    ca_key_path: &Path,
    leaf_cert_path: &Path,
    leaf_key_path: &Path,
    common_name: &str,
    san_dns: &[&str],
) -> Result<(), PkiError> {
    // Reuse existing leaf files if they already exist.
    if leaf_cert_path.exists() && leaf_key_path.exists() {
        return Ok(());
    }

    // Load CA private key (PEM). The CA certificate itself is not required for signing.
    let ca_key_pem = fs::read_to_string(ca_key_path)?;
    let ca_key_pair = rcgen::KeyPair::from_pem(&ca_key_pem).map_err(PkiError::Rcgen)?;

    // Build leaf certificate parameters.
    let mut leaf_params = CertificateParams::new(san_dns.iter().map(|s| s.to_string()).collect())
        .map_err(PkiError::Rcgen)?;
    leaf_params.distinguished_name = DistinguishedName::new();
    leaf_params
        .distinguished_name
        .push(DnType::CommonName, common_name);
    leaf_params.is_ca = IsCa::NoCa;
    leaf_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    leaf_params.alg = &PKCS_RSA_SHA256;
    leaf_params.key_pair = Some(KeyPair::generate_rsa_for(&PKCS_RSA_SHA256, RsaKeySize::_2048)?);

    // Sign the leaf certificate with the CA key.
    let leaf_cert = leaf_params
        .signed_by(&ca_key_pair)
        .map_err(PkiError::Rcgen)?;

    // Serialize leaf private key to PKCS#1 PEM.
    let leaf_key_pair = leaf_params
        .key_pair
        .expect("Key pair should be present after generation");
    let leaf_pkcs8_pem = leaf_key_pair.serialize_pem();
    let leaf_key_pem = to_pkcs1_pem(&leaf_pkcs8_pem)?;

    // Write leaf certificate and key atomically.
    atomic_write(leaf_cert_path, leaf_cert.pem().as_bytes(), 0o644)?;
    atomic_write(leaf_key_path, leaf_key_pem.as_bytes(), 0o600)?;

    Ok(())
}
