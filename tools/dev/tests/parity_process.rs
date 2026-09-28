//! Real owned subprocess regressions; no containers, VMs, or host configuration.
#[path = "../src/parity/process.rs"]
pub mod process;
use process::{Cancellation, CommandFailure, CommandRequest, Commands, OutputMode};
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

fn run(
    script: &str,
    limit: u64,
    timeout: Duration,
    cancel: Cancellation,
    input: &[u8],
) -> (
    tempfile::TempDir,
    std::result::Result<process::CommandResult, CommandFailure>,
) {
    let output = tempfile::tempdir().unwrap();
    let commands = Commands {
        output: output.path().to_owned(),
        cancellation: cancel,
    };
    let argv: Vec<OsString> = vec!["/bin/sh".into(), "-c".into(), script.into()];
    let result = commands.capture(CommandRequest {
        label: "case",
        argv: &argv,
        timeout,
        input,
        required: true,
        byte_limit: limit,
        mode: OutputMode::Merged,
        environment: None,
        current_directory: None,
        launcher: Some(Path::new(env!("CARGO_BIN_EXE_rubix-fixture"))),
    });
    (output, result)
}
#[test]
fn merged_output_order_and_empty_request_eof() {
    let (dir, result) = run(
        "cat; printf first; printf second >&2; printf third",
        4096,
        Duration::from_secs(5),
        Cancellation::default(),
        b"",
    );
    let result = result.unwrap();
    assert_eq!(result.stdout, "firstsecondthird");
    assert!(result.stderr.is_empty());
    assert_eq!(result.receipt["output_eof"], true);
    assert_eq!(result.receipt["owned_process_group_absent"], true);
    assert_eq!(
        std::fs::read(dir.path().join("case.log")).unwrap(),
        b"firstsecondthird"
    );
}
#[test]
fn input_is_delivered_without_inheriting_parent_stdin() {
    let (_dir, result) = run(
        "cat",
        4096,
        Duration::from_secs(5),
        Cancellation::default(),
        b"request\n",
    );
    assert_eq!(result.unwrap().stdout, "request\n");
}
#[test]
fn log_cap_does_not_limit_regular_artifact_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large");
    let script = format!(
        "dd if=/dev/zero of='{}' bs=1024 count=2048 2>/dev/null; printf ok",
        path.display()
    );
    let (_logs, result) = run(
        &script,
        64,
        Duration::from_secs(5),
        Cancellation::default(),
        b"",
    );
    assert_eq!(result.unwrap().stdout, "ok");
    assert_eq!(std::fs::metadata(path).unwrap().len(), 2 * 1024 * 1024);
}
#[test]
fn overflow_stops_and_reaps_owned_producer_with_bounded_log() {
    let (dir, result) = run(
        "exec yes flood",
        1024,
        Duration::from_secs(5),
        Cancellation::default(),
        b"",
    );
    let failure = result.unwrap_err();
    assert!(failure.cleanup_complete, "{failure:?}");
    assert_eq!(failure.receipt["output_limit"], true);
    assert_eq!(
        std::fs::metadata(dir.path().join("case.log"))
            .unwrap()
            .len(),
        1024
    );
    assert!(failure.owner.is_none());
}
#[test]
fn deadline_and_cancellation_settle_owned_process() {
    let (_dir, result) = run(
        "exec sleep 10",
        1024,
        Duration::from_millis(50),
        Cancellation::default(),
        b"",
    );
    let failure = result.unwrap_err();
    assert!(failure.cleanup_complete, "{failure:?}");
    assert_eq!(failure.receipt["timeout"], true);
    let cancel = Cancellation::default();
    let request = cancel.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        request.request();
    });
    let (_dir, result) = run("exec sleep 10", 1024, Duration::from_secs(5), cancel, b"");
    thread.join().unwrap();
    let failure = result.unwrap_err();
    assert!(failure.cleanup_complete, "{failure:?}");
    assert_eq!(failure.receipt["cancelled"], true);
}
#[test]
fn pre_cancelled_request_never_spawns_or_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("case.log"), b"prior").unwrap();
    let cancel = Cancellation::default();
    cancel.request();
    let commands = Commands {
        output: dir.path().into(),
        cancellation: cancel,
    };
    assert!(
        commands
            .run(
                "case",
                &["/bin/true".into()],
                Duration::from_secs(1),
                b"",
                true,
                1024
            )
            .is_err()
    );
    assert_eq!(
        std::fs::read(dir.path().join("case.log")).unwrap(),
        b"prior"
    );
}
#[test]
fn existing_log_is_not_overwritten_and_nonzero_is_settled() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("case.log"), b"prior").unwrap();
    let commands = Commands {
        output: dir.path().into(),
        cancellation: Cancellation::default(),
    };
    let argv = ["/bin/true".into()];
    let failure = commands
        .capture(CommandRequest {
            label: "case",
            argv: &argv,
            timeout: Duration::from_secs(1),
            input: b"",
            required: true,
            byte_limit: 10,
            mode: OutputMode::Merged,
            environment: None,
            current_directory: None,
            launcher: Some(Path::new(env!("CARGO_BIN_EXE_rubix-fixture"))),
        })
        .unwrap_err();
    assert!(failure.cleanup_complete);
    assert_eq!(failure.receipt["spawned"], false);
    assert_eq!(
        std::fs::read(dir.path().join("case.log")).unwrap(),
        b"prior"
    );
    let (_dir, result) = run(
        "printf diagnostic >&2; exit 7",
        1024,
        Duration::from_secs(5),
        Cancellation::default(),
        b"",
    );
    let failure = result.unwrap_err();
    assert!(failure.cleanup_complete);
    assert_eq!(failure.receipt["exit_code"], 7);
}
#[test]
fn inherited_writer_retains_owner_metadata_until_observed_settlement() {
    let (_dir, result) = run(
        "sleep 6 & printf ready",
        1024,
        Duration::from_secs(1),
        Cancellation::default(),
        b"",
    );
    let mut failure = result.unwrap_err();
    assert!(!failure.cleanup_complete);
    assert_eq!(failure.receipt["output_eof"], false);
    let metadata = std::path::PathBuf::from(failure.receipt["owner_directory"].as_str().unwrap());
    assert!(metadata.is_dir());
    let owner = failure.owner.as_mut().unwrap();
    assert_eq!(owner.observe().unwrap()["exit_code"], 0);
    std::thread::sleep(Duration::from_secs(2));
    let mut errors = Vec::new();
    owner.settle(&mut errors);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(!metadata.exists());
}
#[test]
fn raw_output_preserves_invalid_utf8_exactly() {
    let (_dir, result) = run(
        "printf '\\377'",
        64,
        Duration::from_secs(5),
        Cancellation::default(),
        b"",
    );
    let result = result.unwrap();
    assert_eq!(result.stdout_bytes, [255]);
    assert!(String::from_utf8(result.stdout_bytes).is_err());
    assert!(result.stderr_bytes.is_empty());
}
#[test]
fn cancellation_during_exit_drain_is_retained() {
    let cancel = Cancellation::default();
    let signal = cancel.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        signal.request();
    });
    let (_dir, result) = run(
        "sleep 1 & printf ready",
        1024,
        Duration::from_secs(5),
        cancel,
        b"",
    );
    thread.join().unwrap();
    let mut failure = result.unwrap_err();
    assert_eq!(failure.receipt["cancelled"], true);
    if let Some(owner) = failure.owner.as_mut() {
        std::thread::sleep(Duration::from_millis(100));
        let mut errors = Vec::new();
        owner.settle(&mut errors);
        assert!(errors.is_empty(), "{errors:?}");
    }
}
#[test]
fn existing_fifo_receipt_fails_without_blocking_or_spawning() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("case.command.json");
    let (_setup, result) = run(
        &format!("mkfifo '{}'", fifo.display()),
        1024,
        Duration::from_secs(5),
        Cancellation::default(),
        b"",
    );
    result.unwrap();
    let commands = Commands {
        output: dir.path().into(),
        cancellation: Cancellation::default(),
    };
    let argv = ["/bin/true".into()];
    let began = std::time::Instant::now();
    let failure = commands
        .capture(CommandRequest {
            label: "case",
            argv: &argv,
            timeout: Duration::from_secs(1),
            input: b"",
            required: true,
            byte_limit: 1024,
            mode: OutputMode::Merged,
            environment: None,
            current_directory: None,
            launcher: Some(Path::new(env!("CARGO_BIN_EXE_rubix-fixture"))),
        })
        .unwrap_err();
    assert!(began.elapsed() < Duration::from_secs(1));
    assert!(failure.cleanup_complete);
    assert_eq!(failure.receipt["spawned"], false);
}
#[test]
fn unix_signal_guard_latches_while_synchronous_command_is_owned() {
    let cancellation = Cancellation::default();
    let _guard = process::SignalGuard::install(cancellation.clone()).unwrap();
    let thread = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(150));
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::TERM)
            .unwrap();
    });
    let (_dir, result) = run(
        "exec sleep 10",
        1024,
        Duration::from_secs(5),
        cancellation,
        b"",
    );
    thread.join().unwrap();
    let failure = result.unwrap_err();
    assert_eq!(failure.receipt["cancelled"], true);
    assert!(failure.cleanup_complete, "{failure:?}");
}
