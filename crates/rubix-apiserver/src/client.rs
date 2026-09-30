use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};

use rubix_datastore::WatchReceiver;

use crate::config::ApiserverConfig;
use crate::error::ApiserverError;
use crate::rbac::{
    AuthzRequest, ClusterRole, ClusterRoleBinding, PolicyRule, RbacAuthorizer, Role, RoleBinding,
};
use crate::storage::KubernetesStorage;
use crate::token::TokenService;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientIdentity {
    AdminCertificate,
    BearerToken(String),
    ServiceAccount {
        namespace: String,
        name: String,
    },
    User {
        username: String,
        groups: Vec<String>,
    },
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
    rbac: Arc<RbacAuthorizer>,
    token_service: Option<Arc<TokenService>>,
    anonymous_auth_allowed: bool,
}

impl KubernetesApiClient {
    pub fn new(
        storage: KubernetesStorage,
        identity: ClientIdentity,
        rbac: Arc<RbacAuthorizer>,
        token_service: Option<Arc<TokenService>>,
        config: &ApiserverConfig,
    ) -> Self {
        Self {
            storage,
            identity,
            rbac,
            token_service,
            anonymous_auth_allowed: config.anonymous_auth,
        }
    }

    #[must_use]
    pub fn identity(&self) -> &ClientIdentity {
        &self.identity
    }

    fn check_rbac_permission(&self, req: &AuthzRequest<'_>) -> Result<(), ApiserverError> {
        if self.rbac.authorize(req) {
            Ok(())
        } else {
            Err(ApiserverError::Unauthorized {
                reason: format!(
                    "RBAC: {} is forbidden to {} {} in namespace {:?}",
                    req.username, req.verb, req.resource, req.namespace
                ),
            })
        }
    }

    fn check_token_auth(&self, token: &str, req: &AuthzRequest<'_>) -> Result<(), ApiserverError> {
        if let Some(ts) = &self.token_service {
            let claims = ts.verify_token(token, Some("https://kubernetes.default.svc"))?;
            let groups = vec![
                "system:serviceaccounts".to_string(),
                format!("system:serviceaccounts:{}", claims.namespace),
                "system:authenticated".to_string(),
            ];
            self.check_rbac_permission(&AuthzRequest {
                username: &claims.sub,
                groups: &groups,
                ..*req
            })
        } else {
            Err(ApiserverError::Unauthorized {
                reason: format!(
                    "bearer token cannot be verified (token service unavailable) for {} on {}",
                    req.verb, req.resource
                ),
            })
        }
    }

