use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::client::{ClientIdentity, KubernetesApiClient};
use crate::config::ApiserverConfig;
use crate::error::ApiserverError;
use crate::health::{HealthReport, check_apiserver_readiness};
use crate::pki::validate_pki_prerequisites;
use crate::storage::KubernetesStorage;

#[derive(Clone, Debug)]
pub struct ApiserverService {
    config: ApiserverConfig,
    storage: KubernetesStorage,
    running: Arc<AtomicBool>,
}

impl ApiserverService {
    pub fn new(config: ApiserverConfig, storage: KubernetesStorage) -> Self {
        Self {
            config,
            storage,
            running: Arc::new(AtomicBool::new(false)),
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
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Verifies all PKI and storage prerequisites before startup.
    pub async fn check_prerequisites(&self) -> Result<(), ApiserverError> {
        self.config.validate()?;
        validate_pki_prerequisites(&self.config)?;
        self.storage.check_health().await?;
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

    // --- Client Factory ---

    #[must_use]
    pub fn admin_client(&self) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::AdminCertificate,
            &self.config,
        )
    }

    #[must_use]
    pub fn anonymous_client(&self) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::Anonymous,
            &self.config,
        )
    }

    #[must_use]
    pub fn token_client(&self, token: impl Into<String>) -> KubernetesApiClient {
        KubernetesApiClient::new(
            self.storage.clone(),
            ClientIdentity::BearerToken(token.into()),
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
            &self.config,
        )
    }
}
