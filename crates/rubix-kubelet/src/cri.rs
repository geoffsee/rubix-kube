//! CRI runtime provider: implements [`RuntimeProvider`] over [`rubix_cri::CriClient`]
//! for containerd on Linux.
//!
//! Every required [`RuntimeProvider`] method forwards 1:1 to the official Kubernetes v1.35.7
//! CRI v1 protocol without semantic translation beyond error mapping. Container logs are
//! retrieved from the CRI log file recorded at `ContainerStatus.log_path` and parsed
//! according to the CRI log format (`<timestamp> <stream> <tag> <content>`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use tokio::sync::Mutex;

use rubix_cri::endpoint::RuntimeEndpoints;
pub use rubix_cri::runtime::v1 as types;
use rubix_cri::runtime::v1 as cri;
pub use rubix_cri::runtime::v1::*;
use rubix_cri::workload::CriClient;

use crate::error::KubeletError;
use crate::workload::{ExecResult, LogOptions, RuntimeProvider};

/// Runtime provider adapting [`rubix_cri::CriClient`] to [`RuntimeProvider`].
#[derive(Clone, Debug)]
pub struct CriRuntimeProvider {
    socket_path: PathBuf,
    client: Arc<Mutex<Option<CriClient>>>,
    version: Arc<RwLock<String>>,
}

impl CriRuntimeProvider {
    /// Constructs a provider that connects lazily to the Unix domain socket.
    #[must_use]
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            client: Arc::new(Mutex::new(None)),
            version: Arc::new(RwLock::new("containerd".to_string())),
        }
    }

    /// Constructs a provider with a pre-connected [`CriClient`].
    #[must_use]
    pub fn with_client(socket_path: impl Into<PathBuf>, client: CriClient) -> Self {
        Self {
            socket_path: socket_path.into(),
            client: Arc::new(Mutex::new(Some(client))),
            version: Arc::new(RwLock::new("containerd".to_string())),
        }
    }

    /// Returns the configured Unix domain socket path.
    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Obtains a clone of the client, connecting on first call if needed.
    pub async fn client(&self) -> Result<CriClient, KubeletError> {
        let mut guard = self.client.lock().await;
        if let Some(client) = &*guard {
            return Ok(client.clone());
        }

        let endpoint =
            rubix_cri::endpoint::CriEndpoint::from_path(&self.socket_path).map_err(|err| {
                KubeletError::RuntimeUnavailable {
                    endpoint: self.socket_path.display().to_string(),
                    reason: format!("invalid socket path: {err:?}"),
                }
            })?;
        let endpoints = RuntimeEndpoints::single(endpoint);
        let mut client = CriClient::connect(&endpoints).await.map_err(|err| {
            KubeletError::RuntimeUnavailable {
                endpoint: self.socket_path.display().to_string(),
                reason: err.to_string(),
            }
        })?;

        if let Ok(resp) = client.version("v1").await {
            let v = format!("{}://{}", resp.runtime_name, resp.runtime_version);
            if let Ok(mut w) = self.version.write() {
                *w = v;
            }
        }

        *guard = Some(client.clone());
        Ok(client)
    }
}

fn map_status(op: &'static str, status: &rubix_cri::tonic::Status) -> KubeletError {
    KubeletError::ContainerOperationFailed {
        container: op.to_string(),
        reason: status.to_string(),
    }
}

