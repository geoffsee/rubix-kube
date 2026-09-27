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

mod layers;
pub use layers::{
    EnvironmentMode, ExplicitFlags, INPUT_BINDINGS, InputBinding, ParsedEnvironment,
    ResolutionWarning, ResolvedConfig, parse_environment, parse_setting, resolve_layers,
    resolve_parsed_layers,
};

mod render;
pub use render::render_effective_yaml;

mod persistence;
pub use persistence::{
    CleanupFailure, PersistenceError, PersistenceSource, PersistenceStage, WriteOutcome,
    write_document,
};

mod describe;
pub use describe::{SettingDescriptor, describe_settings};
