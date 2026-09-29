use rcgen::{CertificateParams, KeyUsagePurpose, ExtendedKeyUsagePurpose, SanType, DnType};
use std::net::IpAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    Apiserver,
    Kubelet,
    ControllerManager,
    Admin,
    Webhook,
    RequestHeaderClient,
    D2kServer,
    D2kClient,
}

pub fn profile(component: Component, extra_sans: &[String]) -> CertificateParams {
    let mut params = match component {
        Component::Apiserver => {
            let mut params = CertificateParams::new(vec!["kube-apiserver".to_string()]).unwrap();
            params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth, ExtendedKeyUsagePurpose::ClientAuth];
            params.distinguished_name.push(DnType::OrganizationName, "Kubernetes");
            params
        }
        Component::Kubelet => {
            let mut params = CertificateParams::new(vec!["system:node:fixture-node".to_string()]).unwrap();
            params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth, ExtendedKeyUsagePurpose::ClientAuth];
            params.distinguished_name.push(DnType::OrganizationName, "system:nodes");
            params
        }
        Component::ControllerManager => {
            let mut params = CertificateParams::new(vec!["system:kube-controller-manager".to_string()]).unwrap();
            params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
            params.distinguished_name.push(DnType::OrganizationName, "system:kube-controller-manager");
            params
        }
        Component::Admin => {
            let mut params = CertificateParams::new(vec!["kubesolo-admin".to_string()]).unwrap();
            params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth, ExtendedKeyUsagePurpose::ClientAuth];
            params.distinguished_name.push(DnType::OrganizationName, "system:masters");
            params
        }
        Component::Webhook => {
            let mut params = CertificateParams::new(vec!["kubesolo-webhook".to_string()]).unwrap();
            params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
            params.distinguished_name.push(DnType::OrganizationName, "system:masters");
            params
        }
        Component::RequestHeaderClient => {
            let mut params = CertificateParams::new(vec!["system:auth-proxy".to_string()]).unwrap();
            params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
            params.distinguished_name.push(DnType::OrganizationName, "system:auth-proxy");
            params
        }
        Component::D2kServer => {
            let mut params = CertificateParams::new(vec!["d2k".to_string()]).unwrap();
            params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
            params.distinguished_name.push(DnType::OrganizationName, "kubesolo");
            params
        }
        Component::D2kClient => {
            let mut params = CertificateParams::new(vec!["d2k-client".to_string()]).unwrap();
            params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
            params.distinguished_name.push(DnType::OrganizationName, "kubesolo");
            params
        }
    };

    for san in extra_sans {
        let san = san.trim();
        if san.is_empty() {
            continue;
        }

        if let Ok(ip) = san.parse::<std::net::Ipv4Addr>() {
            params.subject_alt_names.push(SanType::IpAddress(IpAddr::V4(ip)));
        } else if is_dns_name(san) {
            params.subject_alt_names.push(SanType::DnsName(san.try_into().unwrap()));
        }
    }

    params
}

fn is_dns_name(s: &str) -> bool {
    if s.is_empty() || s.len() > 255 {
        return false;
    }
    let mut last_was_dot = true;
    for c in s.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' => last_was_dot = false,
            '-' => {
                if last_was_dot {
                    return false;
                }
            }
            '.' => {
                if last_was_dot {
                    return false;
                }
                last_was_dot = true;
            }
            _ => return false,
        }
    }
    !last_was_dot
}
