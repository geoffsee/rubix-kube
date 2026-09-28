//! Private maintenance behavior tested with Cargo's explicit subprocess launcher.
pub use rubix_dev::{
    Error, Result, defaults, json, process, read_bounded, repository_root, resolved_report, sha256,
};
#[path = "../src/drift/mod.rs"]
pub mod drift;
#[path = "../src/upstream/mod.rs"]
pub mod upstream;
#[test]
fn command_parsers_reject_unknown_commands() {
    assert!(upstream::cli(&["invalid".into()]).is_err());
    assert!(drift::cli(&["invalid".into()]).is_err());
}
