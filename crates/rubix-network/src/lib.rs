//! Node networking inputs, IP/MTU resolution, resolv.conf handling and IPv4-only configuration.

pub mod discovery;
pub mod error;
pub mod ip;
pub mod ipv6;
pub mod model;
pub mod mtu;
pub mod resolver;

pub use discovery::{discover_host_interfaces, get_node_ip, get_node_mtu};
pub use error::NetworkError;
pub use ip::{
    classify_ipv4, get_local_ips, is_dns_name, is_global_unicast, is_ipv4_address, is_local_ip,
    is_valid_nameserver, resolve_load_balancer_ip, resolve_node_ip, select_node_ip,
};
pub use ipv6::{disable_ipv6_sysctls, disable_ipv6_sysctls_in_root};
pub use model::{
    DEFAULT_MTU, FALLBACK_NAMESERVERS, INSTANCE_METADATA_SERVICE_IP, InterfaceCandidate,
    MAX_NAMESERVERS, MAX_VALID_MTU, MIN_VALID_MTU,
};
pub use mtu::{resolve_mtu, select_mtu};
pub use resolver::{
    get_host_resolv_conf, get_host_resolv_conf_with_candidates, is_valid_resolv_conf,
    sanitize_resolv_conf,
};
