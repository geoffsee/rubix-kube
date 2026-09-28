//! Only the marked file-view helper runs, against private temporary paths; no mount/namespace effects.
#[path = "../src/parity/process.rs"]
pub mod process;
use process::{Cancellation, CommandRequest, Commands, OutputMode};
use rubix_dev::Result;
use std::{ffi::OsString, fs, os::unix::fs::symlink, path::Path, time::Duration};

fn run_view(
    source: &Path,
    reference: &Path,
    view: &Path,
    tool: &str,
    double: &Path,
) -> Result<bool> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw = fs::read_to_string(root.join("tools/node-constrained/namespace.sh"))?;
    let begin = "# MAKE_TOOL_VIEW_BEGIN\n";
    let end = "# MAKE_TOOL_VIEW_END";
    assert_eq!(raw.matches(begin).count(), 1);
    assert_eq!(raw.matches(end).count(), 1);
    let function = raw
        .split_once(begin)
        .ok_or("view begin")?
        .1
        .split_once(end)
        .ok_or("view end")?
        .0;
    let script =
        format!("set -eu\nulimit -f 1024\nulimit -c 0\n{function}\nmake_tool_view \"$@\"\n");
    let output = tempfile::tempdir()?;
    let argv: Vec<OsString> = vec![
        "/bin/sh".into(),
        "-c".into(),
        script.into(),
        "view-test".into(),
        source.into(),
        reference.into(),
        view.into(),
        tool.into(),
        double.into(),
    ];
    let outcome = Commands {
        output: output.path().into(),
        cancellation: Cancellation::default(),
    }
    .capture(CommandRequest {
        label: "view",
        argv: &argv,
        timeout: Duration::from_secs(10),
        input: b"",
        required: false,
        byte_limit: 1024 * 1024,
        mode: OutputMode::Merged,
        environment: None,
        current_directory: None,
        launcher: Some(Path::new(env!("CARGO_BIN_EXE_rubix-node-fixture"))),
    })?;
    assert_eq!(outcome.receipt["cleanup_complete"], true);
    assert_eq!(outcome.receipt["owned_process_group_absent"], true);
    assert_eq!(outcome.receipt["timeout"], false);
    Ok(outcome.receipt["exit_code"] == 0)
}
#[test]
fn large_executable_and_relative_aliases_preserved_under_file_limit() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().canonicalize()?;
    let source = root.join("usr/sbin");
    let external = root.join("usr/bin");
    fs::create_dir_all(&source)?;
    fs::create_dir_all(&external)?;
    let payload = vec![b'x'; 2 * 1024 * 1024];
    fs::write(source.join("large"), &payload)?;
    fs::write(external.join("outside"), &payload)?;
    symlink("large", source.join("inside-alias"))?;
    symlink("../bin/outside", source.join("outside-alias"))?;
    symlink("large", source.join("iptables"))?;
    let reference = root.join("original");
    symlink(&source, &reference)?;
    let double = root.join("double");
    let contents = b"#!/bin/sh\nexit 73\n";
    fs::write(&double, contents)?;
    let view = root.join("view");
    assert!(run_view(&source, &reference, &view, "iptables", &double)?);
    for name in ["large", "inside-alias"] {
        assert_eq!(fs::read_link(view.join(name))?, reference.join("large"));
    }
    assert_eq!(
        fs::read_link(view.join("outside-alias"))?,
        external.join("outside")
    );
    for name in ["inside-alias", "outside-alias"] {
        assert_eq!(fs::read(view.join(name))?, payload);
    }
    assert!(
        !fs::symlink_metadata(view.join("iptables"))?
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(view.join("iptables"))?, contents);
    assert_eq!(fs::read(source.join("large"))?, payload);
    assert!(
        fs::symlink_metadata(source.join("iptables"))?
            .file_type()
            .is_symlink()
    );
    Ok(())
}
#[test]
fn view_rejects_unbounded_nonregular_or_unapproved_inputs() -> Result<()> {
    for scenario in ["too-many", "dangling", "directory", "tool", "double"] {
        let dir = tempfile::tempdir()?;
        let root = dir.path().canonicalize()?;
        let source = root.join("sbin");
        fs::create_dir(&source)?;
        let reference = root.join("original");
        symlink(&source, &reference)?;
        fs::write(source.join("tool"), b"original")?;
        let double = root.join("double");
        fs::write(&double, b"#!/bin/sh\nexit 1\n")?;
        match scenario {
            "too-many" => {
                for index in 0..256 {
                    fs::write(source.join(index.to_string()), b"")?;
                }
            },
            "dangling" => symlink("missing", source.join("broken"))?,
            "directory" => fs::create_dir(source.join("nested"))?,
            "double" => fs::write(&double, vec![b'x'; 4097])?,
            _ => {},
        }
        assert!(
            !run_view(
                &source,
                &reference,
                &root.join("view"),
                if scenario == "tool" {
                    "unapproved"
                } else {
                    "modprobe"
                },
                &double
            )?,
            "{scenario}"
        );
        assert_eq!(fs::read(source.join("tool"))?, b"original");
    }
    Ok(())
}
