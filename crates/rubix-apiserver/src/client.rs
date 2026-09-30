use serde_json::{Value, json};
use std::collections::BTreeMap;

use rubix_datastore::WatchReceiver;

use crate::config::ApiserverConfig;
use crate::error::ApiserverError;
use crate::storage::KubernetesStorage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientIdentity {
    AdminCertificate,
    BearerToken(String),
    RestrictedUser {
        username: String,
        groups: Vec<String>,
    },
    Anonymous,
}

#[derive(Clone, Debug)]
pub struct KubernetesApiClient {
    storage: KubernetesStorage,
    identity: ClientIdentity,
    anonymous_auth_allowed: bool,
}

impl KubernetesApiClient {
    pub fn new(
        storage: KubernetesStorage,
        identity: ClientIdentity,
        config: &ApiserverConfig,
    ) -> Self {
        Self {
            storage,
            identity,
            anonymous_auth_allowed: config.anonymous_auth,
        }
    }

    fn check_auth(&self, verb: &str, resource: &str) -> Result<(), ApiserverError> {
        match &self.identity {
            ClientIdentity::AdminCertificate => Ok(()),
            ClientIdentity::BearerToken(token) => {
                if token == "admin-token" || token.starts_with("system:admin") {
                    Ok(())
                } else {
                    Err(ApiserverError::Unauthorized {
                        reason: format!("token '{token}' is unauthorized for {verb} on {resource}"),
                    })
                }
            },
            ClientIdentity::RestrictedUser { username, .. } => {
                // Restricted user is forbidden from cluster modifications unless granted by RBAC
                if (verb == "get" || verb == "list") && resource == "namespaces" {
                    return Ok(());
                }
                Err(ApiserverError::Unauthorized {
                    reason: format!("user '{username}' is forbidden from {verb} on {resource}"),
                })
            },
            ClientIdentity::Anonymous => {
                if self.anonymous_auth_allowed {
                    Err(ApiserverError::Unauthorized {
                        reason: "anonymous user cannot perform actions in the cluster".to_string(),
                    })
                } else {
                    Err(ApiserverError::Unauthenticated {
                        reason: "anonymous requests are disabled (--anonymous-auth=false)"
                            .to_string(),
                    })
                }
            },
        }
    }

    // --- Discovery API ---

    pub fn discover_core(&self) -> Result<Value, ApiserverError> {
        self.check_auth("get", "discovery")?;
        Ok(json!({
            "kind": "APIResourceList",
            "apiVersion": "v1",
            "groupVersion": "v1",
            "resources": [
                {
                    "name": "namespaces",
                    "singularName": "namespace",
                    "namespaced": false,
                    "kind": "Namespace",
                    "verbs": ["create", "delete", "get", "list", "watch"]
                },
                {
                    "name": "configmaps",
                    "singularName": "configmap",
                    "namespaced": true,
                    "kind": "ConfigMap",
                    "verbs": ["create", "delete", "get", "list", "update", "watch"]
                },
                {
                    "name": "secrets",
                    "singularName": "secret",
                    "namespaced": true,
                    "kind": "Secret",
                    "verbs": ["create", "delete", "get", "list", "update", "watch"]
                },
                {
                    "name": "pods",
                    "singularName": "pod",
                    "namespaced": true,
                    "kind": "Pod",
                    "verbs": ["create", "delete", "get", "list", "update", "watch"]
                }
            ]
        }))
    }

    pub fn discover_apis(&self) -> Result<Value, ApiserverError> {
        self.check_auth("get", "discovery")?;
        Ok(json!({
            "kind": "APIGroupList",
            "apiVersion": "v1",
            "groups": [
                {
                    "name": "apps",
                    "versions": [
                        {
                            "groupVersion": "apps/v1",
                            "version": "v1"
                        }
                    ],
                    "preferredVersion": {
                        "groupVersion": "apps/v1",
                        "version": "v1"
                    }
                },
                {
                    "name": "apiextensions.k8s.io",
                    "versions": [
                        {
                            "groupVersion": "apiextensions.k8s.io/v1",
                            "version": "v1"
                        }
                    ],
                    "preferredVersion": {
                        "groupVersion": "apiextensions.k8s.io/v1",
                        "version": "v1"
                    }
                }
            ]
        }))
    }

