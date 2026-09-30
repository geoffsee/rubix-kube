pub mod cronjob;
pub mod daemonset;
pub mod deployment;
pub mod garbage_collector;
pub mod job;
pub mod replicaset;
pub mod statefulset;

use std::sync::Arc;

pub use cronjob::CronJobReconciler;
pub use daemonset::DaemonSetReconciler;
pub use deployment::DeploymentReconciler;
pub use garbage_collector::GarbageCollector;
pub use job::JobReconciler;
pub use replicaset::ReplicaSetReconciler;
use rubix_apiserver::client::KubernetesApiClient;
pub use statefulset::StatefulSetReconciler;

use crate::error::ControllerError;

/// Outcome of reconciling a specific workload resource collection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReconcileOutcome {
    pub reconciled: usize,
    pub errors: Vec<String>,
}

/// Coordinates workload controllers and garbage collection.
#[derive(Clone, Debug)]
pub struct WorkloadManager {
    client: Arc<KubernetesApiClient>,
}

impl WorkloadManager {
    #[must_use]
    pub fn new(client: Arc<KubernetesApiClient>) -> Self {
        Self { client }
    }

    #[must_use]
    pub fn client(&self) -> &Arc<KubernetesApiClient> {
        &self.client
    }

    /// Performs a full reconciliation pass across all workloads in the specified namespace:
    /// Deployments -> `ReplicaSets` -> `StatefulSets` -> `DaemonSets` -> `CronJobs` -> Jobs -> Garbage Collection.
    pub async fn reconcile_namespace(
        &self,
        namespace: &str,
    ) -> Result<ReconciliationSummary, ControllerError> {
        let deployment_reconciler = DeploymentReconciler::new(self.client.clone());
        let replicaset_reconciler = ReplicaSetReconciler::new(self.client.clone());
        let statefulset_reconciler = StatefulSetReconciler::new(self.client.clone());
        let daemonset_reconciler = DaemonSetReconciler::new(self.client.clone());
        let cronjob_reconciler = CronJobReconciler::new(self.client.clone());
        let job_reconciler = JobReconciler::new(self.client.clone());
        let gc = GarbageCollector::new(self.client.clone());

        let mut errors = Vec::new();

        let deployments = match deployment_reconciler.reconcile_all(namespace).await {
            Ok(outcome) => {
                errors.extend(outcome.errors);
                outcome.reconciled
            },
            Err(e) => {
                errors.push(e.to_string());
                0
            },
        };

        let replicasets = match replicaset_reconciler.reconcile_all(namespace).await {
            Ok(outcome) => {
                errors.extend(outcome.errors);
                outcome.reconciled
            },
            Err(e) => {
                errors.push(e.to_string());
                0
            },
        };

        let statefulsets = match statefulset_reconciler.reconcile_all(namespace).await {
            Ok(outcome) => {
                errors.extend(outcome.errors);
                outcome.reconciled
            },
            Err(e) => {
                errors.push(e.to_string());
                0
            },
        };

        let daemonsets = match daemonset_reconciler.reconcile_all(namespace).await {
            Ok(outcome) => {
                errors.extend(outcome.errors);
                outcome.reconciled
            },
            Err(e) => {
                errors.push(e.to_string());
                0
            },
        };

        let cronjobs = match cronjob_reconciler.reconcile_all(namespace).await {
            Ok(outcome) => {
                errors.extend(outcome.errors);
                outcome.reconciled
            },
            Err(e) => {
                errors.push(e.to_string());
                0
            },
        };

        let jobs = match job_reconciler.reconcile_all(namespace).await {
            Ok(outcome) => {
                errors.extend(outcome.errors);
                outcome.reconciled
            },
            Err(e) => {
                errors.push(e.to_string());
                0
            },
        };

        let garbage_collected = match gc.reconcile(namespace).await {
            Ok(count) => count,
            Err(e) => {
                errors.push(e.to_string());
                0
            },
        };

        Ok(ReconciliationSummary {
            deployments_reconciled: deployments,
            replicasets_reconciled: replicasets,
            statefulsets_reconciled: statefulsets,
            daemonsets_reconciled: daemonsets,
            cronjobs_reconciled: cronjobs,
            jobs_reconciled: jobs,
            garbage_collected,
            errors,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReconciliationSummary {
    pub deployments_reconciled: usize,
    pub replicasets_reconciled: usize,
    pub statefulsets_reconciled: usize,
    pub daemonsets_reconciled: usize,
    pub cronjobs_reconciled: usize,
    pub jobs_reconciled: usize,
    pub garbage_collected: usize,
    pub errors: Vec<String>,
}
