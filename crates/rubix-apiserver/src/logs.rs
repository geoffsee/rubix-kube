//! Pod log subresource backed by the in-process kubelet.
//!
//! Upstream kube-apiserver proxies `GET .../pods/{name}/log` to the kubelet.
//! Here the kubelet runs in the same process, so it registers a reader with
//! [`crate::ApiserverService`] and the HTTPS gateway calls it directly.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::ApiserverError;

/// Query options accepted by the pod log subresource.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PodLogOptions {
    /// Container to read; the first spec container when absent.
    pub container: Option<String>,
    /// Return only the last `tail_lines` lines.
    pub tail_lines: Option<usize>,
}

impl PodLogOptions {
    /// Parses the raw URL query of a log request.
    #[must_use]
    pub fn from_query(query: Option<&str>) -> Self {
        let mut options = Self::default();
        for pair in query.unwrap_or_default().split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            match key {
                "container" if !value.is_empty() => options.container = Some(value.to_string()),
                "tailLines" => options.tail_lines = value.parse().ok(),
                _ => {},
            }
        }
        options
    }
}

/// Reads container output for a stored Pod object.
#[async_trait]
pub trait PodLogReader: std::fmt::Debug + Send + Sync {
    /// Returns the captured stdout and stderr of one container of `pod`.
    async fn read_pod_log(
        &self,
        pod: &Value,
        options: &PodLogOptions,
    ) -> Result<String, ApiserverError>;
}

#[cfg(test)]
mod tests {
    use super::PodLogOptions;

    #[test]
    fn parses_container_and_tail_from_query() {
        let options = PodLogOptions::from_query(Some("container=hello&tailLines=10&follow=false"));
        assert_eq!(options.container.as_deref(), Some("hello"));
        assert_eq!(options.tail_lines, Some(10));
        assert_eq!(PodLogOptions::from_query(None), PodLogOptions::default());
        assert_eq!(
            PodLogOptions::from_query(Some("tailLines=x")).tail_lines,
            None
        );
    }
}
