use thiserror::Error;

/// Storage-related errors.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Failed to serialize or deserialize JSON manifest payload.
    #[error("failed to serialize manifest: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Invalid storage configuration parameter.
    #[error("invalid storage configuration: {0}")]
    InvalidConfiguration(String),

    /// Manifest generation error.
    #[error("manifest error: {0}")]
    Manifest(String),
}
