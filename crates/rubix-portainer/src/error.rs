use rubix_platform::Architecture;
use thiserror::Error;

/// Errors that can occur during Portainer Edge agent configuration and manifest generation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PortainerError {
    /// Portainer Edge agent is explicitly disabled.
    #[error("Portainer edge agent is disabled")]
    Disabled,

    /// Target architecture does not support the Portainer agent image (e.g. RISC-V 64).
    #[error("Portainer edge agent is not supported on architecture {0:?}")]
    UnsupportedArchitecture(Architecture),

    /// Missing both edge ID and edge key credentials.
    #[error("Missing Portainer edge credentials")]
    MissingCredentials,

    /// Only partial credentials provided (either `edge_id` or `edge_key` is missing).
    #[error(
        "Incomplete Portainer edge credentials: edge_id present={has_id}, edge_key present={has_key}"
    )]
    IncompleteCredentials {
        /// Whether `edge_id` was provided.
        has_id: bool,
        /// Whether `edge_key` was provided.
        has_key: bool,
    },

    /// The provided container image reference is invalid.
    #[error("Invalid image reference: {0}")]
    InvalidImage(String),

    /// Resource serialization failed.
    #[error("Resource serialization error: {0}")]
    Serialization(String),

    /// Kubernetes API error occurred.
    #[error("Kubernetes API error: {0}")]
    Api(String),
}
