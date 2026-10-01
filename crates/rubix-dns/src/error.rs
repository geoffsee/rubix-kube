use thiserror::Error;

/// Errors produced during `CoreDNS` manifest generation and reconciliation.
#[derive(Debug, Error)]
pub enum DnsError {
    #[error("failed to serialize resource to JSON: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("invalid DNS configuration: {0}")]
    InvalidConfig(String),

    #[error("manifest validation error: {0}")]
    Validation(String),

    #[error("kubernetes API error: {0}")]
    Api(String),
}

pub type Result<T> = std::result::Result<T, DnsError>;
