//! Platform coverage, candidate digest verification, and sustained-soak qualification.
//!
//! Provides exhaustive mapping of the accepted platform, runtime, variant, and container
//! matrix (Issue #120 / Gate C14 / E28.03), candidate digest matching against release
//! manifests, 24-hour sustained soak memory growth verification, clean/crash restart
//! bound checks, and historical/per-epic regression tracking.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::similar_names
)]

pub mod candidate;
pub mod matrix;
pub mod regressions;
pub mod report;
pub mod restart;
pub mod runner;
pub mod soak;

pub use candidate::{CandidateArtifactRecord, CandidateVerificationSummary, ObservedArtifact};
pub use matrix::{
    DimensionCategory, EnvironmentMapping, EnvironmentRecord, EnvironmentResult,
    MatrixCompleteness, SupportStatus,
};
pub use regressions::{EpicRegressionRecord, HistoricalRegression, RegressionSuite};
pub use report::{PlatformSoakError, PlatformSoakReport};
pub use restart::{RestartCase, RestartSummary};
pub use runner::PlatformSoakRunner;
pub use soak::{SustainedSoakRecord, SustainedSoakSummary};
