//! Podman engine for [`EngineRuntimeAdapter`](crate::engine::EngineRuntimeAdapter).
//!
//! This module knows only the `podman` command line: how to spell `run`,
//! `ps`, `kill`, `rm`, `logs` and `exec`, and how to read `podman ps --format
//! json`. Pod semantics live in [`crate::engine`]. Every invocation goes
//! through [`PodmanEngine::podman`], and [`run_arguments`] is the single place
//! that builds the `run` command line.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use serde::Deserialize;

use crate::engine::{ContainerEngine, ContainerSpec, ContainerState, ContainerSummary, PullPolicy};
use crate::error::KubeletError;
use crate::workload::{ExecResult, LogOptions};

const ENGINE_NAME: &str = "podman";
const STOP_TIMEOUT_SECONDS: &str = "5";
const SEARCH_LOCATIONS: &[&str] = &[
    "/opt/podman/bin/podman",
    "/opt/homebrew/bin/podman",
    "/usr/local/bin/podman",
    "/usr/bin/podman",
];

/// Drives containers through the `podman` command-line client.
#[derive(Clone, Debug)]
pub struct PodmanEngine {
    binary: PathBuf,
    version: Option<String>,
}

impl PodmanEngine {
    #[must_use]
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            version: None,
        }
    }

    /// Records the client version reported by `podman --version`.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Finds `podman` on `PATH` or in the usual install locations and reads its version.
    #[must_use]
    pub fn detect() -> Option<Self> {
        let from_path = std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("podman"))
                .find(|candidate| candidate.is_file())
        });
        from_path
            .or_else(|| {
                SEARCH_LOCATIONS
                    .iter()
                    .map(PathBuf::from)
                    .find(|candidate| candidate.is_file())
            })
            .map(|binary| {
                let version = std::process::Command::new(&binary)
                    .arg("--version")
                    .output()
                    .ok()
                    .filter(|output| output.status.success())
                    .and_then(|output| parse_version(&String::from_utf8_lossy(&output.stdout)));
                let engine = Self::new(binary);
                match version {
                    Some(version) => engine.with_version(version),
                    None => engine,
                }
            })
    }

    #[must_use]
    pub fn binary(&self) -> &Path {
        &self.binary
    }

    fn unavailable(&self, e: &std::io::Error, what: &str) -> KubeletError {
        KubeletError::RuntimeUnavailable {
            endpoint: self.binary.display().to_string(),
            reason: format!("failed to execute podman {what}: {e}"),
        }
    }

    /// Runs one podman command and returns its output. Spawn failures mean the
    /// engine is unavailable; nonzero exits carry stderr.
    async fn podman(
        &self,
        args: &[String],
        context: &str,
    ) -> Result<std::process::Output, KubeletError> {
        let verb = args.first().map_or("", String::as_str);
        let output = tokio::process::Command::new(&self.binary)
            .args(args)
            .output()
            .await
            .map_err(|e| self.unavailable(&e, verb))?;
        if !output.status.success() {
            return Err(KubeletError::ContainerOperationFailed {
                container: context.to_string(),
                reason: format!(
                    "podman {verb} exited with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            });
        }
        Ok(output)
    }

    async fn podman_stdout(&self, args: &[String], context: &str) -> Result<String, KubeletError> {
        let output = self.podman(args, context).await?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[async_trait]
impl ContainerEngine for PodmanEngine {
    fn name(&self) -> &str {
        ENGINE_NAME
    }

    fn version(&self) -> Option<String> {
        self.version.clone()
    }

    async fn ping(&self) -> Result<(), KubeletError> {
        self.podman(
            &[
                "version".to_string(),
                "--format".to_string(),
                "{{.Client.Version}}".to_string(),
            ],
            "version",
        )
        .await
        .map(drop)
        .map_err(|e| KubeletError::RuntimeUnavailable {
            endpoint: self.binary.display().to_string(),
            reason: e.to_string(),
        })
    }

    async fn run_detached(&self, spec: &ContainerSpec) -> Result<String, KubeletError> {
        let args = run_arguments(spec)?;
        let stdout = self.podman_stdout(&args, &spec.name).await?;
        Ok(stdout.trim().to_string())
    }

    async fn list(
        &self,
        label_filters: &[(&str, &str)],
    ) -> Result<Vec<ContainerSummary>, KubeletError> {
        let mut args = vec!["ps".to_string(), "--all".to_string()];
        for (key, value) in label_filters {
            args.push("--filter".to_string());
            args.push(format!("label={key}={value}"));
        }
        args.push("--format".to_string());
        args.push("json".to_string());
        let stdout = self.podman_stdout(&args, "ps").await?;
        Ok(parse_ps_output(&stdout)?
            .into_iter()
            .map(PsEntry::into_summary)
            .collect())
    }

    async fn signal(&self, ids: &[String], signal: &str) -> Result<(), KubeletError> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut args = vec![
            "kill".to_string(),
            "--signal".to_string(),
            signal.to_string(),
        ];
        args.extend(ids.iter().cloned());
        self.podman(&args, &ids.join(",")).await.map(drop)
    }

    async fn remove(&self, ids: &[String]) -> Result<(), KubeletError> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut args = vec![
            "rm".to_string(),
            "--force".to_string(),
            "--time".to_string(),
            STOP_TIMEOUT_SECONDS.to_string(),
        ];
        args.extend(ids.iter().cloned());
        self.podman(&args, &ids.join(",")).await.map(drop)
    }

    /// `podman logs` replays the container's log in time order but writes each
    /// entry to its original stream. Both streams are joined onto one pipe so the
    /// order survives, as it does in a CRI log file.
    async fn logs(&self, id: &str, options: &LogOptions) -> Result<String, KubeletError> {
        let args = log_arguments(id, options);
        let (reader, writer) = std::io::pipe().map_err(|e| self.unavailable(&e, "logs"))?;
        let stderr_writer = writer
            .try_clone()
            .map_err(|e| self.unavailable(&e, "logs"))?;
        let mut child = tokio::process::Command::new(&self.binary)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(writer))
            .stderr(Stdio::from(stderr_writer))
            .spawn()
            .map_err(|e| self.unavailable(&e, "logs"))?;
        let collect = tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buffer = Vec::new();
            std::io::Read::read_to_end(&mut reader, &mut buffer).map(|_| buffer)
        });
        let status = child
            .wait()
            .await
            .map_err(|e| self.unavailable(&e, "logs"))?;
        let output = collect
            .await
            .map_err(|e| KubeletError::ContainerOperationFailed {
                container: id.to_string(),
                reason: format!("log reader task failed: {e}"),
            })?
            .map_err(|e| self.unavailable(&e, "logs"))?;
        let text = String::from_utf8_lossy(&output).into_owned();
        if !status.success() {
            return Err(KubeletError::ContainerOperationFailed {
                container: id.to_string(),
                reason: format!("podman logs exited with {status}: {}", text.trim()),
            });
        }
        Ok(text)
    }

    async fn exec(&self, id: &str, command: &[String]) -> Result<ExecResult, KubeletError> {
        let mut args = vec!["exec".to_string(), id.to_string()];
        args.extend(command.iter().cloned());
        let output = tokio::process::Command::new(&self.binary)
            .args(&args)
            .output()
            .await
            .map_err(|e| self.unavailable(&e, "exec"))?;
        Ok(ExecResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// Builds the `podman run` argument vector for one container spec.
pub fn run_arguments(spec: &ContainerSpec) -> Result<Vec<String>, KubeletError> {
    let pull = match spec.pull {
        PullPolicy::Missing => "missing",
        PullPolicy::Always => "always",
        PullPolicy::Never => "never",
    };
    let mut args = vec![
        "run".to_string(),
        "--detach".to_string(),
        "--name".to_string(),
        spec.name.clone(),
        "--pull".to_string(),
        pull.to_string(),
    ];
    for (key, value) in &spec.labels {
        args.push("--label".to_string());
        args.push(format!("{key}={value}"));
    }
    for (key, value) in &spec.env {
        args.push("--env".to_string());
        args.push(format!("{key}={value}"));
    }
    if let Some(dir) = &spec.working_dir {
        args.push("--workdir".to_string());
        args.push(dir.clone());
    }
    if let Some(entrypoint) = &spec.entrypoint {
        args.push("--entrypoint".to_string());
        args.push(serde_json::to_string(entrypoint)?);
    }
    args.push(spec.image.clone());
    args.extend(spec.args.iter().cloned());
    Ok(args)
}

/// Builds the `podman logs` argument vector.
pub fn log_arguments(id: &str, options: &LogOptions) -> Vec<String> {
    let mut args = vec!["logs".to_string()];
    if let Some(tail) = options.tail_lines {
        args.push("--tail".to_string());
        args.push(tail.to_string());
    }
    if options.timestamps {
        args.push("--timestamps".to_string());
    }
    if let Some(since) = options.since_seconds {
        args.push("--since".to_string());
        args.push(format!("{since}s"));
    }
    args.push(id.to_string());
    args
}

/// One row of `podman ps --format json`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct PsEntry {
    pub id: String,
    #[serde(default)]
    pub image: String,
    #[serde(default, rename = "ImageID")]
    pub image_id: String,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub exit_code: i32,
    #[serde(default)]
    pub started_at: i64,
    #[serde(default)]
    pub exited_at: i64,
}

impl PsEntry {
    fn into_summary(self) -> ContainerSummary {
        // Podman prints Go's zero time (negative) for timestamps it has not set.
        let started_at = u64::try_from(self.started_at).ok();
        let state = match self.state.as_str() {
            "running" => ContainerState::Running { started_at },
            "exited" | "stopped" => ContainerState::Exited {
                exit_code: self.exit_code,
                started_at,
                finished_at: u64::try_from(self.exited_at).ok(),
            },
            other => ContainerState::Idle(if other.is_empty() {
                "unknown".to_string()
            } else {
                other.to_string()
            }),
        };
        let image_id = if self.image_id.is_empty() || self.image_id.contains(':') {
            self.image_id
        } else {
            format!("sha256:{}", self.image_id)
        };
        ContainerSummary {
            id: self.id,
            image: self.image,
            image_id,
            labels: self.labels,
            state,
        }
    }
}

/// Parses `podman ps --format json`; an empty list prints `[]` or nothing.
pub fn parse_ps_output(stdout: &str) -> Result<Vec<PsEntry>, KubeletError> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(trimmed).map_err(|e| KubeletError::ContainerOperationFailed {
        container: "ps".to_string(),
        reason: format!("podman ps output is not JSON: {e}"),
    })
}

