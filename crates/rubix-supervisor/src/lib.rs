//! Dependency-aware supervision of cooperative async adapters.
//! OS process ownership and executable signal handling are separate adapters.
mod coordinator;
mod model;
pub use coordinator::Supervisor;
pub use model::*;