#[async_trait]
impl RuntimeProvider for CriRuntimeProvider {
    fn provider_name(&self) -> &'static str {
        "containerd"
    }

    fn runtime_version(&self) -> String {
        self.version
            .read()
            .map_or_else(|_| "containerd".to_string(), |v| v.clone())
    }

    fn requires_socket(&self) -> bool {
        true
    }

    fn is_external(&self) -> bool {
        true
    }

    async fn check_available(&self) -> Result<(), KubeletError> {
        let mut client = self.client().await?;
        client
            .version("v1")
            .await
            .map_err(|s| KubeletError::RuntimeUnavailable {
                endpoint: self.socket_path.display().to_string(),
                reason: s.to_string(),
            })?;
        Ok(())
    }

    async fn run_pod_sandbox(
        &self,
        config: &cri::PodSandboxConfig,
    ) -> Result<String, KubeletError> {
        let mut client = self.client().await?;
        let req = cri::RunPodSandboxRequest {
            config: Some(config.clone()),
            runtime_handler: String::new(),
        };
        client
            .run_pod_sandbox(req)
            .await
            .map_err(|s| map_status("run_pod_sandbox", &s))
    }

    async fn stop_pod_sandbox(&self, pod_sandbox_id: &str) -> Result<(), KubeletError> {
        let mut client = self.client().await?;
        client
            .stop_pod_sandbox(pod_sandbox_id)
            .await
            .map_err(|s| map_status("stop_pod_sandbox", &s))
    }

    async fn remove_pod_sandbox(&self, pod_sandbox_id: &str) -> Result<(), KubeletError> {
        let mut client = self.client().await?;
        client
            .remove_pod_sandbox(pod_sandbox_id)
            .await
            .map_err(|s| map_status("remove_pod_sandbox", &s))
    }

    async fn list_pod_sandbox(
        &self,
        filter: Option<&cri::PodSandboxFilter>,
    ) -> Result<Vec<cri::PodSandbox>, KubeletError> {
        let mut client = self.client().await?;
        client
            .list_pod_sandboxes(filter.cloned())
            .await
            .map_err(|s| map_status("list_pod_sandbox", &s))
    }

    async fn pod_sandbox_status(
        &self,
        pod_sandbox_id: &str,
    ) -> Result<cri::PodSandboxStatus, KubeletError> {
        let mut client = self.client().await?;
        client
            .pod_sandbox_status(pod_sandbox_id, false)
            .await
            .map_err(|s| map_status("pod_sandbox_status", &s))
    }

    async fn create_container(
        &self,
        pod_sandbox_id: &str,
        config: &cri::ContainerConfig,
        sandbox_config: &cri::PodSandboxConfig,
    ) -> Result<String, KubeletError> {
        let mut client = self.client().await?;
        let req = cri::CreateContainerRequest {
            pod_sandbox_id: pod_sandbox_id.to_string(),
            config: Some(config.clone()),
            sandbox_config: Some(sandbox_config.clone()),
        };
        client
            .create_container(req)
            .await
            .map_err(|s| map_status("create_container", &s))
    }

    async fn start_container(&self, container_id: &str) -> Result<(), KubeletError> {
        let mut client = self.client().await?;
        client
            .start_container(container_id)
            .await
            .map_err(|s| map_status("start_container", &s))
    }

    async fn stop_container(
        &self,
        container_id: &str,
        timeout_secs: i64,
    ) -> Result<(), KubeletError> {
        let mut client = self.client().await?;
        client
            .stop_container(container_id, timeout_secs)
            .await
            .map_err(|s| map_status("stop_container", &s))
    }

    async fn remove_container(&self, container_id: &str) -> Result<(), KubeletError> {
        let mut client = self.client().await?;
        client
            .remove_container(container_id)
            .await
            .map_err(|s| map_status("remove_container", &s))
    }

    async fn list_containers(
        &self,
        filter: Option<&cri::ContainerFilter>,
    ) -> Result<Vec<cri::Container>, KubeletError> {
        let mut client = self.client().await?;
        client
            .list_containers(filter.cloned())
            .await
            .map_err(|s| map_status("list_containers", &s))
    }

    async fn container_status(
        &self,
        container_id: &str,
    ) -> Result<cri::ContainerStatus, KubeletError> {
        let mut client = self.client().await?;
        client
            .container_status(container_id, false)
            .await
            .map_err(|s| map_status("container_status", &s))
    }

    async fn exec_sync(
        &self,
        container_id: &str,
        cmd: &[String],
        timeout_secs: i64,
    ) -> Result<ExecResult, KubeletError> {
        let mut client = self.client().await?;
        let resp = client
            .exec_sync(container_id, cmd.to_vec(), timeout_secs)
            .await
            .map_err(|s| map_status("exec_sync", &s))?;
        Ok(ExecResult {
            exit_code: resp.exit_code,
            stdout: String::from_utf8_lossy(&resp.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&resp.stderr).into_owned(),
        })
    }

    async fn container_logs(
        &self,
        container_id: &str,
        options: &LogOptions,
    ) -> Result<String, KubeletError> {
        let status = self.container_status(container_id).await?;
        if status.log_path.is_empty() {
            return Ok(String::new());
        }
        let raw = match tokio::fs::read_to_string(&status.log_path).await {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
            Err(err) => return Err(KubeletError::Io(err)),
        };
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs().cast_signed());
        Ok(parse_cri_logs(&raw, options, now_unix))
    }

    async fn image_status(
        &self,
        image: &cri::ImageSpec,
    ) -> Result<Option<cri::Image>, KubeletError> {
        let mut client = self.client().await?;
        client
            .image_status_spec(Some(image.clone()), false)
            .await
            .map_err(|s| map_status("image_status", &s))
    }

    async fn pull_image(
        &self,
        image: &cri::ImageSpec,
        sandbox_config: Option<&cri::PodSandboxConfig>,
    ) -> Result<String, KubeletError> {
        let mut client = self.client().await?;
        client
            .pull_image_spec(Some(image.clone()), None, sandbox_config.cloned())
            .await
            .map_err(|s| map_status("pull_image", &s))
    }
}

