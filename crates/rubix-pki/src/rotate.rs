use rcgen::CertificateParams;
use std::collections::HashSet;
use std::net::IpAddr;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use x509_parser::extensions::{GeneralName, ParsedExtension};
use x509_parser::prelude::*;

use crate::PkiError;
use crate::leaf::issue_leaf_certificate;

pub fn validate_pki_dir(dir: &Path) -> Result<(), PkiError> {
    let s = dir.to_string_lossy();
    if s.is_empty() || s == "." || s == "/" {
        return Err(PkiError::UnsafePath(format!("unsafe pki directory: {s}")));
    }
    if dir.is_symlink() {
        return Err(PkiError::UnsafePath(
            "symlink pki directory is rejected".into(),
        ));
    }
    if let Ok(canonical) = dir.canonicalize() {
        if canonical == Path::new("/") {
            return Err(PkiError::UnsafePath("canonical root / is rejected".into()));
        }
    }
    Ok(())
}

pub fn inspect_leaf(
    cert_path: &Path,
    key_path: &Path,
    expected_cn: &str,
    expected_dns: &[String],
    expected_ips: &[IpAddr],
) -> Result<bool, PkiError> {
    if !cert_path.exists() || !key_path.exists() {
        return Ok(false);
    }

    // Check key validity
    let key_bytes = match std::fs::read(key_path) {
        Ok(b) => b,
        Err(_) => return Ok(false),
    };
    let key_str = match std::str::from_utf8(&key_bytes) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };
    if ::pem::parse(key_str).is_err() {
        return Ok(false);
    }

    // Check cert validity
    let cert_bytes = match std::fs::read(cert_path) {
        Ok(b) => b,
        Err(_) => return Ok(false),
    };
    let cert_str = match std::str::from_utf8(&cert_bytes) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };
    let parsed_pem = match ::pem::parse(cert_str) {
        Ok(p) => p,
        Err(_) => return Ok(false),
    };
    let (_, cert) = match X509Certificate::from_der(parsed_pem.contents()) {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };

    // Check expiration: not_before <= now <= not_after
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
        .try_into()
        .unwrap_or(0i64);
    let not_after = cert.validity().not_after.timestamp();
    let not_before = cert.validity().not_before.timestamp();
    if now < not_before || now >= not_after {
        return Ok(false);
    }

    // Check CN if present in expected
    if !expected_cn.is_empty() {
        let mut cn_matched = false;
        for rdn in cert.subject().iter_rdn() {
            for attr in rdn.iter() {
                if attr.attr_type() == &x509_parser::oid_registry::OID_X509_COMMON_NAME {
                    if let Ok(cn) = attr.as_str() {
                        if cn == expected_cn {
                            cn_matched = true;
                        }
                    }
                }
            }
        }
        if !cn_matched {
            return Ok(false);
        }
    }

    // Check SANs
    let mut found_dns: HashSet<String> = HashSet::new();
    let mut found_ips: HashSet<IpAddr> = HashSet::new();

    for ext in cert.iter_extensions() {
        if let ParsedExtension::SubjectAlternativeName(san) = ext.parsed_extension() {
            for gn in &san.general_names {
                match gn {
                    GeneralName::DNSName(name) => {
                        found_dns.insert((*name).to_string());
                    },
                    GeneralName::IPAddress(ip_bytes) => {
                        if ip_bytes.len() == 4 {
                            found_ips.insert(IpAddr::V4(std::net::Ipv4Addr::new(
                                ip_bytes[0],
                                ip_bytes[1],
                                ip_bytes[2],
                                ip_bytes[3],
                            )));
                        } else if ip_bytes.len() == 16 {
                            let mut octets = [0u8; 16];
                            octets.copy_from_slice(ip_bytes);
                            found_ips.insert(IpAddr::V6(std::net::Ipv6Addr::from(octets)));
                        }
                    },
                    _ => {},
                }
            }
        }
    }

    for dns in expected_dns {
        if !found_dns.contains(dns) {
            return Ok(false);
        }
    }

    for ip in expected_ips {
        if !found_ips.contains(ip) {
            return Ok(false);
        }
    }

    Ok(true)
}

pub fn rotate_leaf_if_needed(
    cert_path: &Path,
    key_path: &Path,
    signer_cert_path: &Path,
    signer_key_path: &Path,
    params: &CertificateParams,
    expected_cn: &str,
    expected_dns: &[String],
    expected_ips: &[IpAddr],
) -> Result<bool, PkiError> {
    if inspect_leaf(cert_path, key_path, expected_cn, expected_dns, expected_ips)? {
        return Ok(false);
    }

    issue_leaf_certificate(
        cert_path,
        key_path,
        signer_cert_path,
        signer_key_path,
        params,
    )?;
    Ok(true)
}
