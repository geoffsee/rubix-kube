//! Exercise the actual executable's internal launcher without Docker or chroot.
use rubix_dev::process::{Cancellation, CommandRequest, Commands, OutputMode};
use std::{path::Path, time::Duration};
#[test]
fn prerequisite_binary_preserves_launcher_protocol_and_reaps_child() -> rubix_dev::Result<()> {
    let directory = tempfile::tempdir()?;
    let commands = Commands {
        output: directory.path().to_owned(),
        cancellation: Cancellation::default(),
    };
    let result = commands.capture(CommandRequest {
        label: "launcher",
        argv: &["/bin/sh".into(), "-c".into(), "cat; printf verified".into()],
        timeout: Duration::from_secs(5),
        input: b"request-",
        required: true,
        byte_limit: 4096,
        mode: OutputMode::Merged,
        environment: None,
        current_directory: None,
        launcher: Some(Path::new(env!("CARGO_BIN_EXE_rubix-prerequisite-fixture"))),
    })?;
    assert_eq!(result.stdout_bytes, b"request-verified");
    assert_eq!(result.receipt["cleanup_complete"], true);
    assert_eq!(result.receipt["owned_process_group_absent"], true);
    assert_eq!(result.receipt["output_eof"], true);
    Ok(())
}
