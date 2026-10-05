//! Podman engine for [`OciRuntimeAdapter`](crate::oci::OciRuntimeAdapter).
//!
//! This module knows only the `podman` command line: how to spell `run`,
//! `ps`, `rm`, `logs` and `exec`, and how to read `podman ps --format json`.
//! Pod semantics live in [`crate::oci`]. Every invocation goes through
//! [`PodmanEngine::podman`], and [`run_arguments`] is the single place that
//! builds the `run` command line.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Deserialize;

use crate::error::KubeletError;
use crate::oci::{ContainerSpec, ContainerState, ContainerSummary, OciEngine, PullPolicy};
use crate::workload::ExecResult;

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

    /// Runs one podman command and returns its output. Spawn failures mean the
    /// engine is unavailable; nonzero exits carry stderr.
    async fn podman(
        &self,
        args: &[String],
        context: &str,
    ) -> Result<std::process::Output, KubeletError> {
        let output = tokio::process::Command::new(&self.binary)
            .args(args)
            .output()
            .await
            .map_err(|e| KubeletError::RuntimeUnavailable {
                endpoint: self.binary.display().to_string(),
                reason: format!("failed to execute podman: {e}"),
            })?;
        if !output.status.success() {
            return Err(KubeletError::ContainerOperationFailed {
                container: context.to_string(),
                reason: format!(
                    "podman {} exited with {}: {}",
                    args.first().map_or("", String::as_str),
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
impl OciEngine for PodmanEngine {
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

    async fn logs(&self, id: &str, tail_lines: Option<usize>) -> Result<String, KubeletError> {
        let mut args = vec!["logs".to_string()];
        if let Some(tail) = tail_lines {
            args.push("--tail".to_string());
            args.push(tail.to_string());
        }
        args.push(id.to_string());
        let output = self.podman(&args, id).await?;
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        Ok(text)
    }

    async fn exec(&self, id: &str, command: &[String]) -> Result<ExecResult, KubeletError> {
        let mut args = vec!["exec".to_string(), id.to_string()];
        args.extend(command.iter().cloned());
        let output = tokio::process::Command::new(&self.binary)
            .args(&args)
            .output()
            .await
            .map_err(|e| KubeletError::RuntimeUnavailable {
                endpoint: self.binary.display().to_string(),
                reason: format!("failed to execute podman exec: {e}"),
            })?;
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
        ContainerSummary {
            id: self.id,
            image: self.image,
            image_id: self.image_id,
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
            name: "rubix_default_hello_hello_abc".to_string(),
            image: "localhost/rubix-hello:latest".to_string(),
            pull: PullPolicy::Never,
            entrypoint: Some(vec!["/rubix-hello".to_string()]),
            args: vec!["600".to_string()],
            env: vec![("GREETING".to_string(), "hi".to_string())],
            working_dir: Some("/work".to_string()),
            labels: BTreeMap::from([("io.rubix.pod-uid".to_string(), "u1".to_string())]),
        };
        let args = run_arguments(&spec).unwrap();
        assert_eq!(
            args,
            [
                "run",
                "--detach",
                "--name",
                "rubix_default_hello_hello_abc",
                "--pull",
                "never",
                "--label",
                "io.rubix.pod-uid=u1",
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
    fn ps_output_maps_running_and_exited_containers() {
        let stdout = r#"[
          {"Id":"abc123","Image":"localhost/rubix-hello:latest","ImageID":"e247","Labels":{"io.rubix.container-name":"hello","io.rubix.pod-uid":"u1"},"State":"running","ExitCode":0,"Exited":false,"StartedAt":1791146364,"ExitedAt":-62135596800},
          {"Id":"def456","Image":"img","ImageID":"i2","Labels":{"io.rubix.container-name":"side"},"State":"exited","ExitCode":3,"Exited":true,"StartedAt":1791146364,"ExitedAt":1791146370},
          {"Id":"ghi789","State":"created"}
        ]"#;
        let entries = parse_ps_output(stdout).unwrap();
        assert_eq!(entries.len(), 3);
        let running = entries[0].clone().into_summary();
        assert_eq!(running.id, "abc123");
        assert_eq!(running.labels["io.rubix.container-name"], "hello");
        assert_eq!(
            running.state,
            ContainerState::Running {
                started_at: Some(1_791_146_364)
            }
        );
        assert_eq!(
            entries[1].clone().into_summary().state,
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