    pub fn check_auth_detailed(
        &self,
        verb: &str,
        api_group: &str,
        resource: &str,
        namespace: Option<&str>,
        resource_name: Option<&str>,
    ) -> Result<(), ApiserverError> {
        let base_req = AuthzRequest {
            username: "",
            groups: &[],
            verb,
            api_group,
            resource,
            namespace,
            resource_name,
        };

        match &self.identity {
            ClientIdentity::AdminCertificate => Ok(()),
            ClientIdentity::BearerToken(token) => self.check_token_auth(token, &base_req),
            ClientIdentity::ServiceAccount {
                namespace: sa_ns,
                name: sa_name,
            } => {
                let username = format!("system:serviceaccount:{sa_ns}:{sa_name}");
                let groups = vec![
                    "system:serviceaccounts".to_string(),
                    format!("system:serviceaccounts:{sa_ns}"),
                    "system:authenticated".to_string(),
                ];
                self.check_rbac_permission(&AuthzRequest {
                    username: &username,
                    groups: &groups,
                    ..base_req
                })
            },
            ClientIdentity::User { username, groups } => {
                self.check_rbac_permission(&AuthzRequest {
                    username,
                    groups,
                    ..base_req
                })
            },
            ClientIdentity::RestrictedUser { username, groups } => {
                if self
                    .check_rbac_permission(&AuthzRequest {
                        username,
                        groups,
                        ..base_req
                    })
                    .is_ok()
                {
                    return Ok(());
                }

                // Default fallback: allow read on namespaces for restricted users
                if (verb == "get" || verb == "list") && resource == "namespaces" {
                    return Ok(());
                }

                Err(ApiserverError::Unauthorized {
                    reason: format!("user '{username}' is forbidden from {verb} on {resource}"),
                })
            },
            ClientIdentity::Anonymous => {
                if self.anonymous_auth_allowed {
                    let groups = vec!["system:unauthenticated".to_string()];
                    self.check_rbac_permission(&AuthzRequest {
                        username: "system:anonymous",
                        groups: &groups,
                        ..base_req
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

    pub fn check_auth(&self, verb: &str, resource: &str) -> Result<(), ApiserverError> {
        self.check_auth_detailed(verb, "", resource, None, None)
    }

    // --- Discovery API ---

    pub fn discover_core(&self) -> Result<Value, ApiserverError> {
        self.check_auth_detailed("get", "", "discovery", None, None)?;
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
        self.check_auth_detailed("get", "", "discovery", None, None)?;
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
                },
                {
                    "name": "rbac.authorization.k8s.io",
                    "versions": [
                        {
                            "groupVersion": "rbac.authorization.k8s.io/v1",
                            "version": "v1"
                        }
                    ],
                    "preferredVersion": {
                        "groupVersion": "rbac.authorization.k8s.io/v1",
                        "version": "v1"
                    }
                }
            ]
        }))
    }

    // --- Namespace CRUD ---

    pub async fn create_namespace(&self, name: &str) -> Result<Value, ApiserverError> {
        self.check_auth_detailed("create", "", "namespaces", None, Some(name))?;
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
        self.check_auth_detailed("get", "", "namespaces", None, Some(name))?;
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
        self.check_auth_detailed("list", "", "namespaces", None, None)?;
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
        self.check_auth_detailed("delete", "", "namespaces", None, Some(name))?;
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
        self.check_auth_detailed("create", "", "configmaps", Some(namespace), Some(name))?;
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
        self.check_auth_detailed("get", "", "configmaps", Some(namespace), Some(name))?;
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
        expected_version: Option<u64>,
    ) -> Result<Value, ApiserverError> {
        self.check_auth_detailed("update", "", "configmaps", Some(namespace), Some(name))?;
        let key = format!("{}/configmaps/{namespace}/{name}", self.storage.prefix());
        let doc = json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": {
                "name": name,
                "namespace": namespace
            },
            "data": data
        });
        let bytes = serde_json::to_vec(&doc)?;
        let kv = self.storage.update(&key, bytes, expected_version).await?;
        let mut result = doc;
        if let Some(meta) = result.get_mut("metadata").and_then(Value::as_object_mut) {
            meta.insert(
                "resourceVersion".to_string(),
                json!(kv.mod_revision.to_string()),
            );
        }
        Ok(result)
    }

    pub async fn list_configmaps(&self, namespace: &str) -> Result<Value, ApiserverError> {
        self.check_auth_detailed("list", "", "configmaps", Some(namespace), None)?;
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

    pub async fn delete_configmap(
        &self,
        namespace: &str,
        name: &str,
        expected_version: Option<u64>,
    ) -> Result<(), ApiserverError> {
        self.check_auth_detailed("delete", "", "configmaps", Some(namespace), Some(name))?;
        let key = format!("{}/configmaps/{namespace}/{name}", self.storage.prefix());
        let res = self.storage.delete(&key, expected_version).await?;
        if res.is_none() {
            return Err(ApiserverError::NotFound {
                resource: "configmaps".to_string(),
                name: format!("{namespace}/{name}"),
            });
        }
        Ok(())
    }

    // --- CustomResourceDefinition CRUD ---

    pub async fn create_crd(&self, crd: Value) -> Result<Value, ApiserverError> {
        let name = crd
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| ApiserverError::InvalidInput {
                field: "metadata.name".to_string(),
                reason: "CustomResourceDefinition requires metadata.name".to_string(),
            })?;

        self.check_auth_detailed(
            "create",
            "apiextensions.k8s.io",
            "customresourcedefinitions",
            None,
            Some(name),
        )?;

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
        self.check_auth_detailed(
            "get",
            "apiextensions.k8s.io",
            "customresourcedefinitions",
            None,
            Some(name),
        )?;
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
        self.check_auth_detailed(
            "list",
            "apiextensions.k8s.io",
            "customresourcedefinitions",
            None,
            None,
        )?;
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
        self.check_auth_detailed(
            "delete",
            "apiextensions.k8s.io",
            "customresourcedefinitions",
            None,
            Some(name),
        )?;
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
        self.check_auth_detailed("create", group, plural, Some(namespace), Some(name))?;
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
        self.check_auth_detailed("get", group, plural, Some(namespace), Some(name))?;
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

    // --- RBAC API Management ---

    fn verify_caller_holds_rule(
        &self,
        rule: &PolicyRule,
        namespace: Option<&str>,
    ) -> Result<(), ApiserverError> {
        let names: Vec<Option<&str>> = if rule.resource_names.is_empty() {
            vec![None]
        } else {
            rule.resource_names
                .iter()
                .map(|n| Some(n.as_str()))
                .collect()
        };

        for verb in &rule.verbs {
            for group in &rule.api_groups {
                self.verify_caller_holds_verb_group(
                    verb,
                    group,
                    &rule.resources,
                    &names,
                    namespace,
                )?;
            }
        }
        Ok(())
    }

    fn verify_caller_holds_verb_group(
        &self,
        verb: &str,
        group: &str,
        resources: &[String],
        names: &[Option<&str>],
        namespace: Option<&str>,
    ) -> Result<(), ApiserverError> {
        for res in resources {
            for name in names {
                self.check_auth_detailed(verb, group, res, namespace, *name)?;
            }
        }
        Ok(())
    }

    fn verify_caller_holds_rules(
        &self,
        rules: &[PolicyRule],
        namespace: Option<&str>,
    ) -> Result<(), ApiserverError> {
        for rule in rules {
            self.verify_caller_holds_rule(rule, namespace)?;
        }
        Ok(())
    }

    fn verify_cluster_role_creation_privileges(
        &self,
        cr: &ClusterRole,
    ) -> Result<(), ApiserverError> {
        if self.identity == ClientIdentity::AdminCertificate {
            return Ok(());
        }

        let has_escalate = self
            .check_auth_detailed(
                "escalate",
                "rbac.authorization.k8s.io",
                "clusterroles",
                None,
                Some(&cr.name),
            )
            .is_ok();

        if has_escalate {
            return Ok(());
        }

        self.verify_caller_holds_rules(&cr.rules, None)
    }

    fn verify_cluster_role_binding_privileges(&self, role_ref: &str) -> Result<(), ApiserverError> {
        if self.identity == ClientIdentity::AdminCertificate {
            return Ok(());
        }

        let has_bind = self
            .check_auth_detailed(
                "bind",
                "rbac.authorization.k8s.io",
                "clusterroles",
                None,
                Some(role_ref),
            )
            .is_ok();

        if has_bind {
            return Ok(());
        }

        let rules = if let Some(cr) = self.rbac.get_cluster_role(role_ref) {
            cr.rules
        } else {
            return Err(ApiserverError::InvalidInput {
                field: "role_ref".to_string(),
                reason: format!("referenced ClusterRole '{role_ref}' does not exist"),
            });
        };

        self.verify_caller_holds_rules(&rules, None)
    }

    fn verify_role_creation_privileges(
        &self,
        namespace: &str,
        role: &Role,
    ) -> Result<(), ApiserverError> {
        if self.identity == ClientIdentity::AdminCertificate {
            return Ok(());
        }

        let has_escalate = self
            .check_auth_detailed(
                "escalate",
                "rbac.authorization.k8s.io",
                "roles",
                Some(namespace),
                Some(&role.name),
            )
            .is_ok();

        if has_escalate {
            return Ok(());
        }

        self.verify_caller_holds_rules(&role.rules, Some(namespace))
    }

    fn verify_role_binding_privileges(
        &self,
        namespace: &str,
        role_ref: &str,
    ) -> Result<(), ApiserverError> {
        if self.identity == ClientIdentity::AdminCertificate {
            return Ok(());
        }

        let has_bind = self
            .check_auth_detailed(
                "bind",
                "rbac.authorization.k8s.io",
                "roles",
                Some(namespace),
                Some(role_ref),
            )
            .is_ok()
            || self
                .check_auth_detailed(
                    "bind",
                    "rbac.authorization.k8s.io",
                    "clusterroles",
                    None,
                    Some(role_ref),
                )
                .is_ok();

        if has_bind {
            return Ok(());
        }

        let rules = if let Some(r) = self.rbac.get_role(namespace, role_ref) {
            r.rules
        } else if let Some(cr) = self.rbac.get_cluster_role(role_ref) {
            cr.rules
        } else {
            return Err(ApiserverError::InvalidInput {
                field: "role_ref".to_string(),
                reason: format!("referenced role '{role_ref}' does not exist"),
            });
        };

        self.verify_caller_holds_rules(&rules, Some(namespace))
    }

    pub async fn create_cluster_role(
        &self,
        role: ClusterRole,
    ) -> Result<ClusterRole, ApiserverError> {
        self.check_auth_detailed(
            "create",
            "rbac.authorization.k8s.io",
            "clusterroles",
            None,
            Some(&role.name),
        )?;
        self.verify_cluster_role_creation_privileges(&role)?;
        let key = format!("{}/clusterroles/{}", self.storage.prefix(), role.name);
        let bytes = serde_json::to_vec(&role)?;
        self.storage.create(&key, bytes).await?;
        self.rbac.add_cluster_role(role.clone());
        Ok(role)
    }

    pub async fn get_cluster_role(&self, name: &str) -> Result<ClusterRole, ApiserverError> {
        self.check_auth_detailed(
            "get",
            "rbac.authorization.k8s.io",
            "clusterroles",
            None,
            Some(name),
        )?;
        if let Some(role) = self.rbac.get_cluster_role(name) {
            return Ok(role);
        }
        let key = format!("{}/clusterroles/{name}", self.storage.prefix());
        let kv = self
            .storage
            .get(&key)
            .await?
            .ok_or_else(|| ApiserverError::NotFound {
                resource: "clusterroles".to_string(),
                name: name.to_string(),
            })?;
        let role: ClusterRole = serde_json::from_slice(&kv.value)?;
        self.rbac.add_cluster_role(role.clone());
        Ok(role)
    }

    pub async fn create_cluster_role_binding(
        &self,
        binding: ClusterRoleBinding,
    ) -> Result<ClusterRoleBinding, ApiserverError> {
        self.check_auth_detailed(
            "create",
            "rbac.authorization.k8s.io",
            "clusterrolebindings",
            None,
            Some(&binding.name),
        )?;
        self.verify_cluster_role_binding_privileges(&binding.role_ref)?;
        let key = format!(
            "{}/clusterrolebindings/{}",
            self.storage.prefix(),
            binding.name
        );
        let bytes = serde_json::to_vec(&binding)?;
        self.storage.create(&key, bytes).await?;
        self.rbac.add_cluster_role_binding(binding.clone());
        Ok(binding)
    }

    pub async fn get_cluster_role_binding(
        &self,
        name: &str,
    ) -> Result<ClusterRoleBinding, ApiserverError> {
        self.check_auth_detailed(
            "get",
            "rbac.authorization.k8s.io",
            "clusterrolebindings",
            None,
            Some(name),
        )?;
        if let Some(binding) = self.rbac.get_cluster_role_binding(name) {
            return Ok(binding);
        }
        let key = format!("{}/clusterrolebindings/{name}", self.storage.prefix());
        let kv = self
            .storage
            .get(&key)
            .await?
            .ok_or_else(|| ApiserverError::NotFound {
                resource: "clusterrolebindings".to_string(),
                name: name.to_string(),
            })?;
        let binding: ClusterRoleBinding = serde_json::from_slice(&kv.value)?;
        self.rbac.add_cluster_role_binding(binding.clone());
        Ok(binding)
    }

    pub async fn create_role(&self, role: Role) -> Result<Role, ApiserverError> {
        self.check_auth_detailed(
            "create",
            "rbac.authorization.k8s.io",
            "roles",
            Some(&role.namespace),
            Some(&role.name),
        )?;
        self.verify_role_creation_privileges(&role.namespace, &role)?;
        let key = format!(
            "{}/roles/{}/{}",
            self.storage.prefix(),
            role.namespace,
            role.name
        );
        let bytes = serde_json::to_vec(&role)?;
        self.storage.create(&key, bytes).await?;
        self.rbac.add_role(role.clone());
        Ok(role)
    }

    pub async fn get_role(&self, namespace: &str, name: &str) -> Result<Role, ApiserverError> {
        self.check_auth_detailed(
            "get",
            "rbac.authorization.k8s.io",
            "roles",
            Some(namespace),
            Some(name),
        )?;
        if let Some(role) = self.rbac.get_role(namespace, name) {
            return Ok(role);
        }
        let key = format!("{}/roles/{namespace}/{name}", self.storage.prefix());
        let kv = self
            .storage
            .get(&key)
            .await?
            .ok_or_else(|| ApiserverError::NotFound {
                resource: "roles".to_string(),
                name: format!("{namespace}/{name}"),
            })?;
        let role: Role = serde_json::from_slice(&kv.value)?;
        self.rbac.add_role(role.clone());
        Ok(role)
    }

    pub async fn create_role_binding(
        &self,
        binding: RoleBinding,
    ) -> Result<RoleBinding, ApiserverError> {
        self.check_auth_detailed(
            "create",
            "rbac.authorization.k8s.io",
            "rolebindings",
            Some(&binding.namespace),
            Some(&binding.name),
        )?;
        self.verify_role_binding_privileges(&binding.namespace, &binding.role_ref)?;
        let key = format!(
            "{}/rolebindings/{}/{}",
            self.storage.prefix(),
            binding.namespace,
            binding.name
        );
        let bytes = serde_json::to_vec(&binding)?;
        self.storage.create(&key, bytes).await?;
        self.rbac.add_role_binding(binding.clone());
        Ok(binding)
    }

    pub async fn get_role_binding(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<RoleBinding, ApiserverError> {
        self.check_auth_detailed(
            "get",
            "rbac.authorization.k8s.io",
            "rolebindings",
            Some(namespace),
            Some(name),
        )?;
        if let Some(binding) = self.rbac.get_role_binding(namespace, name) {
            return Ok(binding);
        }
        let key = format!("{}/rolebindings/{namespace}/{name}", self.storage.prefix());
        let kv = self
            .storage
            .get(&key)
            .await?
            .ok_or_else(|| ApiserverError::NotFound {
                resource: "rolebindings".to_string(),
                name: format!("{namespace}/{name}"),
            })?;
        let binding: RoleBinding = serde_json::from_slice(&kv.value)?;
        self.rbac.add_role_binding(binding.clone());
        Ok(binding)
    }

    // --- Watch Streaming ---

    pub async fn watch(&self, prefix: &str) -> Result<WatchReceiver, ApiserverError> {
        self.check_auth_detailed("watch", "", prefix, None, None)?;
        let full_prefix = format!(
            "{}/{}",
            self.storage.prefix(),
            prefix.trim_start_matches('/')
        );
        Ok(self.storage.watch(&full_prefix).await)
    }
}
