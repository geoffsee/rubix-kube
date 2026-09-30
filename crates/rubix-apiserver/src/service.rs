use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::client::{ClientIdentity, KubernetesApiClient};
use crate::config::ApiserverConfig;
use crate::error::ApiserverError;
use crate::health::{HealthReport, check_apiserver_readiness};
use crate::pki::validate_pki_prerequisites;
use crate::rbac::{ClusterRole, ClusterRoleBinding, RbacAuthorizer, Role, RoleBinding};
use crate::storage::KubernetesStorage;
use crate::token::{TokenProjection, TokenService};

#[derive(Clone, Debug)]
pub struct ApiserverService {
    config: ApiserverConfig,
    storage: KubernetesStorage,
    running: Arc<AtomicBool>,
    rbac: Arc<RbacAuthorizer>,
    token_service: Arc<RwLock<Option<TokenService>>>,
}

impl ApiserverService {
    pub fn new(config: ApiserverConfig, storage: KubernetesStorage) -> Self {
        let rbac = Arc::new(RbacAuthorizer::new());
        let token_service = Arc::new(RwLock::new(None));

        // Attempt early initialization of TokenService if key files already exist
        if config.service_account_signing_key_file.exists()
            && let Ok(ts) = TokenService::new(
                config.service_account_issuer.clone(),
                config.api_audiences.clone(),
                &config.service_account_signing_key_file,
                &config.service_account_key_file,
            )
        {
            let mut guard = token_service.write().unwrap();
            *guard = Some(ts);
        }

        Self {
            config,
            storage,
            running: Arc::new(AtomicBool::new(false)),
            rbac,
            token_service,
        }
    }

    #[must_use]
    pub fn config(&self) -> &ApiserverConfig {
        &self.config
    }

    #[must_use]
    pub fn storage(&self) -> &KubernetesStorage {
        &self.storage
    }

    #[must_use]
    pub fn rbac(&self) -> &Arc<RbacAuthorizer> {
        &self.rbac
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn token_service(&self) -> Result<TokenService, ApiserverError> {
        {
            let guard = self.token_service.read().unwrap();
            if let Some(ts) = guard.as_ref() {
                return Ok(ts.clone());
            }
        }

        let ts = TokenService::new(
            self.config.service_account_issuer.clone(),
            self.config.api_audiences.clone(),
            &self.config.service_account_signing_key_file,
            &self.config.service_account_key_file,
        )?;

        let mut guard = self.token_service.write().unwrap();
        *guard = Some(ts.clone());
        Ok(ts)
    }

    fn token_service_opt(&self) -> Option<Arc<TokenService>> {
        self.token_service().ok().map(Arc::new)
    }

    /// Verifies all PKI and storage prerequisites before startup and initializes token service.
    pub async fn check_prerequisites(&self) -> Result<(), ApiserverError> {
        self.config.validate()?;
        validate_pki_prerequisites(&self.config)?;
        self.storage.check_health().await?;

        // Initialize TokenService
        let _ = self.token_service()?;

        // Restore persisted RBAC definitions from datastore
        self.restore_rbac_from_storage().await?;

        Ok(())
    }

    /// Restores any customized `Roles`, `RoleBindings`, `ClusterRoles`, and `ClusterRoleBindings` from storage.
    pub async fn restore_rbac_from_storage(&self) -> Result<(), ApiserverError> {
        // 1. ClusterRoles
        let cluster_roles_prefix = format!("{}/clusterroles", self.storage.prefix());
        if let Ok(entries) = self.storage.list(&cluster_roles_prefix).await {
            for kv in entries {
                if let Ok(cr) = serde_json::from_slice::<ClusterRole>(&kv.value) {
                    self.rbac.add_cluster_role(cr);
                }
            }
        }

        // 2. ClusterRoleBindings
        let cluster_bindings_prefix = format!("{}/clusterrolebindings", self.storage.prefix());
        if let Ok(entries) = self.storage.list(&cluster_bindings_prefix).await {
            for kv in entries {
                if let Ok(crb) = serde_json::from_slice::<ClusterRoleBinding>(&kv.value) {
                    self.rbac.add_cluster_role_binding(crb);
                }
            }
        }

        // 3. Roles
        let namespaced_roles_prefix = format!("{}/roles", self.storage.prefix());
        if let Ok(entries) = self.storage.list(&namespaced_roles_prefix).await {
            for kv in entries {
                if let Ok(r) = serde_json::from_slice::<Role>(&kv.value) {
                    self.rbac.add_role(r);
                }
            }
        }

        // 4. RoleBindings
        let namespaced_bindings_prefix = format!("{}/rolebindings", self.storage.prefix());
        if let Ok(entries) = self.storage.list(&namespaced_bindings_prefix).await {
            for kv in entries {
                if let Ok(rb) = serde_json::from_slice::<RoleBinding>(&kv.value) {
                    self.rbac.add_role_binding(rb);
                }
            }
        }

        Ok(())
    }

    /// Evaluates current operational readiness.
    pub async fn check_readiness(&self) -> Result<HealthReport, ApiserverError> {
        check_apiserver_readiness(&self.config, &self.storage).await
    }

    pub fn start(&self) -> Result<(), ApiserverError> {
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    // --- Service Account Token Issuance & Projection ---

    pub fn issue_service_account_token(
        &self,
        namespace: &str,
        name: &str,
        audiences: &[String],
        lifetime: Duration,
    ) -> Result<String, ApiserverError> {
        let ts = self.token_service()?;
        ts.issue_token(namespace, name, "sa-uid-default", audiences, lifetime)
    }

    pub fn project_service_account_token(
        &self,
        target_dir: &Path,
        namespace: &str,
        name: &str,
        audiences: &[String],
        lifetime: Duration,
    ) -> Result<PathBuf, ApiserverError> {
        let ts = self.token_service()?;
        let ca_cert = if self.config.client_ca_file.exists() {
            fs::read(&self.config.client_ca_file).unwrap_or_default()
        } else {
            Vec::new()
        };
        let projection = TokenProjection {
            target_dir,
            namespace,
            sa_name: name,
            sa_uid: "sa-uid-default",
            audiences,
            lifetime,
            ca_cert: &ca_cert,
        };
        ts.project_token_volume(&projection)
    }

    // --- Client Factory ---

    #[must_use]
    pub fn admin_client(&self) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::AdminCertificate,
            self.rbac.clone(),
            self.token_service_opt(),
            &self.config,
        )
    }

