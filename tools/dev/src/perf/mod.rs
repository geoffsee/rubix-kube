//! Matched Go and Rust performance baseline measurement and evaluation harness.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::similar_names
)]

pub mod contract;
pub mod harness;
pub mod metrics;
pub mod report;
pub mod secondary;

pub use contract::{GateEvaluationReport, GateResult};
pub use harness::{
    REQUIRED_RETAINED_PROCESSES, RunOrder, measure_artifact_footprint, parse_pss_bytes,
    parse_vm_rss_bytes, sum_process_memory, synthesize_summary, verify_retained_process_coverage,
};
pub use metrics::{
    Architecture, ArtifactFootprint, ComponentVersions, HardwareInfo, IdleFootprint,
    ImplementationKind, PerformanceReport, PodDensity, ProcessMemoryBreakdown, ShutdownMeasurement,
    StartupLatencies, SustainedGrowth, VarianceSummary, WorkloadSpec,
};
pub use report::{generate_markdown_report, load_report, save_report};
pub use secondary::{SecondaryArchitectureGap, SecondaryTargetsRegistry};