/// Parses an RFC 3339 timestamp string into Unix epoch seconds.
#[must_use]
pub fn parse_rfc3339_unix_secs(ts: &str) -> Option<i64> {
    let ts = ts.trim();
    let (date_part, time_part) = ts.split_once('T')?;
    let mut date_pieces = date_part.split('-');
    let year: i64 = date_pieces.next()?.parse().ok()?;
    let month: u64 = date_pieces.next()?.parse().ok()?;
    let day: u64 = date_pieces.next()?.parse().ok()?;
    if date_pieces.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let (time_str, tz_offset_secs) = if let Some(idx) = time_part.find('Z') {
        (&time_part[..idx], 0i64)
    } else if let Some(idx) = time_part.rfind('+') {
        let offset_str = &time_part[idx + 1..];
        let (oh, om) = offset_str.split_once(':')?;
        let offset = oh.parse::<i64>().ok()? * 3600 + om.parse::<i64>().ok()? * 60;
        (&time_part[..idx], offset)
    } else {
        let idx = time_part.rfind('-')?;
        let offset_str = &time_part[idx + 1..];
        let (oh, om) = offset_str.split_once(':')?;
        let offset = -(oh.parse::<i64>().ok()? * 3600 + om.parse::<i64>().ok()? * 60);
        (&time_part[..idx], offset)
    };

    let mut time_pieces = time_str.split(':');
    let hour: i64 = time_pieces.next()?.parse().ok()?;
    let minute: i64 = time_pieces.next()?.parse().ok()?;
    let sec_str = time_pieces.next()?;
    if time_pieces.next().is_some() || hour > 23 || minute > 59 {
        return None;
    }

    let sec_int_str = sec_str.split('.').next().unwrap_or(sec_str);
    let second: i64 = sec_int_str.parse().ok()?;
    if second > 60 {
        return None;
    }

    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - tz_offset_secs)
}

fn days_from_civil(y: i64, m: u64, d: u64) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400).cast_unsigned();
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe.cast_signed() - 719_468
}

struct PartialLog {
    timestamp: String,
    timestamp_unix: Option<i64>,
    content: String,
}

struct LogEntry {
    timestamp: String,
    timestamp_unix: Option<i64>,
    content: String,
}

