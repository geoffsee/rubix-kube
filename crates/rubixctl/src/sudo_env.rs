use std::collections::BTreeMap;
use std::io;

/// Scans raw environ bytes (typically from `/proc/$PPID/environ`) and recovers
/// Portainer Edge variables stripped by sudo `env_reset`.
///
/// Only triggers when running under sudo (`SUDO_USER` is present) with
/// `KUBESOLO_PORTAINER_EDGE_KEY` missing from the current process environment.
pub fn recover_sudo_env_from_bytes(
    environment: &mut BTreeMap<String, String>,
    environ_bytes: &[u8],
) {
    if !environment.contains_key("SUDO_USER") {
        return;
    }
    if environment.contains_key("KUBESOLO_PORTAINER_EDGE_KEY") {
        return;
    }
    for chunk in environ_bytes.split(|b| *b == 0) {
        if chunk.starts_with(b"KUBESOLO_")
            && let Some(pos) = chunk.iter().position(|b| *b == b'=')
            && let (Ok(key), Ok(val)) = (
                std::str::from_utf8(&chunk[..pos]),
                std::str::from_utf8(&chunk[pos + 1..]),
            )
        {
            if !matches!(
                key,
                "KUBESOLO_PORTAINER_EDGE_ID"
                    | "KUBESOLO_PORTAINER_EDGE_KEY"
                    | "KUBESOLO_PORTAINER_EDGE_ASYNC"
                    | "KUBESOLO_PORTAINER_EDGE_IMAGE"
            ) {
                continue;
            }
            environment
                .entry(key.to_string())
                .or_insert_with(|| val.to_string());
        }
    }
}

/// Helper that detects parent PID from `/proc/self/status` and reads parent environ bytes.
pub fn recover_sudo_environment(
    environment: &mut BTreeMap<String, String>,
    reader: impl Fn(u32) -> io::Result<Vec<u8>>,
) {
    if !environment.contains_key("SUDO_USER")
        || environment.contains_key("KUBESOLO_PORTAINER_EDGE_KEY")
    {
        return;
    }
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("PPid:")
                && let Ok(ppid) = rest.trim().parse::<u32>()
                && let Ok(bytes) = reader(ppid)
            {
                recover_sudo_env_from_bytes(environment, &bytes);
                return;
            }
        }
    }
}
