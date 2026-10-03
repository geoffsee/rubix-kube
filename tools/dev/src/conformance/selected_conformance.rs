//! Selected single-node upstream Kubernetes conformance specification and reporting.

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Upstream conformance focus regex matching namespaced, single-node-safe conformance areas.
pub const CONFORMANCE_FOCUS_REGEX: &str = r"(ConfigMap|Secret|Pods|Services|Deployment|ReplicaSet|DNS|Projected|Downward|EmptyDir).*\[Conformance\]";

/// Upstream conformance skip regex matching tests requiring multiple nodes or disruptive/flaky executions.
pub const CONFORMANCE_SKIP_REGEX: &str =
    r"\[Serial\]|\[Disruptive\]|\[Slow\]|\[Flaky\]|two nodes|multiple nodes|more than one node";

/// Explicit disclaimer avoiding false claims of full Kubernetes certification.
pub const CERTIFICATION_DISCLAIMER: &str = "Notice: Rubix is a single-node Kubernetes distribution. These test results demonstrate qualification against explicitly selected single-node upstream conformance and manifest tiers; they explicitly DO NOT claim official CNCF Certified Kubernetes qualification or multi-node certification.";

/// Reason category for excluding upstream conformance tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionCategory {
    MultiNode,
    Disruptive,
    SerialSlow,
    Flaky,
}

/// Explicit exclusion entry with technical rationale.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceExclusion {
    pub pattern: String,
    pub category: ExclusionCategory,
    pub rationale: String,
}

/// A selected single-node upstream Kubernetes conformance test definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceTestCase {
    pub id: String,
    pub name: String,
    pub focus_keyword: String,
    pub single_node_safe: bool,
    pub description: String,
}

/// Individual conformance test execution result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceResult {
    pub test_id: String,
    pub name: String,
    pub passed: bool,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Comprehensive summary of selected conformance execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceSummary {
    pub total_selected: usize,
    pub passed: usize,
    pub failed: usize,
    pub excluded_count: usize,
    pub focus_filter: String,
    pub skip_filter: String,
    pub certification_disclaimer: String,
    pub exclusions: Vec<ConformanceExclusion>,
    pub results: Vec<ConformanceResult>,
}

impl ConformanceSummary {
    /// Verify that all selected tests passed, zero unexplained failures, and disclaimer is intact.
    pub fn verify_qualification(&self) -> Result<(), String> {
        if !self
            .certification_disclaimer
            .contains("DO NOT claim official CNCF Certified Kubernetes qualification")
        {
            return Err(
                "Qualification report missing required certification disclaimer".to_string(),
            );
        }
        if self.failed > 0 {
            return Err(format!(
                "Selected conformance reported {} test failures",
                self.failed
            ));
        }
        if self.passed == 0 || self.passed != self.total_selected {
            return Err(format!(
                "Mismatch in conformance test count: {} passed out of {} selected",
                self.passed, self.total_selected
            ));
        }
        if self.exclusions.is_empty() {
            return Err(
                "Exclusions list must explicitly document skipped patterns with rationale"
                    .to_string(),
            );
        }
        Ok(())
    }
}

/// Inventory of selected single-node conformance tests and explicit exclusions.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConformanceInventory;

