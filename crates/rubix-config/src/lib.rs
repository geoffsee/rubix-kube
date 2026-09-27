//! KubeSolo-compatible versioned configuration decoding without host side effects.
mod model;
pub use model::*;
mod decode;
pub use decode::{
    ConfigError, DecodeLimits, DecodedConfig, ErrorKind, Presence, Warning, decode, decode_bytes,
    decode_with_limits, read_file,
};
mod validate;
pub use validate::{
    HostContext, ValidatedConfig, ValidationWarning, normalize_image, resolve_runtime_endpoint,
};
mod runtime;
pub use runtime::{RuntimeEndpoint, RuntimePath, RuntimeProbe, RuntimeSettings};
mod inventory;
pub use inventory::{FIELDS, FieldSpec, FieldType};
