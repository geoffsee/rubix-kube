use rcgen::{
    BasicConstraints, CertificateParams, IsCa, KeyPair, KeyUsagePurpose, PKCS_RSA_SHA256,
    RsaKeySize,
};
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
pub mod cluster;
pub mod kubeconfig;
pub mod leaf;
pub mod profile;
pub mod rotate;

#[derive(Debug)]
pub enum PkiError {
    Io(std::io::Error),
    Rcgen(rcgen::Error),
    InvalidKey,
    InvalidCert,
    UnsafePath(String),
}

impl std::error::Error for PkiError {}
impl std::fmt::Display for PkiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl From<std::io::Error> for PkiError {
    fn from(e: std::io::Error) -> Self {
        PkiError::Io(e)
    }
}
impl From<rcgen::Error> for PkiError {
    fn from(e: rcgen::Error) -> Self {
        PkiError::Rcgen(e)
    }
}

pub fn ensure_ca(cert_path: &Path, key_path: &Path, common_name: &str) -> Result<(), PkiError> {
    if cert_path.exists() && key_path.exists() {
        return Ok(());
    }

    if let Some(parent) = cert_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut params =
        CertificateParams::new(vec![common_name.to_string()]).map_err(PkiError::Rcgen)?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let key_pair = KeyPair::generate_rsa_for(&PKCS_RSA_SHA256, RsaKeySize::_2048)?;

    let cert = params.self_signed(&key_pair)?;

    let pkcs8_pem = key_pair.serialize_pem();
    let key_pem = to_pkcs1_pem(&pkcs8_pem)?;

    atomic_write(cert_path, cert.pem().as_bytes(), 0o644)?;
    atomic_write(key_path, key_pem.as_bytes(), 0o600)?;

    Ok(())
}

pub fn ensure_service_account_key(key_path: &Path) -> Result<(), PkiError> {
    if key_path.exists() {
        return Ok(());
    }

    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let key_pair = KeyPair::generate_rsa_for(&PKCS_RSA_SHA256, RsaKeySize::_2048)?;
    let pkcs8_pem = key_pair.serialize_pem();
    let key_pem = to_pkcs1_pem(&pkcs8_pem)?;

    atomic_write(key_path, key_pem.as_bytes(), 0o600)?;

    Ok(())
}

fn to_pkcs1_pem(pkcs8_pem: &str) -> Result<String, PkiError> {
    use pkcs8::der::Decode;

    let parsed_pem = pem::parse(pkcs8_pem).map_err(|_| PkiError::InvalidKey)?;
    let pkcs8_doc =
        pkcs8::PrivateKeyInfo::from_der(parsed_pem.contents()).map_err(|_| PkiError::InvalidKey)?;

    let pkcs1_pem = pem::encode(&pem::Pem::new("RSA PRIVATE KEY", pkcs8_doc.private_key));
    Ok(pkcs1_pem)
}

pub fn validate_certificate_pem(bytes: &[u8]) -> Result<(), PkiError> {
    let pem_entries = pem::parse_many(bytes).map_err(|_| PkiError::InvalidCert)?;
    if pem_entries.is_empty() {
        return Err(PkiError::InvalidCert);
    }
    let mut found = false;
    for entry in pem_entries {
        if entry.tag() == "CERTIFICATE" {
            found = true;
            let (_, _cert) = x509_parser::parse_x509_certificate(entry.contents())
                .map_err(|_| PkiError::InvalidCert)?;
        }
    }
    if !found {
        return Err(PkiError::InvalidCert);
    }
    Ok(())
}

pub fn validate_private_key_pem(bytes: &[u8]) -> Result<(), PkiError> {
    let pem_entries = pem::parse_many(bytes).map_err(|_| PkiError::InvalidKey)?;
    if pem_entries.is_empty() {
        return Err(PkiError::InvalidKey);
    }
    let mut found = false;
    for entry in pem_entries {
        let tag = entry.tag();
        if tag == "RSA PRIVATE KEY" || tag == "PRIVATE KEY" || tag == "EC PRIVATE KEY" {
            found = true;
            if entry.contents().is_empty() {
                return Err(PkiError::InvalidKey);
            }
            let text = std::str::from_utf8(bytes).map_err(|_| PkiError::InvalidKey)?;
            if KeyPair::from_pem(text).is_err()
                && pkcs8::PrivateKeyInfo::try_from(entry.contents()).is_err()
            {
                return Err(PkiError::InvalidKey);
            }
        }
    }
    if !found {
        return Err(PkiError::InvalidKey);
    }
    Ok(())
}

fn atomic_write(path: &Path, data: &[u8], mode: u32) -> Result<(), std::io::Error> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::Builder::new().tempfile_in(dir)?;

    // Set permissions before writing
    #[cfg(unix)]
    {
        let mut perms = temp.as_file().metadata()?.permissions();
        perms.set_mode(mode);
        temp.as_file_mut().set_permissions(perms)?;
    }

    temp.write_all(data)?;
    temp.flush()?;

    // Atomically persist
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_ca_generation_and_reuse() {
        let dir = tempdir().unwrap();
        let cert_path = dir.path().join("ca.crt");
        let key_path = dir.path().join("ca.key");

        // Generate initially
        ensure_ca(&cert_path, &key_path, "test-ca").unwrap();
        assert!(cert_path.exists());
        assert!(key_path.exists());
        #[cfg(unix)]
        {
            assert_eq!(
                fs::metadata(&key_path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&cert_path).unwrap().permissions().mode() & 0o777,
                0o644
            );
        }

        let initial_cert = fs::read(&cert_path).unwrap();

        // Ensure reuse
        ensure_ca(&cert_path, &key_path, "test-ca").unwrap();
        let reused_cert = fs::read(&cert_path).unwrap();
        assert_eq!(initial_cert, reused_cert);
    }

    #[test]
    fn test_sa_key_generation_and_reuse() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("sa.key");

        // Generate initially
        ensure_service_account_key(&key_path).unwrap();
        assert!(key_path.exists());
        #[cfg(unix)]
        {
            assert_eq!(
                fs::metadata(&key_path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        let initial_key = fs::read(&key_path).unwrap();

        // Ensure reuse
        ensure_service_account_key(&key_path).unwrap();
        let reused_key = fs::read(&key_path).unwrap();
        assert_eq!(initial_key, reused_key);
    }
}
