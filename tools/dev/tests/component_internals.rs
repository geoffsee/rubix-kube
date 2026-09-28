//! Compile private component interfaces with an explicit Cargo-provided launcher.
pub use rubix_dev::{Error, Result, json, read_bounded, repository_root, resolved_capture, sha256};
#[path = "../src/api_json/mod.rs"]
pub mod api_json;
#[path = "../src/component_boundary/mod.rs"]
pub mod component_boundary;
#[path = "../src/defaults/mod.rs"]
pub mod defaults;
#[path = "../src/parity/process.rs"]
pub mod process;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
#[test]
fn persistent_component_clean_shutdown_reaps_owned_group() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut runtime = component_boundary::runtime::Runtime::testing(
        directory.path(),
        Path::new(env!("CARGO_BIN_EXE_rubix-api-json")),
    )?;
    let marker = directory.path().join("ready");
    runtime.start(
        "sh",
        &[
            "-c",
            "trap 'exit 0' TERM; : > \"$1\"; while :; do sleep 1; done",
            "fixture",
            marker.to_str().ok_or("test path")?,
        ],
        "one",
    )?;
    let ready = (|| -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !marker.exists() {
            runtime.alive()?;
            if Instant::now() >= deadline {
                return Err("fixture ready deadline".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    })();
    let stopped = runtime.shutdown();
    ready?;
    stopped?;
    assert!(!runtime.owners_remain());
    assert_eq!(runtime.report["shutdowns"][0]["exit_code"], 0);
    assert_eq!(runtime.report["shutdowns"][0]["forced"], false);
    assert_eq!(
        runtime.report["shutdowns"][0]["owned_group_remained"],
        false
    );
    assert!(fs::metadata(directory.path().join("sh-one.log"))?.len() < 65536);
    Ok(())
}
#[test]
fn early_component_failure_is_retained_as_failed_shutdown() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut runtime = component_boundary::runtime::Runtime::testing(
        directory.path(),
        Path::new(env!("CARGO_BIN_EXE_rubix-api-json")),
    )?;
    runtime.start("sh", &["-c", "printf failed; exit 9"], "early")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while runtime.alive().is_ok() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = runtime.shutdown();
    assert!(result.is_err());
    assert_eq!(runtime.report["shutdowns"][0]["exit_code"], 9);
    assert!(!runtime.owners_remain());
    assert_eq!(fs::read(directory.path().join("sh-early.log"))?, b"failed");
    Ok(())
}