    #[must_use]
    pub fn anonymous_client(&self) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::Anonymous,
            self.rbac.clone(),
            self.token_service_opt(),
            &self.config,
        )
    }

    #[must_use]
    pub fn token_client(&self, token: impl Into<String>) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::BearerToken(token.into()),
            self.rbac.clone(),
            self.token_service_opt(),
            &self.config,
        )
    }

    #[must_use]
    pub fn service_account_client(
        &self,
        namespace: impl Into<String>,
        name: impl Into<String>,
    ) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::ServiceAccount {
                namespace: namespace.into(),
                name: name.into(),
            },
            self.rbac.clone(),
            self.token_service_opt(),
            &self.config,
        )
    }

    #[must_use]
    pub fn controller_manager_client(&self) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::User {
                username: "system:kube-controller-manager".to_string(),
                groups: vec!["system:authenticated".to_string()],
            },
            self.rbac.clone(),
            self.token_service_opt(),
            &self.config,
        )
    }

    #[must_use]
    pub fn scheduler_client(&self) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::User {
                username: "system:kube-scheduler".to_string(),
                groups: vec!["system:authenticated".to_string()],
            },
            self.rbac.clone(),
            self.token_service_opt(),
            &self.config,
        )
    }

    #[must_use]
    pub fn node_client(&self, node_name: &str) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::User {
                username: format!("system:node:{node_name}"),
                groups: vec![
                    "system:nodes".to_string(),
                    "system:authenticated".to_string(),
                ],
            },
            self.rbac.clone(),
            self.token_service_opt(),
            &self.config,
        )
    }

    #[must_use]
    pub fn restricted_client(
        &self,
        username: impl Into<String>,
        groups: Vec<String>,
    ) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::RestrictedUser {
                username: username.into(),
                groups,
            },
            self.rbac.clone(),
            self.token_service_opt(),
            &self.config,
        )
    }
}