    // --- Namespace CRUD ---

    pub async fn create_namespace(&self, name: &str) -> Result<Value, ApiserverError> {
        self.check_auth("create", "namespaces")?;
        let key = format!("{}/namespaces/{name}", self.storage.prefix());
        let doc = json!({
            "apiVersion": "v1",
            "kind": "Namespace",
            "metadata": {
                "name": name,
                "creationTimestamp": "2026-09-30T00:00:00Z"
            },
            "status": {
                "phase": "Active"
            }
        });
        let bytes = serde_json::to_vec(&doc)?;
        let kv = self.storage.create(&key, bytes).await?;
        let mut result = doc;
        if let Some(meta) = result.get_mut("metadata").and_then(Value::as_object_mut) {
            meta.insert(
                "resourceVersion".to_string(),
                json!(kv.mod_revision.to_string()),
            );
        }
        Ok(result)
    }

    pub async fn get_namespace(&self, name: &str) -> Result<Value, ApiserverError> {
        self.check_auth("get", "namespaces")?;
        let key = format!("{}/namespaces/{name}", self.storage.prefix());
        let kv = self
            .storage
            .get(&key)
            .await?
            .ok_or_else(|| ApiserverError::NotFound {
                resource: "namespaces".to_string(),
                name: name.to_string(),
            })?;
        let mut doc: Value = serde_json::from_slice(&kv.value)?;
        if let Some(meta) = doc.get_mut("metadata").and_then(Value::as_object_mut) {
            meta.insert(
                "resourceVersion".to_string(),
                json!(kv.mod_revision.to_string()),
            );
        }
        Ok(doc)
    }

    pub async fn list_namespaces(&self) -> Result<Value, ApiserverError> {
        self.check_auth("list", "namespaces")?;
        let prefix = format!("{}/namespaces/", self.storage.prefix());
        let kvs = self.storage.list(&prefix).await?;
        let mut items = Vec::new();
        for kv in kvs {
            let mut doc: Value = serde_json::from_slice(&kv.value)?;
            if let Some(meta) = doc.get_mut("metadata").and_then(Value::as_object_mut) {
                meta.insert(
                    "resourceVersion".to_string(),
                    json!(kv.mod_revision.to_string()),
                );
            }
            items.push(doc);
        }
        let cur_rev = self.storage.current_revision().await;
        Ok(json!({
            "apiVersion": "v1",
            "kind": "NamespaceList",
            "metadata": {
                "resourceVersion": cur_rev.to_string()
            },
            "items": items
        }))
    }

    pub async fn delete_namespace(&self, name: &str) -> Result<(), ApiserverError> {
        self.check_auth("delete", "namespaces")?;
        let key = format!("{}/namespaces/{name}", self.storage.prefix());
        let res = self.storage.delete(&key, None).await?;
        if res.is_none() {
            return Err(ApiserverError::NotFound {
                resource: "namespaces".to_string(),
                name: name.to_string(),
            });
        }
        Ok(())
    }

    // --- ConfigMap CRUD ---

