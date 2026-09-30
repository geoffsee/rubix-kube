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

        let deployments = deployment_reconciler.reconcile_all(namespace).await?;
        let replicasets = replicaset_reconciler.reconcile_all(namespace).await?;
        let statefulsets = statefulset_reconciler.reconcile_all(namespace).await?;
        let daemonsets = daemonset_reconciler.reconcile_all(namespace).await?;
        let cronjobs = cronjob_reconciler.reconcile_all(namespace).await?;
        let jobs = job_reconciler.reconcile_all(namespace).await?;
        let garbage_collected = gc.reconcile(namespace).await?;

        Ok(ReconciliationSummary {
            deployments_reconciled: deployments,
            replicasets_reconciled: replicasets,
            statefulsets_reconciled: statefulsets,
            daemonsets_reconciled: daemonsets,
            cronjobs_reconciled: cronjobs,
            jobs_reconciled: jobs,
            garbage_collected,
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
}
