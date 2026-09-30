use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::ApiserverError;
use crate::storage::KubernetesStorage;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailurePolicy {
    #[serde(rename = "Fail")]
    Fail,
    #[serde(rename = "Ignore")]
    Ignore,
}

impl Default for FailurePolicy {
    fn default() -> Self {
        Self::Fail
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceReference {
    pub namespace: String,
    pub name: String,
    pub path: Option<String>,
    pub port: Option<i32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WebhookClientConfig {
    pub url: Option<String>,
    pub service: Option<ServiceReference>,
    #[serde(rename = "caBundle")]
    pub ca_bundle: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleWithOperations {
    pub operations: Vec<String>,
    #[serde(rename = "apiGroups")]
    pub api_groups: Vec<String>,
    #[serde(rename = "apiVersions")]
    pub api_versions: Vec<String>,
    pub resources: Vec<String>,
    pub scope: Option<String>,
}

impl RuleWithOperations {
    #[must_use]
    pub fn matches(&self, op: &str, group: &str, version: &str, resource: &str) -> bool {
        let op_match = self
            .operations
            .iter()
            .any(|o| o == "*" || o.eq_ignore_ascii_case(op));
        let group_match = self.api_groups.iter().any(|g| g == "*" || g == group);
        let version_match = self.api_versions.iter().any(|v| v == "*" || v == version);
        let res_match = self.resources.iter().any(|r| r == "*" || r == resource);

        op_match && group_match && version_match && res_match
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebhookDefinition {
    pub name: String,
    pub rules: Vec<RuleWithOperations>,
    #[serde(rename = "clientConfig")]
    pub client_config: WebhookClientConfig,
    #[serde(default, rename = "failurePolicy")]
    pub failure_policy: FailurePolicy,
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: Option<u32>,
    #[serde(default, rename = "admissionReviewVersions")]
    pub admission_review_versions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatingWebhookConfiguration {
    pub metadata: BTreeMap<String, Value>,
    pub webhooks: Vec<WebhookDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutatingWebhookConfiguration {
    pub metadata: BTreeMap<String, Value>,
    pub webhooks: Vec<WebhookDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupVersionKind {
    pub group: String,
    pub version: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupVersionResource {
    pub group: String,
    pub version: String,
    pub resource: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub uid: Option<String>,
    pub groups: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdmissionRequest {
    pub uid: String,
    pub kind: GroupVersionKind,
    pub resource: GroupVersionResource,
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub operation: String,
    #[serde(rename = "userInfo")]
    pub user_info: UserInfo,
    pub object: Option<Value>,
    #[serde(rename = "oldObject")]
    pub old_object: Option<Value>,
    #[serde(rename = "dryRun")]
    pub dry_run: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdmissionStatus {
    pub code: Option<u16>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdmissionResponse {
    pub uid: String,
    pub allowed: bool,
    pub status: Option<AdmissionStatus>,
    pub patch: Option<String>,
    #[serde(rename = "patchType")]
    pub patch_type: Option<String>,
}

impl AdmissionResponse {
    #[must_use]
    pub fn allow(uid: impl Into<String>) -> Self {
        Self {
            uid: uid.into(),
            allowed: true,
            status: None,
            patch: None,
            patch_type: None,
        }
    }

    #[must_use]
    pub fn deny(uid: impl Into<String>, code: u16, message: impl Into<String>) -> Self {
        Self {
            uid: uid.into(),
            allowed: false,
            status: Some(AdmissionStatus {
                code: Some(code),
                message: Some(message.into()),
            }),
            patch: None,
            patch_type: None,
        }
    }

    #[must_use]
    pub fn mutate_json_patch(uid: impl Into<String>, patch_json_bytes: &[u8]) -> Self {
        let enc = rubix_pki::base64_encode(patch_json_bytes);
        Self {
            uid: uid.into(),
            allowed: true,
            status: None,
            patch: Some(enc),
            patch_type: Some("JSONPatch".to_string()),
        }
    }
}

#[async_trait]
pub trait WebhookHandler: Send + Sync {
    async fn handle(&self, req: &AdmissionRequest) -> Result<AdmissionResponse, ApiserverError>;
}

struct RegisteredEndpoint {
    handler: Arc<dyn WebhookHandler>,
    server_cert_pem: Option<String>,
}

#[derive(Clone, Default)]
pub struct AdmissionEngine {
    mutating_configs: Arc<RwLock<Vec<MutatingWebhookConfiguration>>>,
    validating_configs: Arc<RwLock<Vec<ValidatingWebhookConfiguration>>>,
    endpoints: Arc<RwLock<BTreeMap<String, Arc<RegisteredEndpoint>>>>,
}

impl std::fmt::Debug for AdmissionEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdmissionEngine")
            .field(
                "mutating_configs_count",
                &self.mutating_configs.read().map(|c| c.len()).unwrap_or(0),
            )
            .field(
                "validating_configs_count",
                &self.validating_configs.read().map(|c| c.len()).unwrap_or(0),
            )
            .field(
                "endpoints_count",
                &self.endpoints.read().map(|e| e.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl AdmissionEngine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_webhook_endpoint(
        &self,
        name: impl Into<String>,
        handler: Arc<dyn WebhookHandler>,
        server_cert_pem: Option<String>,
    ) {
        let mut guard = self.endpoints.write().unwrap();
        guard.insert(
            name.into(),
            Arc::new(RegisteredEndpoint {
                handler,
                server_cert_pem,
            }),
        );
    }

    pub fn add_mutating_webhook_config(&self, config: MutatingWebhookConfiguration) {
        let mut guard = self.mutating_configs.write().unwrap();
        // Remove existing config with same name if present
        if let Some(name) = config.metadata.get("name").and_then(Value::as_str) {
            guard.retain(|c| {
                c.metadata
                    .get("name")
                    .and_then(Value::as_str)
                    .map_or(true, |n| n != name)
            });
        }
        guard.push(config);
    }

    pub fn add_validating_webhook_config(&self, config: ValidatingWebhookConfiguration) {
        let mut guard = self.validating_configs.write().unwrap();
        if let Some(name) = config.metadata.get("name").and_then(Value::as_str) {
            guard.retain(|c| {
                c.metadata
                    .get("name")
                    .and_then(Value::as_str)
                    .map_or(true, |n| n != name)
            });
        }
        guard.push(config);
    }

    pub fn clear_configs(&self) {
        self.mutating_configs.write().unwrap().clear();
        self.validating_configs.write().unwrap().clear();
    }

    /// Verifies the webhook endpoint's TLS certificate against the ca_bundle if present.
    fn verify_webhook_trust(
        &self,
        webhook_name: &str,
        client_config: &WebhookClientConfig,
        endpoint: &RegisteredEndpoint,
    ) -> Result<(), ApiserverError> {
        let Some(ca_bundle_b64) = &client_config.ca_bundle else {
            return Ok(());
        };

        let ca_bytes = rubix_pki::base64_decode(ca_bundle_b64).map_err(|e| {
            ApiserverError::InvalidCredentials {
                reason: format!("invalid base64 in webhook '{webhook_name}' caBundle: {e}"),
            }
        })?;

        let ca_pem =
            std::str::from_utf8(&ca_bytes).map_err(|e| ApiserverError::InvalidCredentials {
                reason: format!("invalid UTF-8 in webhook '{webhook_name}' caBundle: {e}"),
            })?;

        let Some(server_cert) = &endpoint.server_cert_pem else {
            return Err(ApiserverError::InvalidCredentials {
                reason: format!(
                    "webhook '{webhook_name}' expects TLS verification against caBundle, but endpoint provided no certificate"
                ),
            });
        };

        rubix_pki::verify_certificate_chain(server_cert, ca_pem).map_err(|e| {
            ApiserverError::InvalidCredentials {
                reason: format!(
                    "webhook '{webhook_name}' server TLS certificate rejected by caBundle trust: {e}"
                ),
            }
        })?;

        Ok(())
    }

    /// Runs all matching mutating webhooks against the admission request.
    /// Modifies `req.object` in place if patches are returned.
    pub async fn run_mutating_admission(
        &self,
        req: &mut AdmissionRequest,
    ) -> Result<(), ApiserverError> {
        let configs = self.mutating_configs.read().unwrap().clone();

        for config in configs {
            for webhook in &config.webhooks {
                let matches = webhook.rules.iter().any(|r| {
                    r.matches(
                        &req.operation,
                        &req.resource.group,
                        &req.resource.version,
                        &req.resource.resource,
                    )
                });

                if !matches {
                    continue;
                }

                let endpoint = {
                    let guard = self.endpoints.read().unwrap();
                    guard
                        .get(&webhook.name)
                        .or_else(|| {
                            webhook
                                .client_config
                                .service
                                .as_ref()
                                .and_then(|s| guard.get(&s.name))
                        })
                        .or_else(|| {
                            webhook.client_config.url.as_ref().and_then(|u| {
                                guard.get(u).or_else(|| {
                                    if let Some(host) = u
                                        .strip_prefix("https://")
                                        .or_else(|| u.strip_prefix("http://"))
                                    {
                                        let host_name = host
                                            .split('/')
                                            .next()
                                            .unwrap_or(host)
                                            .split(':')
                                            .next()
                                            .unwrap_or(host);
                                        guard.get(host_name)
                                    } else {
                                        None
                                    }
                                })
                            })
                        })
                        .cloned()
                };

                let Some(endpoint) = endpoint else {
                    if webhook.failure_policy == FailurePolicy::Fail {
                        return Err(ApiserverError::WebhookFailure {
                            webhook: webhook.name.clone(),
                            reason: "webhook endpoint handler not registered".to_string(),
                        });
                    }
                    continue;
                };

                // Trust verification
                if let Err(err) =
                    self.verify_webhook_trust(&webhook.name, &webhook.client_config, &endpoint)
                {
                    if webhook.failure_policy == FailurePolicy::Fail {
                        return Err(ApiserverError::WebhookFailure {
                            webhook: webhook.name.clone(),
                            reason: format!("TLS certificate chain verification failed: {err}"),
                        });
                    }
                    continue;
                }

                // Invoke webhook handler
                let response = match endpoint.handler.handle(req).await {
                    Ok(resp) => resp,
                    Err(err) => {
                        if webhook.failure_policy == FailurePolicy::Fail {
                            return match err {
                                ApiserverError::WebhookFailure { .. } => Err(err),
                                other => Err(ApiserverError::WebhookFailure {
                                    webhook: webhook.name.clone(),
                                    reason: other.to_string(),
                                }),
                            };
                        }
                        continue;
                    },
                };

                if !response.allowed {
                    let msg = response.status.and_then(|s| s.message).unwrap_or_else(|| {
                        format!("admission denied by webhook '{}'", webhook.name)
                    });
                    return Err(ApiserverError::AdmissionDenied { reason: msg });
                }

                // Apply patch if returned
                if let Some(patch_b64) = response.patch
                    && let Some(obj) = &mut req.object
                {
                    let patch_bytes = rubix_pki::base64_decode(&patch_b64).map_err(|e| {
                        ApiserverError::InvalidInput {
                            field: "patch".to_string(),
                            reason: format!(
                                "failed to decode base64 JSON patch from {}: {e}",
                                webhook.name
                            ),
                        }
                    })?;
                    apply_json_patch(obj, &patch_bytes)?;
                }
            }
        }

        Ok(())
    }

    /// Runs all matching validating webhooks against the admission request.
    pub async fn run_validating_admission(
        &self,
        req: &AdmissionRequest,
    ) -> Result<(), ApiserverError> {
        let configs = self.validating_configs.read().unwrap().clone();

        for config in configs {
            for webhook in &config.webhooks {
                let matches = webhook.rules.iter().any(|r| {
                    r.matches(
                        &req.operation,
                        &req.resource.group,
                        &req.resource.version,
                        &req.resource.resource,
                    )
                });

                if !matches {
                    continue;
                }

                let endpoint = {
                    let guard = self.endpoints.read().unwrap();
                    guard
                        .get(&webhook.name)
                        .or_else(|| {
                            webhook
                                .client_config
                                .service
                                .as_ref()
                                .and_then(|s| guard.get(&s.name))
                        })
                        .or_else(|| {
                            webhook.client_config.url.as_ref().and_then(|u| {
                                guard.get(u).or_else(|| {
                                    if let Some(host) = u
                                        .strip_prefix("https://")
                                        .or_else(|| u.strip_prefix("http://"))
                                    {
                                        let host_name = host
                                            .split('/')
                                            .next()
                                            .unwrap_or(host)
                                            .split(':')
                                            .next()
                                            .unwrap_or(host);
                                        guard.get(host_name)
                                    } else {
                                        None
                                    }
                                })
                            })
                        })
                        .cloned()
                };

                let Some(endpoint) = endpoint else {
                    if webhook.failure_policy == FailurePolicy::Fail {
                        return Err(ApiserverError::WebhookFailure {
                            webhook: webhook.name.clone(),
                            reason: "webhook endpoint handler not registered".to_string(),
                        });
                    }
                    continue;
                };

                // Trust verification
                if let Err(err) =
                    self.verify_webhook_trust(&webhook.name, &webhook.client_config, &endpoint)
                {
                    if webhook.failure_policy == FailurePolicy::Fail {
                        return Err(ApiserverError::WebhookFailure {
                            webhook: webhook.name.clone(),
                            reason: format!("TLS certificate chain verification failed: {err}"),
                        });
                    }
                    continue;
                }

                // Invoke webhook handler
                let response = match endpoint.handler.handle(req).await {
                    Ok(resp) => resp,
                    Err(err) => {
                        if webhook.failure_policy == FailurePolicy::Fail {
                            return match err {
                                ApiserverError::WebhookFailure { .. } => Err(err),
                                other => Err(ApiserverError::WebhookFailure {
                                    webhook: webhook.name.clone(),
                                    reason: other.to_string(),
                                }),
                            };
                        }
                        continue;
                    },
                };

                if !response.allowed {
                    let msg = response.status.and_then(|s| s.message).unwrap_or_else(|| {
                        format!("admission denied by webhook '{}'", webhook.name)
                    });
                    return Err(ApiserverError::AdmissionDenied { reason: msg });
                }
            }
        }

        Ok(())
    }

    /// Restores webhook configurations from storage.
    pub async fn restore_from_storage(
        &self,
        storage: &KubernetesStorage,
    ) -> Result<(), ApiserverError> {
        // Mutating
        let mutating_prefix = format!("{}/mutatingwebhookconfigurations", storage.prefix());
        let mutating_kvs = storage.list(&mutating_prefix).await?;
        for kv in mutating_kvs {
            let config: MutatingWebhookConfiguration = serde_json::from_slice(&kv.value)?;
            self.add_mutating_webhook_config(config);
        }

        // Validating
        let validating_prefix = format!("{}/validatingwebhookconfigurations", storage.prefix());
        let validating_kvs = storage.list(&validating_prefix).await?;
        for kv in validating_kvs {
            let config: ValidatingWebhookConfiguration = serde_json::from_slice(&kv.value)?;
            self.add_validating_webhook_config(config);
        }

        Ok(())
    }
}

// --- RFC 6902 JSONPatch Engine ---

pub fn apply_json_patch(target: &mut Value, patch_bytes: &[u8]) -> Result<(), ApiserverError> {
    let patches: Value = serde_json::from_slice(patch_bytes)?;
    let Some(ops) = patches.as_array() else {
        return Err(ApiserverError::InvalidInput {
            field: "patch".to_string(),
            reason: "JSON patch must be an array of operations".to_string(),
        });
    };

    for op in ops {
        let op_type =
            op.get("op")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiserverError::InvalidInput {
                    field: "patch.op".to_string(),
                    reason: "patch operation requires 'op'".to_string(),
                })?;

        let path =
            op.get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiserverError::InvalidInput {
                    field: "patch.path".to_string(),
                    reason: "patch operation requires 'path'".to_string(),
                })?;

        match op_type {
            "add" => {
                let value = op
                    .get("value")
                    .ok_or_else(|| ApiserverError::InvalidInput {
                        field: "patch.value".to_string(),
                        reason: "'add' operation requires 'value'".to_string(),
                    })?;
                patch_add(target, path, value.clone())?;
            },
            "replace" => {
                let value = op
                    .get("value")
                    .ok_or_else(|| ApiserverError::InvalidInput {
                        field: "patch.value".to_string(),
                        reason: "'replace' operation requires 'value'".to_string(),
                    })?;
                patch_replace(target, path, value.clone())?;
            },
            "remove" => {
                patch_remove(target, path)?;
            },
            other => {
                return Err(ApiserverError::InvalidInput {
                    field: "patch.op".to_string(),
                    reason: format!("unsupported patch operation: {other}"),
                });
            },
        }
    }

    Ok(())
}

fn split_pointer(path: &str) -> Vec<String> {
    if path == "/" || path.is_empty() {
        return Vec::new();
    }
    let trimmed = path.strip_prefix('/').unwrap_or(path);
    trimmed
        .split('/')
        .map(|seg| seg.replace("~1", "/").replace("~0", "~"))
        .collect()
}

fn patch_add(target: &mut Value, path: &str, value: Value) -> Result<(), ApiserverError> {
    let tokens = split_pointer(path);
    if tokens.is_empty() {
        *target = value;
        return Ok(());
    }

    let mut current = target;
    for (i, token) in tokens.iter().enumerate() {
        let is_last = i == tokens.len() - 1;
        if is_last {
            match current {
                Value::Object(map) => {
                    map.insert(token.clone(), value);
                    return Ok(());
                },
                Value::Array(arr) => {
                    if token == "-" {
                        arr.push(value);
                        return Ok(());
                    }
                    if let Ok(idx) = token.parse::<usize>() {
                        if idx <= arr.len() {
                            arr.insert(idx, value);
                            return Ok(());
                        }
                    }
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: format!("array index {token} out of bounds"),
                    });
                },
                _ => {
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: "cannot add property to non-object/non-array".to_string(),
                    });
                },
            }
        } else {
            // Navigate or create intermediate object
            if current.is_null() {
                *current = Value::Object(serde_json::Map::new());
            }
            match current {
                Value::Object(map) => {
                    current = map
                        .entry(token.clone())
                        .or_insert_with(|| Value::Object(serde_json::Map::new()));
                },
                Value::Array(arr) => {
                    let idx = token
                        .parse::<usize>()
                        .map_err(|_| ApiserverError::InvalidInput {
                            field: path.to_string(),
                            reason: format!("invalid array index {token}"),
                        })?;
                    if idx < arr.len() {
                        current = &mut arr[idx];
                    } else {
                        return Err(ApiserverError::InvalidInput {
                            field: path.to_string(),
                            reason: format!("array index {idx} out of bounds"),
                        });
                    }
                },
                _ => {
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: "cannot navigate into non-container".to_string(),
                    });
                },
            }
        }
    }
    Ok(())
}

fn patch_replace(target: &mut Value, path: &str, value: Value) -> Result<(), ApiserverError> {
    let tokens = split_pointer(path);
    if tokens.is_empty() {
        *target = value;
        return Ok(());
    }

    let mut current = target;
    for (i, token) in tokens.iter().enumerate() {
        let is_last = i == tokens.len() - 1;
        if is_last {
            match current {
                Value::Object(map) => {
                    if map.contains_key(token) {
                        map.insert(token.clone(), value);
                        return Ok(());
                    }
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: format!("key '{token}' does not exist for replace"),
                    });
                },
                Value::Array(arr) => {
                    let idx = token
                        .parse::<usize>()
                        .map_err(|_| ApiserverError::InvalidInput {
                            field: path.to_string(),
                            reason: format!("invalid array index {token}"),
                        })?;
                    if idx < arr.len() {
                        arr[idx] = value;
                        return Ok(());
                    }
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: format!("array index {idx} out of bounds for replace"),
                    });
                },
                _ => {
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: "cannot replace on non-container".to_string(),
                    });
                },
            }
        } else {
            match current {
                Value::Object(map) => {
                    current = map
                        .get_mut(token)
                        .ok_or_else(|| ApiserverError::InvalidInput {
                            field: path.to_string(),
                            reason: format!("path component '{token}' not found"),
                        })?;
                },
                Value::Array(arr) => {
                    let idx = token
                        .parse::<usize>()
                        .map_err(|_| ApiserverError::InvalidInput {
                            field: path.to_string(),
                            reason: format!("invalid array index {token}"),
                        })?;
                    if idx < arr.len() {
                        current = &mut arr[idx];
                    } else {
                        return Err(ApiserverError::InvalidInput {
                            field: path.to_string(),
                            reason: format!("array index {idx} out of bounds"),
                        });
                    }
                },
                _ => {
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: "cannot navigate into non-container".to_string(),
                    });
                },
            }
        }
    }
    Ok(())
}