impl ConformanceInventory {
    /// Return the list of 24 explicitly selected single-node upstream conformance tests.
    pub fn selected_tests() -> Vec<ConformanceTestCase> {
        vec![
            // 1. ConfigMap (3 tests)
            ConformanceTestCase {
                id: "k8s-conf-cm-01".into(),
                name: "[k8s.io] ConfigMap should be consumable via environment variables [Conformance]".into(),
                focus_keyword: "ConfigMap".into(),
                single_node_safe: true,
                description: "Verifies env variable values sourced from ConfigMap data keys".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-cm-02".into(),
                name: "[k8s.io] ConfigMap should be consumable via volume mounts with permissions [Conformance]".into(),
                focus_keyword: "ConfigMap".into(),
                single_node_safe: true,
                description: "Verifies ConfigMap volume projection into container filesystem".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-cm-03".into(),
                name: "[k8s.io] ConfigMap should update volume contents upon modification [Conformance]".into(),
                focus_keyword: "ConfigMap".into(),
                single_node_safe: true,
                description: "Verifies dynamic updates of mounted ConfigMap data".into(),
            },

            // 2. Secret (3 tests)
            ConformanceTestCase {
                id: "k8s-conf-sec-01".into(),
                name: "[k8s.io] Secret should be consumable in volume mounts with mode 0600 [Conformance]".into(),
                focus_keyword: "Secret".into(),
                single_node_safe: true,
                description: "Verifies Secret data projection with mode 0600 file permissions".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-sec-02".into(),
                name: "[k8s.io] Secret should be consumable via environment variables [Conformance]".into(),
                focus_keyword: "Secret".into(),
                single_node_safe: true,
                description: "Verifies env variable injection from opaque and tls secret values".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-sec-03".into(),
                name: "[k8s.io] Secret should support base64 binary and stringData decoding [Conformance]".into(),
                focus_keyword: "Secret".into(),
                single_node_safe: true,
                description: "Verifies data and stringData serialization fidelity".into(),
            },

            // 3. Pods (3 tests)
            ConformanceTestCase {
                id: "k8s-conf-pod-01".into(),
                name: "[k8s.io] Pods should support readinessProbe and livenessProbe lifecycle [Conformance]".into(),
                focus_keyword: "Pods".into(),
                single_node_safe: true,
                description: "Verifies probe execution, status reporting, and restart on liveness failure".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-pod-02".into(),
                name: "[k8s.io] Pods should support container restartPolicy Always and Never [Conformance]".into(),
                focus_keyword: "Pods".into(),
                single_node_safe: true,
                description: "Verifies restart policy conformance and exit code propagation".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-pod-03".into(),
                name: "[k8s.io] Pods should execute terminationGracePeriodSeconds on deletion [Conformance]".into(),
                focus_keyword: "Pods".into(),
                single_node_safe: true,
                description: "Verifies graceful SIGTERM settlement before SIGKILL escalation".into(),
            },

            // 4. Services (3 tests)
            ConformanceTestCase {
                id: "k8s-conf-svc-01".into(),
                name: "[k8s.io] Services should route in-cluster traffic to endpoints via ClusterIP [Conformance]".into(),
                focus_keyword: "Services".into(),
                single_node_safe: true,
                description: "Verifies ClusterIP allocation and internal port routing".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-svc-02".into(),
                name: "[k8s.io] Services should allocate unique nodePort on service creation [Conformance]".into(),
                focus_keyword: "Services".into(),
                single_node_safe: true,
                description: "Verifies apiserver port allocation in the 30000-32767 range".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-svc-03".into(),
                name: "[k8s.io] Services should synchronize EndpointSlices on backend pod changes [Conformance]".into(),
                focus_keyword: "Services".into(),
                single_node_safe: true,
                description: "Verifies discovery.k8s.io/v1 EndpointSlice reconciliation".into(),
            },

            // 5. Deployment (2 tests)
            ConformanceTestCase {
                id: "k8s-conf-dep-01".into(),
                name: "[k8s.io] Deployment should scale child ReplicaSets and update rollout status [Conformance]".into(),
                focus_keyword: "Deployment".into(),
                single_node_safe: true,
                description: "Verifies deployment controller reconciliation of spec.replicas".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-dep-02".into(),
                name: "[k8s.io] Deployment should adopt matching ReplicaSets via ownerReferences [Conformance]".into(),
                focus_keyword: "Deployment".into(),
                single_node_safe: true,
                description: "Verifies owner reference binding and selector matching".into(),
            },

            // 6. ReplicaSet (2 tests)
            ConformanceTestCase {
                id: "k8s-conf-rs-01".into(),
                name: "[k8s.io] ReplicaSet should maintain desired pod count and adopt orphans [Conformance]".into(),
                focus_keyword: "ReplicaSet".into(),
                single_node_safe: true,
                description: "Verifies replicaset controller scaling and pod adoption".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-rs-02".into(),
                name: "[k8s.io] ReplicaSet should terminate excess pods on downward scaling [Conformance]".into(),
                focus_keyword: "ReplicaSet".into(),
                single_node_safe: true,
                description: "Verifies clean teardown of excess pod replicas".into(),
            },

            // 7. DNS (2 tests)
            ConformanceTestCase {
                id: "k8s-conf-dns-01".into(),
                name: "[k8s.io] DNS should resolve cluster-local service domain names [Conformance]".into(),
                focus_keyword: "DNS".into(),
                single_node_safe: true,
                description: "Verifies <svc>.<ns>.svc.cluster.local A-record resolution".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-dns-02".into(),
                name: "[k8s.io] DNS should resolve headless service pod IP addresses [Conformance]".into(),
                focus_keyword: "DNS".into(),
                single_node_safe: true,
                description: "Verifies direct endpoint IP resolution for clusterIP None services".into(),
            },

            // 8. Projected (2 tests)
            ConformanceTestCase {
                id: "k8s-conf-proj-01".into(),
                name: "[k8s.io] Projected should project serviceAccountToken with specified audience [Conformance]".into(),
                focus_keyword: "Projected".into(),
                single_node_safe: true,
                description: "Verifies audience-bound projected service account token file".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-proj-02".into(),
                name: "[k8s.io] Projected should combine multiple sources into a single volume [Conformance]".into(),
                focus_keyword: "Projected".into(),
                single_node_safe: true,
                description: "Verifies projected volume combining configMap, secret, and token".into(),
            },

            // 9. Downward (2 tests)
            ConformanceTestCase {
                id: "k8s-conf-down-01".into(),
                name: "[k8s.io] Downward should inject pod name and namespace into environment [Conformance]".into(),
                focus_keyword: "Downward".into(),
                single_node_safe: true,
                description: "Verifies metadata.name and metadata.namespace downward API fields".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-down-02".into(),
                name: "[k8s.io] Downward should inject labels and annotations via downwardAPI volume [Conformance]".into(),
                focus_keyword: "Downward".into(),
                single_node_safe: true,
                description: "Verifies downwardAPI volume projection of labels and annotations".into(),
            },

            // 10. EmptyDir (2 tests)
            ConformanceTestCase {
                id: "k8s-conf-emp-01".into(),
                name: "[k8s.io] EmptyDir should provide writable scratch storage surviving restarts [Conformance]".into(),
                focus_keyword: "EmptyDir".into(),
                single_node_safe: true,
                description: "Verifies emptyDir volume persistence across container restarts".into(),
            },
            ConformanceTestCase {
                id: "k8s-conf-emp-02".into(),
                name: "[k8s.io] EmptyDir should support memory medium tmpfs storage [Conformance]".into(),
                focus_keyword: "EmptyDir".into(),
                single_node_safe: true,
                description: "Verifies medium Memory emptyDir mount backing tmpfs".into(),
            },
        ]
    }

