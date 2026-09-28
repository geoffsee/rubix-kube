//! Exercise finite output through the actual binary and shared owned launcher.
use rubix_dev::process::{Cancellation, CommandRequest, Commands, OutputMode};
use std::{path::Path, time::Duration};

#[test]
fn supervisor_helper_preserves_raw_stdout_stderr_write_order() -> rubix_dev::Result<()> {
    let directory = tempfile::tempdir()?;
    let binary = Path::new(env!("CARGO_BIN_EXE_rubix-supervisor-fixture"));
    let commands = Commands {
        output: directory.path().to_owned(),
        cancellation: Cancellation::default(),
    };
    let result = commands.capture(CommandRequest {
        label: "merged",
        argv: &[
            binary.as_os_str().to_owned(),
            "output-child".into(),
            "merged".into(),
            directory.path().as_os_str().to_owned(),
        ],
        timeout: Duration::from_secs(5),
        input: b"",
        required: true,
        byte_limit: 64,
        mode: OutputMode::Merged,
        environment: None,
        current_directory: None,
        launcher: Some(binary),
    })?;
    assert_eq!(result.stdout_bytes, b"out\xfferr\0");
    assert_eq!(
        std::fs::read(directory.path().join("merged.log"))?,
        b"out\xfferr\0"
    );
    for field in [
        "cleanup_complete",
        "owned_process_group_absent",
        "output_eof",
    ] {
        assert_eq!(result.receipt[field], true);
    }
    Ok(())
}
