use rubix_config::{ConfigError, DecodedConfig, HostContext};
use rubix_kube::{StartupAction, StartupInputs};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

#[derive(Default)]
struct Inputs {
    files: BTreeMap<String, String>,
    reads: usize,
    probes: usize,
}
impl StartupInputs for Inputs {
    fn read_config(&mut self, path: &Path) -> Result<Option<DecodedConfig>, ConfigError> {
        self.reads += 1;
        if path == Path::new("/tmp") {
            return Err(ConfigError {
                kind: rubix_config::ErrorKind::Io,
                path: "/tmp".into(),
                message: "is a directory".into(),
            });
        }
        self.files
            .get(&path.display().to_string())
            .map(|text| rubix_config::decode(text))
            .transpose()
    }
    fn host(&mut self) -> io::Result<HostContext> {
        self.probes += 1;
        Ok(HostContext {
            cpu_count: 4,
            architecture: "arm64".into(),
            detected_container_mode: true,
        })
    }
}
fn assert_output(case: &Value, stdout: &str, stderr: &str) {
    let combined = format!("{stdout}{stderr}");
    for (name, text) in [
        ("stdout", stdout),
        ("stderr", stderr),
        ("combined", combined.as_str()),
    ] {
        if let Some(expected) = case["expect"][format!("{name}_equals")].as_str() {
            assert_eq!(text, expected, "{} {name}", case["id"]);
        }
        if let Some(markers) = case["expect"][format!("{name}_contains")].as_array() {
            for marker in markers {
                assert!(
                    text.contains(marker.as_str().expect("fixture marker")),
                    "{} {name}: missing {marker} in {text}",
                    case["id"]
                );
            }
        }
    }
}
fn replay(suite: &str) {
    let suite: Value = serde_json::from_str(suite).expect("reviewed fixture JSON");
    for case in suite["cases"].as_array().expect("fixture cases") {
        let mut inputs = Inputs::default();
        if let Some(files) = case["files"].as_object() {
            for (name, text) in files {
                inputs.files.insert(
                    format!("{{fixture:{name}}}"),
                    text.as_str().expect("file text").into(),
                );
            }
        }
        let args: Vec<_> = case["argv"]
            .as_array()
            .expect("argv")
            .iter()
            .map(|v| v.as_str().expect("argument").into())
            .collect();
        let env: BTreeMap<_, _> = case["env"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(key, v)| (key.clone(), v.as_str().expect("environment").into()))
            .collect();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result = rubix_kube::execute(
            &args,
            &env,
            "fixture-version",
            &mut inputs,
            &mut out,
            &mut err,
        )
        .expect("memory outputs");
        let StartupAction::Exit(code) = result else {
            panic!("startup reached by fixture {}", case["id"]);
        };
        assert_eq!(
            u64::from(code),
            case["expect"]["exit_code"].as_u64().expect("exit"),
            "{}: {}",
            case["id"],
            String::from_utf8_lossy(&err)
        );
        assert_output(
            case,
            &String::from_utf8(out).expect("UTF8 stdout"),
            &String::from_utf8(err).expect("UTF8 stderr"),
        );
    }
}
#[test]
fn matches_real_startup_parser_cases() {
    replay(include_str!("fixtures/startup.json"));
}
#[test]
fn matches_configuration_command_cases() {
    replay(include_str!(
        "../../../tools/parity/fixtures/config-command.json"
    ));
}
#[test]
fn version_and_help_never_read_file_or_probe_host() {
    for command in ["--version", "--help"] {
        let mut inputs = Inputs::default();
        let action = rubix_kube::execute(
            &[command.into(), "--config=/tmp".into()],
            &BTreeMap::new(),
            "test",
            &mut inputs,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect("output");
        assert!(matches!(action, StartupAction::Exit(0)));
        assert_eq!((inputs.reads, inputs.probes), (0, 0));
    }
}
#[test]
fn print_exits_but_normal_resolution_returns_start_action() {
    for (args, printing) in [(vec!["--print-config".into()], true), (Vec::new(), false)] {
        let mut inputs = Inputs::default();
        let action = rubix_kube::execute(
            &args,
            &BTreeMap::new(),
            "test",
            &mut inputs,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect("output");
        assert_eq!(matches!(action, StartupAction::Exit(0)), printing);
        assert_eq!((inputs.reads, inputs.probes), (1, 1));
    }
}
struct BrokenWriter;
impl io::Write for BrokenWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn output_errors_are_propagated() {
    let error = rubix_kube::execute(
        &["--print-config".into()],
        &BTreeMap::new(),
        "test",
        &mut Inputs::default(),
        &mut BrokenWriter,
        &mut Vec::new(),
    )
    .expect_err("broken stdout");
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
}
#[test]
fn actual_executable_version_bypasses_invalid_environment() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rubix-kube"))
        .args(["--version", "--config=/tmp"])
        .env_clear()
        .env("KUBESOLO_MTU", "invalid")
        .output()
        .expect("version process");
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("version"));
}

#[test]
fn startup_action_debug_does_not_disclose_resolved_values() {
    let action = rubix_kube::execute(
        &["--portainer-edge-key=private-edge-credential".into()],
        &BTreeMap::new(),
        "test",
        &mut Inputs::default(),
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect("resolution");
    assert!(matches!(action, StartupAction::Start(_)));
    assert_eq!(format!("{action:?}"), "Start(<resolved configuration>)");
}

#[test]
fn actual_executable_prints_then_refuses_unimplemented_startup() {
    for (printing, expected_success) in [(true, true), (false, false)] {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_rubix-kube"));
        command.env_clear().arg("--config=");
        if printing {
            command.arg("--print-config");
        }
        let output = command.output().expect("startup process");
        assert_eq!(output.status.success(), expected_success);
        if printing {
            assert!(
                String::from_utf8_lossy(&output.stdout)
                    .contains("apiVersion: kubesolo.io/v1alpha1")
            );
            assert!(!String::from_utf8_lossy(&output.stderr).contains("runtime startup"));
        } else {
            assert!(output.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("runtime startup is not implemented")
            );
        }
    }
}