    /// Return the list of explicit exclusions with documented technical rationale.
    pub fn explicit_exclusions() -> Vec<ConformanceExclusion> {
        vec![
            ConformanceExclusion {
                pattern: "[Serial]".into(),
                category: ExclusionCategory::SerialSlow,
                rationale: "Tests tagged [Serial] serialize the entire test run and take multiple hours, exceeding CI budget. Single-node concurrency is qualified via focused domain tests.".into(),
            },
            ConformanceExclusion {
                pattern: "[Disruptive]".into(),
                category: ExclusionCategory::Disruptive,
                rationale: "Disruptive tests intentionally cordon, drain, or reboot the node, which terminates the single control plane / worker host in single-node topologies.".into(),
            },
            ConformanceExclusion {
                pattern: "[Slow]".into(),
                category: ExclusionCategory::SerialSlow,
                rationale: "Tests tagged [Slow] test multi-hour soaking and extreme replica counts (1000+ pods) that exceed single-node edge footprints.".into(),
            },
            ConformanceExclusion {
                pattern: "[Flaky]".into(),
                category: ExclusionCategory::Flaky,
                rationale: "Upstream tests identified as flaky are excluded to prevent non-deterministic failure reporting in automated qualification gates.".into(),
            },
            ConformanceExclusion {
                pattern: "two nodes".into(),
                category: ExclusionCategory::MultiNode,
                rationale: "Rubix is an architectural single-node Kubernetes distribution using NodeSetter admission; multi-node scheduling topologies are not applicable.".into(),
            },
            ConformanceExclusion {
                pattern: "multiple nodes".into(),
                category: ExclusionCategory::MultiNode,
                rationale: "Multi-node scheduling, cross-node pod anti-affinity, and node failover are out of scope for single-node Rubix clusters.".into(),
            },
            ConformanceExclusion {
                pattern: "more than one node".into(),
                category: ExclusionCategory::MultiNode,
                rationale: "Workload distribution across multiple physical hosts requires a full cluster topology, inapplicable to single-node architecture.".into(),
            },
        ]
    }

    /// Validate that a given test name matches the focus filter and does not match any skip filter.
    pub fn is_selected(test_name: &str) -> bool {
        let focus_re = Regex::new(CONFORMANCE_FOCUS_REGEX).expect("valid focus regex");
        let skip_re = Regex::new(CONFORMANCE_SKIP_REGEX).expect("valid skip regex");

        focus_re.is_match(test_name) && !skip_re.is_match(test_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selected_inventory_matches_focus_regex() {
        let tests = ConformanceInventory::selected_tests();
        assert_eq!(tests.len(), 24);

        for t in &tests {
            assert!(
                ConformanceInventory::is_selected(&t.name),
                "Test '{}' must be selected by focus regex",
                t.name
            );
        }
    }

    #[test]
    fn test_exclusions_are_all_rejected_by_skip_regex() {
        let exclusions = ConformanceInventory::explicit_exclusions();
        assert_eq!(exclusions.len(), 7);

        let skip_re = Regex::new(CONFORMANCE_SKIP_REGEX).unwrap();
        for ex in &exclusions {
            assert!(
                skip_re.is_match(&ex.pattern),
                "Exclusion pattern '{}' must match skip regex",
                ex.pattern
            );
        }
    }

    #[test]
    fn test_qualification_verification_requires_disclaimer() {
        let summary = ConformanceSummary {
            total_selected: 24,
            passed: 24,
            failed: 0,
            excluded_count: 7,
            focus_filter: CONFORMANCE_FOCUS_REGEX.to_string(),
            skip_filter: CONFORMANCE_SKIP_REGEX.to_string(),
            certification_disclaimer: CERTIFICATION_DISCLAIMER.to_string(),
            exclusions: ConformanceInventory::explicit_exclusions(),
            results: vec![],
        };
        assert!(summary.verify_qualification().is_ok());

        let mut invalid = summary.clone();
        invalid.certification_disclaimer = "Fully certified Kubernetes cluster".to_string();
        assert!(invalid.verify_qualification().is_err());
    }
}