/// Parses CRI-formatted log text (`<timestamp> <stream> <tag> <content>`)
/// applying [`LogOptions`] filters (`tail_lines`, `timestamps`, `since_seconds`).
#[must_use]
pub fn parse_cri_logs(raw: &str, options: &LogOptions, now_unix: i64) -> String {
    let mut entries = Vec::new();
    let mut stdout_partial: Option<PartialLog> = None;
    let mut stderr_partial: Option<PartialLog> = None;

    for line in raw.lines() {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        if parts.len() >= 3
            && (parts[1] == "stdout" || parts[1] == "stderr")
            && (parts[2] == "P" || parts[2] == "F")
        {
            let ts_str = parts[0];
            let stream = parts[1];
            let tag = parts[2];
            let content = if parts.len() == 4 { parts[3] } else { "" };
            let partial_slot = if stream == "stdout" {
                &mut stdout_partial
            } else {
                &mut stderr_partial
            };

            if tag == "P" {
                if let Some(partial) = partial_slot {
                    partial.content.push_str(content);
                } else {
                    *partial_slot = Some(PartialLog {
                        timestamp: ts_str.to_string(),
                        timestamp_unix: parse_rfc3339_unix_secs(ts_str),
                        content: content.to_string(),
                    });
                }
            } else if let Some(mut partial) = partial_slot.take() {
                partial.content.push_str(content);
                entries.push(LogEntry {
                    timestamp: partial.timestamp,
                    timestamp_unix: partial.timestamp_unix,
                    content: partial.content,
                });
            } else {
                entries.push(LogEntry {
                    timestamp: ts_str.to_string(),
                    timestamp_unix: parse_rfc3339_unix_secs(ts_str),
                    content: content.to_string(),
                });
            }
        } else {
            // Graceful fallback for non-CRI text
            entries.push(LogEntry {
                timestamp: String::new(),
                timestamp_unix: None,
                content: line.to_string(),
            });
        }
    }

    if let Some(partial) = stdout_partial.take() {
        entries.push(LogEntry {
            timestamp: partial.timestamp,
            timestamp_unix: partial.timestamp_unix,
            content: partial.content,
        });
    }
    if let Some(partial) = stderr_partial.take() {
        entries.push(LogEntry {
            timestamp: partial.timestamp,
            timestamp_unix: partial.timestamp_unix,
            content: partial.content,
        });
    }

    let filtered: Vec<String> = entries
        .into_iter()
        .filter(|entry| {
            if let (Some(since), Some(ts_unix)) = (options.since_seconds, entry.timestamp_unix) {
                let cutoff = now_unix.saturating_sub(since.cast_signed());
                if ts_unix < cutoff {
                    return false;
                }
            }
            true
        })
        .map(|entry| {
            if options.timestamps && !entry.timestamp.is_empty() {
                format!("{} {}\n", entry.timestamp, entry.content)
            } else {
                format!("{}\n", entry.content)
            }
        })
        .collect();

    let final_lines = if let Some(tail) = options.tail_lines {
        let start = filtered.len().saturating_sub(tail);
        &filtered[start..]
    } else {
        &filtered[..]
    };

    let mut result = String::new();
    for line in final_lines {
        result.push_str(line);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_timestamp_parsing() {
        assert_eq!(parse_rfc3339_unix_secs("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339_unix_secs("2026-10-08T00:00:00Z"),
            Some(1_791_417_600)
        );
        assert_eq!(
            parse_rfc3339_unix_secs("2026-10-08T01:30:15.123456789Z"),
            Some(1_791_417_600 + 3600 + 1800 + 15)
        );
        // Timezone offset +02:00
        assert_eq!(
            parse_rfc3339_unix_secs("2026-10-08T02:00:00+02:00"),
            Some(1_791_417_600)
        );
        // Timezone offset -05:00
        assert_eq!(
            parse_rfc3339_unix_secs("2026-10-07T19:00:00-05:00"),
            Some(1_791_417_600)
        );
    }

    #[test]
    fn cri_log_parsing_full_and_partial_stitching() {
        let raw = "\
2026-10-08T18:00:00.000Z stdout F full line 1
2026-10-08T18:00:01.000Z stderr P part 1 - 
2026-10-08T18:00:02.000Z stderr F part 2
2026-10-08T18:00:03.000Z stdout F full line 2
";
        let default_opts = LogOptions::default();
        let parsed = parse_cri_logs(raw, &default_opts, 1_791_482_400);
        assert_eq!(parsed, "full line 1\npart 1 - part 2\nfull line 2\n");

        let with_ts = LogOptions {
            timestamps: true,
            ..Default::default()
        };
        let parsed_ts = parse_cri_logs(raw, &with_ts, 1_791_482_400);
        assert_eq!(
            parsed_ts,
            "2026-10-08T18:00:00.000Z full line 1\n2026-10-08T18:00:01.000Z part 1 - part 2\n2026-10-08T18:00:03.000Z full line 2\n"
        );
    }

    #[test]
    fn cri_log_tail_lines() {
        let raw = "\
2026-10-08T18:00:00Z stdout F line 1
2026-10-08T18:00:01Z stdout F line 2
2026-10-08T18:00:02Z stdout F line 3
";
        let opts = LogOptions {
            tail_lines: Some(2),
            ..Default::default()
        };
        let parsed = parse_cri_logs(raw, &opts, 1_791_482_400);
        assert_eq!(parsed, "line 2\nline 3\n");
    }

    #[test]
    fn cri_log_since_seconds() {
        // 2026-10-08T00:00:00Z = 1_791_417_600
        let raw = "\
2026-10-08T00:00:00Z stdout F old line
2026-10-08T00:00:50Z stdout F recent line
";
        let now_unix = 1_791_417_600 + 60; // 60 seconds after base
        let opts = LogOptions {
            since_seconds: Some(20), // only last 20 seconds (cutoff = 40)
            ..Default::default()
        };
        let parsed = parse_cri_logs(raw, &opts, now_unix);
        assert_eq!(parsed, "recent line\n");
    }

    #[test]
    fn non_cri_fallback() {
        let raw = "hello world\nanother raw line\n";
        let parsed = parse_cri_logs(raw, &LogOptions::default(), 0);
        assert_eq!(parsed, "hello world\nanother raw line\n");
    }

    #[test]
    fn provider_properties() {
        let provider = CriRuntimeProvider::new("/run/containerd/containerd.sock");
        assert_eq!(provider.provider_name(), "containerd");
        assert!(provider.requires_socket());
        assert!(provider.is_external());
        assert_eq!(
            provider.socket_path(),
            Path::new("/run/containerd/containerd.sock")
        );
    }
}