fn patch_remove(target: &mut Value, path: &str) -> Result<(), ApiserverError> {
    let tokens = split_pointer(path);
    if tokens.is_empty() {
        return Err(ApiserverError::InvalidInput {
            field: path.to_string(),
            reason: "cannot remove root object".to_string(),
        });
    }

    let mut current = target;
    for (i, token) in tokens.iter().enumerate() {
        let is_last = i == tokens.len() - 1;
        if is_last {
            match current {
                Value::Object(map) => {
                    if map.remove(token).is_some() {
                        return Ok(());
                    }
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: format!("key '{token}' does not exist for remove"),
                    });
                },
                Value::Array(arr) => {
                    let idx = token
                        .parse::<usize>()
                        .map_err(|_| ApiserverError::InvalidInput {
                            field: path.to_string(),
                            reason: format!("invalid array index {token}"),
                        })?;
                    if idx < arr.len() {
                        arr.remove(idx);
                        return Ok(());
                    }
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: format!("array index {idx} out of bounds for remove"),
                    });
                },
                _ => {
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: "cannot remove on non-container".to_string(),
                    });
                },
            }
        } else {
            match current {
                Value::Object(map) => {
                    current = map
                        .get_mut(token)
                        .ok_or_else(|| ApiserverError::InvalidInput {
                            field: path.to_string(),
                            reason: format!("path component '{token}' not found"),
                        })?;
                },
                Value::Array(arr) => {
                    let idx = token
                        .parse::<usize>()
                        .map_err(|_| ApiserverError::InvalidInput {
                            field: path.to_string(),
                            reason: format!("invalid array index {token}"),
                        })?;
                    if idx < arr.len() {
                        current = &mut arr[idx];
                    } else {
                        return Err(ApiserverError::InvalidInput {
                            field: path.to_string(),
                            reason: format!("array index {idx} out of bounds"),
                        });
                    }
                },
                _ => {
                    return Err(ApiserverError::InvalidInput {
                        field: path.to_string(),
                        reason: "cannot navigate into non-container".to_string(),
                    });
                },
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_patch_operations() {
        let mut doc = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": "test-pod",
                "labels": {
                    "env": "dev"
                }
            }
        });

        // Add
        let patch_add = json!([
            { "op": "add", "path": "/metadata/labels/team", "value": "core" },
            { "op": "add", "path": "/spec", "value": { "nodeName": "node-1" } }
        ]);
        apply_json_patch(&mut doc, serde_json::to_vec(&patch_add).unwrap().as_slice()).unwrap();
        assert_eq!(doc["metadata"]["labels"]["team"], "core");
        assert_eq!(doc["spec"]["nodeName"], "node-1");

        // Replace
        let patch_rep = json!([
            { "op": "replace", "path": "/metadata/labels/env", "value": "prod" }
        ]);
        apply_json_patch(&mut doc, serde_json::to_vec(&patch_rep).unwrap().as_slice()).unwrap();
        assert_eq!(doc["metadata"]["labels"]["env"], "prod");

        // Remove
        let patch_rem = json!([
            { "op": "remove", "path": "/metadata/labels/team" }
        ]);
        apply_json_patch(&mut doc, serde_json::to_vec(&patch_rem).unwrap().as_slice()).unwrap();
        assert!(doc["metadata"]["labels"].get("team").is_none());
    }
}
