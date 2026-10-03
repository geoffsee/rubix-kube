//! Published container endpoint resolution and selective kubeconfig edits for
//! named instances. Only the entries owned by the selected context are touched;
//! certificate and credential data are never modified.
use std::fmt;

use serde_json::Value;

/// Host-reachable loopback endpoint a container published its API port on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublishedEndpoint {
    pub port: u16,
}

impl PublishedEndpoint {
    /// Server URL routed through loopback, preserving the https scheme.
    pub fn server_url(&self) -> String {
        format!("https://127.0.0.1:{}", self.port)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointError {
    NoPublishedPort,
    ContextNotFound(String),
    ClusterNotFound(String),
}

impl fmt::Display for EndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPublishedPort => f.write_str("no published host port found"),
            Self::ContextNotFound(n) => write!(f, "context '{n}' not found"),
            Self::ClusterNotFound(n) => write!(f, "cluster '{n}' not found"),
        }
    }
}

impl std::error::Error for EndpointError {}

/// Parses `docker port <container> <port>/tcp` output (for example
/// `0.0.0.0:49153` and `[::]:49153`) into a loopback endpoint.
pub fn resolve_published_endpoint(port_output: &str) -> Result<PublishedEndpoint, EndpointError> {
    port_output
        .lines()
        .filter_map(|line| line.trim().rsplit_once(':'))
        .filter_map(|(_, port)| port.trim().parse::<u16>().ok())
        .find(|port| *port != 0)
        .map(|port| PublishedEndpoint { port })
        .ok_or(EndpointError::NoPublishedPort)
}

fn find_named<'a>(cfg: &'a Value, list: &str, name: &str) -> Option<&'a Value> {
    cfg.get(list)?
        .as_array()?
        .iter()
        .find(|e| e.get("name").and_then(Value::as_str) == Some(name))
}

/// Rewrites only the server of the cluster referenced by `context`.
/// Certificate authority and user credentials remain intact.
pub fn route_context(
    cfg: &mut Value,
    context: &str,
    endpoint: PublishedEndpoint,
) -> Result<(), EndpointError> {
    let cluster_name = find_named(cfg, "contexts", context)
        .ok_or_else(|| EndpointError::ContextNotFound(context.to_string()))?
        .get("context")
        .and_then(|c| c.get("cluster"))
        .and_then(Value::as_str)
        .ok_or_else(|| EndpointError::ClusterNotFound(String::new()))?
        .to_string();
    let cluster = cfg
        .get_mut("clusters")
        .and_then(Value::as_array_mut)
        .and_then(|l| {
            l.iter_mut()
                .find(|e| e.get("name").and_then(Value::as_str) == Some(&cluster_name))
        })
        .and_then(|e| e.get_mut("cluster"))
        .and_then(Value::as_object_mut)
        .ok_or(EndpointError::ClusterNotFound(cluster_name))?;
    cluster.insert("server".to_string(), Value::String(endpoint.server_url()));
    Ok(())
}

fn referenced(cfg: &Value, key: &str, name: &str) -> bool {
    cfg.get("contexts")
        .and_then(Value::as_array)
        .is_some_and(|l| {
            l.iter().any(|c| {
                c.get("context")
                    .and_then(|x| x.get(key))
                    .and_then(Value::as_str)
                    == Some(name)
            })
        })
}

/// Removes the named context and its cluster/user entries when no other
/// context still references them. Unrelated data is preserved; a matching
/// `current-context` is cleared. Returns whether the context existed.
pub fn remove_instance(cfg: &mut Value, context: &str) -> bool {
    let Some(entry) = find_named(cfg, "contexts", context) else {
        return false;
    };
    let ctx = entry.get("context");
    let cluster = ctx
        .and_then(|c| c.get("cluster"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let user = ctx
        .and_then(|c| c.get("user"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let drop_named = |cfg: &mut Value, list: &str, name: &str| {
        if let Some(l) = cfg.get_mut(list).and_then(Value::as_array_mut) {
            l.retain(|e| e.get("name").and_then(Value::as_str) != Some(name));
        }
    };
    drop_named(cfg, "contexts", context);
    if let Some(c) = cluster
        && !referenced(cfg, "cluster", &c)
    {
        drop_named(cfg, "clusters", &c);
    }
    if let Some(u) = user
        && !referenced(cfg, "user", &u)
    {
        drop_named(cfg, "users", &u);
    }
    if cfg.get("current-context").and_then(Value::as_str) == Some(context)
        && let Some(o) = cfg.as_object_mut()
    {
        o.remove("current-context");
    }
    true
}