    pub async fn create_configmap(
        &self,
        namespace: &str,
        name: &str,
        data: BTreeMap<String, String>,
    ) -> Result<Value, ApiserverError> {
        self.check_auth("create", "configmaps")?;
        let key = format!("{}/configmaps/{namespace}/{name}", self.storage.prefix());
        let doc = json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": {
                "name": name,
                "namespace": namespace,
                "creationTimestamp": "2026-09-30T00:00:00Z"
            },
            "data": data
        });
        let bytes = serde_json::to_vec(&doc)?;
        let kv = self.storage.create(&key, bytes).await?;
        let mut result = doc;
        if let Some(meta) = result.get_mut("metadata").and_then(Value::as_object_mut) {
            meta.insert(
                "resourceVersion".to_string(),
                json!(kv.mod_revision.to_string()),
            );
        }
        Ok(result)
    }

    pub async fn get_configmap(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Value, ApiserverError> {
        self.check_auth("get", "configmaps")?;
        let key = format!("{}/configmaps/{namespace}/{name}", self.storage.prefix());
        let kv = self
            .storage
            .get(&key)
            .await?
            .ok_or_else(|| ApiserverError::NotFound {
                resource: "configmaps".to_string(),
                name: format!("{namespace}/{name}"),
            })?;
        let mut doc: Value = serde_json::from_slice(&kv.value)?;
        if let Some(meta) = doc.get_mut("metadata").and_then(Value::as_object_mut) {
            meta.insert(
                "resourceVersion".to_string(),
                json!(kv.mod_revision.to_string()),
            );
        }
        Ok(doc)
    }

    pub async fn update_configmap(
        &self,
        namespace: &str,
        name: &str,
        data: BTreeMap<String, String>,
        expected_mod_revision: Option<u64>,
    ) -> Result<Value, ApiserverError> {
        self.check_auth("update", "configmaps")?;
        let key = format!("{}/configmaps/{namespace}/{name}", self.storage.prefix());
        let doc = json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": {
                "name": name,
                "namespace": namespace,
                "creationTimestamp": "2026-09-30T00:00:00Z"
            },
            "data": data
        });
        let bytes = serde_json::to_vec(&doc)?;
        let kv = self
            .storage
            .update(&key, bytes, expected_mod_revision)
            .await?;
        let mut result = doc;
        if let Some(meta) = result.get_mut("metadata").and_then(Value::as_object_mut) {
            meta.insert(
                "resourceVersion".to_string(),
                json!(kv.mod_revision.to_string()),
            );
        }
        Ok(result)
    }

    pub async fn delete_configmap(
        &self,
        namespace: &str,
        name: &str,
        expected_mod_revision: Option<u64>,
    ) -> Result<(), ApiserverError> {
        self.check_auth("delete", "configmaps")?;
        let key = format!("{}/configmaps/{namespace}/{name}", self.storage.prefix());
        let res = self.storage.delete(&key, expected_mod_revision).await?;
        if res.is_none() {
            return Err(ApiserverError::NotFound {
                resource: "configmaps".to_string(),
                name: format!("{namespace}/{name}"),
            });
        }
        Ok(())
    }

    pub async fn list_configmaps(&self, namespace: &str) -> Result<Value, ApiserverError> {
        self.check_auth("list", "configmaps")?;
        let prefix = format!("{}/configmaps/{namespace}/", self.storage.prefix());
        let kvs = self.storage.list(&prefix).await?;
        let mut items = Vec::new();
        for kv in kvs {
            let mut doc: Value = serde_json::from_slice(&kv.value)?;
            if let Some(meta) = doc.get_mut("metadata").and_then(Value::as_object_mut) {
                meta.insert(
                    "resourceVersion".to_string(),
                    json!(kv.mod_revision.to_string()),
                );
            }
            items.push(doc);
        }
        let cur_rev = self.storage.current_revision().await;
        Ok(json!({
            "apiVersion": "v1",
            "kind": "ConfigMapList",
            "metadata": {
                "resourceVersion": cur_rev.to_string()
            },
            "items": items
        }))
    }

    // --- CRD & Dynamic Custom Resource Support ---

    pub async fn create_crd(&self, crd: Value) -> Result<Value, ApiserverError> {
        self.check_auth("create", "customresourcedefinitions")?;
        let name = crd
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| ApiserverError::BadRequest {
                message: "CRD metadata.name is required".to_string(),
            })?;
        let key = format!("{}/customresourcedefinitions/{name}", self.storage.prefix());
        let bytes = serde_json::to_vec(&crd)?;
        let kv = self.storage.create(&key, bytes).await?;
        let mut result = crd;
        if let Some(meta) = result.get_mut("metadata").and_then(Value::as_object_mut) {
            meta.insert(
                "resourceVersion".to_string(),
                json!(kv.mod_revision.to_string()),
            );
        }
        Ok(result)
    }

    pub async fn get_crd(&self, name: &str) -> Result<Value, ApiserverError> {
        self.check_auth("get", "customresourcedefinitions")?;
        let key = format!("{}/customresourcedefinitions/{name}", self.storage.prefix());
        let kv = self
            .storage
            .get(&key)
            .await?
            .ok_or_else(|| ApiserverError::NotFound {
                resource: "customresourcedefinitions".to_string(),
                name: name.to_string(),
            })?;
        let mut doc: Value = serde_json::from_slice(&kv.value)?;
        if let Some(meta) = doc.get_mut("metadata").and_then(Value::as_object_mut) {
            meta.insert(
                "resourceVersion".to_string(),
                json!(kv.mod_revision.to_string()),
            );
        }
        Ok(doc)
    }

    pub async fn list_crds(&self) -> Result<Value, ApiserverError> {
        self.check_auth("list", "customresourcedefinitions")?;
        let prefix = format!("{}/customresourcedefinitions/", self.storage.prefix());
        let kvs = self.storage.list(&prefix).await?;
        let mut items = Vec::new();
        for kv in kvs {
            let mut doc: Value = serde_json::from_slice(&kv.value)?;
            if let Some(meta) = doc.get_mut("metadata").and_then(Value::as_object_mut) {
                meta.insert(
                    "resourceVersion".to_string(),
                    json!(kv.mod_revision.to_string()),
                );
            }
            items.push(doc);
        }
        let cur_rev = self.storage.current_revision().await;
        Ok(json!({
            "apiVersion": "apiextensions.k8s.io/v1",
            "kind": "CustomResourceDefinitionList",
            "metadata": {
                "resourceVersion": cur_rev.to_string()
            },
            "items": items
        }))
    }

    pub async fn delete_crd(&self, name: &str) -> Result<(), ApiserverError> {
        self.check_auth("delete", "customresourcedefinitions")?;
        let key = format!("{}/customresourcedefinitions/{name}", self.storage.prefix());
        let res = self.storage.delete(&key, None).await?;
        if res.is_none() {
            return Err(ApiserverError::NotFound {
                resource: "customresourcedefinitions".to_string(),
                name: name.to_string(),
            });
        }
        Ok(())
    }

    // --- Dynamic Custom Resources ---

    pub async fn create_custom_resource(
        &self,
        group: &str,
        plural: &str,
        namespace: &str,
        name: &str,
        resource: Value,
    ) -> Result<Value, ApiserverError> {
        self.check_auth("create", plural)?;
        let key = format!(
            "{}/{group}/{plural}/{namespace}/{name}",
            self.storage.prefix()
        );
        let bytes = serde_json::to_vec(&resource)?;
        let kv = self.storage.create(&key, bytes).await?;
        let mut result = resource;
        if let Some(meta) = result.get_mut("metadata").and_then(Value::as_object_mut) {
            meta.insert(
                "resourceVersion".to_string(),
                json!(kv.mod_revision.to_string()),
            );
        }
        Ok(result)
    }

    pub async fn get_custom_resource(
        &self,
        group: &str,
        plural: &str,
        namespace: &str,
        name: &str,
    ) -> Result<Value, ApiserverError> {
        self.check_auth("get", plural)?;
        let key = format!(
            "{}/{group}/{plural}/{namespace}/{name}",
            self.storage.prefix()
        );
        let kv = self
            .storage
            .get(&key)
            .await?
            .ok_or_else(|| ApiserverError::NotFound {
                resource: plural.to_string(),
                name: format!("{namespace}/{name}"),
            })?;
        let mut doc: Value = serde_json::from_slice(&kv.value)?;
        if let Some(meta) = doc.get_mut("metadata").and_then(Value::as_object_mut) {
            meta.insert(
                "resourceVersion".to_string(),
                json!(kv.mod_revision.to_string()),
            );
        }
        Ok(doc)
    }

    // --- Watch Streaming ---

    pub async fn watch(&self, prefix: &str) -> Result<WatchReceiver, ApiserverError> {
        self.check_auth("watch", prefix)?;
        let full_prefix = format!(
            "{}/{}",
            self.storage.prefix(),
            prefix.trim_start_matches('/')
        );
        Ok(self.storage.watch(&full_prefix).await)
    }
}