/// Extracts `6.0.2` from `podman version 6.0.2`.
fn parse_version(stdout: &str) -> Option<String> {
    stdout
        .split_whitespace()
        .last()
        .filter(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_arguments_spell_the_podman_command_line() {
        let spec = ContainerSpec {
            name: "k8s_hello_hello_default_u1_0".to_string(),
            image: "localhost/rubix-hello:latest".to_string(),
            pull: PullPolicy::Never,
            entrypoint: Some(vec!["/rubix-hello".to_string()]),
            args: vec!["600".to_string()],
            env: vec![("GREETING".to_string(), "hi".to_string())],
            working_dir: Some("/work".to_string()),
            labels: BTreeMap::from([("io.kubernetes.pod.uid".to_string(), "u1".to_string())]),
        };
        let args = run_arguments(&spec).unwrap();
        assert_eq!(
            args,
            [
                "run",
                "--detach",
                "--name",
                "k8s_hello_hello_default_u1_0",
                "--pull",
                "never",
                "--label",
                "io.kubernetes.pod.uid=u1",
                "--env",
                "GREETING=hi",
                "--workdir",
                "/work",
                "--entrypoint",
                "[\"/rubix-hello\"]",
                "localhost/rubix-hello:latest",
                "600"
            ]
        );
        let minimal = run_arguments(&ContainerSpec {
            name: "n".to_string(),
            image: "img".to_string(),
            ..ContainerSpec::default()
        })
        .unwrap();
        assert_eq!(
            minimal,
            ["run", "--detach", "--name", "n", "--pull", "missing", "img"]
        );
    }

    #[test]
    fn log_arguments_carry_tail_timestamps_and_since() {
        let options = LogOptions {
            tail_lines: Some(5),
            timestamps: true,
            since_seconds: Some(30),
        };
        assert_eq!(
            log_arguments("abc", &options),
            [
                "logs",
                "--tail",
                "5",
                "--timestamps",
                "--since",
                "30s",
                "abc"
            ]
        );
        assert_eq!(
            log_arguments("abc", &LogOptions::default()),
            ["logs", "abc"]
        );
    }

    #[test]
    fn ps_output_maps_running_and_exited_containers() {
        let stdout = r#"[
          {"Id":"abc123","Image":"localhost/rubix-hello:latest","ImageID":"e247","Labels":{"io.kubernetes.container.name":"hello","io.kubernetes.pod.uid":"u1"},"State":"running","ExitCode":0,"Exited":false,"StartedAt":1791146364,"ExitedAt":-62135596800},
          {"Id":"def456","Image":"img","ImageID":"sha256:i2","Labels":{"io.kubernetes.container.name":"side"},"State":"exited","ExitCode":3,"Exited":true,"StartedAt":1791146364,"ExitedAt":1791146370},
          {"Id":"ghi789","State":"created"}
        ]"#;
        let entries = parse_ps_output(stdout).unwrap();
        assert_eq!(entries.len(), 3);
        let running = entries[0].clone().into_summary();
        assert_eq!(running.id, "abc123");
        assert_eq!(running.image_id, "sha256:e247");
        assert_eq!(running.labels["io.kubernetes.container.name"], "hello");
        assert_eq!(
            running.state,
            ContainerState::Running {
                started_at: Some(1_791_146_364)
            }
        );
        let exited = entries[1].clone().into_summary();
        assert_eq!(exited.image_id, "sha256:i2");
        assert_eq!(
            exited.state,
            ContainerState::Exited {
                exit_code: 3,
                started_at: Some(1_791_146_364),
                finished_at: Some(1_791_146_370),
            }
        );
        assert_eq!(
            entries[2].clone().into_summary().state,
            ContainerState::Idle("created".to_string())
        );
        assert!(parse_ps_output("").unwrap().is_empty());
        assert!(parse_ps_output("[]").unwrap().is_empty());
        assert!(parse_ps_output("not json").is_err());
    }

    #[test]
    fn version_is_parsed_from_podman_output() {
        assert_eq!(
            parse_version("podman version 6.0.2\n").as_deref(),
            Some("6.0.2")
        );
        assert_eq!(parse_version(""), None);
        assert_eq!(PodmanEngine::new("/x/podman").version(), None);
        assert_eq!(
            PodmanEngine::new("/x/podman")
                .with_version("6.0.2")
                .version()
                .as_deref(),
            Some("6.0.2")
        );
        assert_eq!(PodmanEngine::new("/x/podman").name(), "podman");
    }
}
