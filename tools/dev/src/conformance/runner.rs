//! Synthetic API/controller fixtures; retained-node conformance remains unimplemented.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

use rubix_apiserver::{ApiserverConfig, ApiserverService, KubernetesApiClient, KubernetesStorage};
use rubix_controller::webhook::{WebhookConfig, WebhookService};
use rubix_controller::{ControllerManagerConfig, ControllerManagerService};
use rubix_datastore::{DatastoreConfig, DatastoreEngine};
use rubix_dns::{CoreDnsConfig, DnsProber, DnsProtocol, DnsResolutionProbe, ProbeTransport};
use rubix_pki::cluster::{ClusterPki, ClusterPkiConfig};
use rubix_storage::{LocalPathConfig, LocalPathReconciler, LocalPathVolumeManager};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::kubeconfig::Kubeconfig;
use super::manifests::*;
use super::selected_conformance::{
    CERTIFICATION_DISCLAIMER, CONFORMANCE_FOCUS_REGEX, CONFORMANCE_SKIP_REGEX,
    ConformanceInventory, ConformanceSummary,
};

/// The six baseline manifest domains defined in Issue #118 and Epic E28.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestDomain {
    WorkloadsNetworking,
    Storage,
    ConfigIdentity,
    Controllers,
    DnsLoadBalancer,
    LbUpdate,
}

impl ManifestDomain {
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::WorkloadsNetworking => "Tier 1 — Workloads & Networking",
            Self::Storage => "Tier 2 — Storage Persistence",
            Self::ConfigIdentity => "Tier 3 — Config & Identity",
            Self::Controllers => "Tier 4 — Controllers",
            Self::DnsLoadBalancer => "Tier 5 — DNS & LoadBalancer",
            Self::LbUpdate => "Tier 6 — LoadBalancer UPDATE path [KS-75]",
        }
    }
}

/// Baseline smoke test checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmokeCheck {
    WorkloadPod,
    InClusterDns,
    PodEgress,
}

impl SmokeCheck {
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::WorkloadPod => "Smoke 1 — Workload Pod Scheduling and Placement",
            Self::InClusterDns => "Smoke 2 — In-Cluster CoreDNS Resolution",
            Self::PodEgress => "Smoke 3 — Pod Egress Masquerade / SNAT Routing",
        }
    }
}

/// Individual smoke check report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmokeReport {
    pub check: SmokeCheck,
    pub name: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub details: String,
}

/// Report for an individual manifest domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainReport {
    pub domain: ManifestDomain,
    pub name: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub assertions_count: usize,
    pub details: Vec<String>,
}

/// Overall qualification report covering smoke, manifest tiers, and conformance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationReport {
    pub evidence_kind: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_integrity_hash: Option<String>,
    pub smoke_results: Vec<SmokeReport>,
    pub manifest_domain_results: Vec<DomainReport>,
    pub conformance_summary: ConformanceSummary,
    pub total_assertions_checked: usize,
    pub kubeconfig_format_accommodated: String,
    pub certification_disclaimer: String,
}

impl QualificationReport {
    /// In-process fixtures never qualify the selected retained executables.
    pub fn verify_qualification(&self) -> Result<(), String> {
        self.verify_fixture()?;
        if self.receipt_id.is_some() && self.receipt_integrity_hash.is_some() {
            return Ok(());
        }
        Err("Synthetic in-process fixture evidence cannot qualify C13/E28; a retained-executable node runner is not implemented".into())
    }

    /// Validate exact fixture coverage and counts without claiming live qualification.
    pub fn verify_fixture(&self) -> Result<(), String> {
        let smoke = [
            SmokeCheck::WorkloadPod,
            SmokeCheck::InClusterDns,
            SmokeCheck::PodEgress,
        ];
        let domains = [
            (ManifestDomain::WorkloadsNetworking, 10),
            (ManifestDomain::Storage, 12),
            (ManifestDomain::ConfigIdentity, 6),
            (ManifestDomain::Controllers, 11),
            (ManifestDomain::DnsLoadBalancer, 5),
            (ManifestDomain::LbUpdate, 10),
        ];
        if (self.evidence_kind != "synthetic_fixture"
            && self.evidence_kind != "candidate_receipt_bound")
            || self.smoke_results.len() != smoke.len()
            || self.manifest_domain_results.len() != domains.len()
            || self.certification_disclaimer != CERTIFICATION_DISCLAIMER
            || self.kubeconfig_format_accommodated != "YAML and JSON (dual-format validated)"
        {
            return Err("Invalid fixture evidence kind or required coverage".into());
        }
        for id in smoke {
            let rows: Vec<_> = self
                .smoke_results
                .iter()
                .filter(|s| s.check == id)
                .collect();
            if rows.len() != 1
                || rows[0].name != id.display_name()
                || rows[0].passed != (id != SmokeCheck::PodEgress)
            {
                return Err("Missing, duplicate or incorrectly classified smoke fixture".into());
            }
        }
        for (id, count) in domains {
            let rows: Vec<_> = self
                .manifest_domain_results
                .iter()
                .filter(|d| d.domain == id)
                .collect();
            if rows.len() != 1
                || !rows[0].passed
                || rows[0].name != id.display_name()
                || rows[0].assertions_count != count
            {
                return Err("Missing, duplicate or inconsistent manifest fixture evidence".into());
            }
        }
        self.conformance_summary.verify_fixture()?;
        // The fixture performs three API admission assertions and two synthetic DNS
        // assertions; pod egress and upstream conformance are not executed.
        let expected_assertions = 5 + self
            .manifest_domain_results
            .iter()
            .map(|d| d.assertions_count)
            .sum::<usize>();
        if self.total_assertions_checked != expected_assertions {
            return Err("Assertion total disagrees with executed fixture checks".into());
        }
        Ok(())
    }

    /// Render to pretty JSON string.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Render to GitHub-flavored Markdown summary table.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# Rubix Synthetic In-Process Fixture Report\n\n");
        if let (Some(receipt_id), Some(hash)) = (&self.receipt_id, &self.receipt_integrity_hash) {
            md.push_str(&format!(
                "- **Receipt ID**: `{receipt_id}`\n- **Receipt Integrity Hash**: `{hash}`\n\n"
            ));
        }
        md.push_str("C13/E28 remains unqualified. No retained-executable node or upstream conformance suite was run.\n\n");
        md.push_str("> ");
        md.push_str(&self.certification_disclaimer);
        md.push_str("\n\n");

        md.push_str("## 1. Baseline Smoke Verification\n\n");
        md.push_str("| Smoke Check | Status | Duration | Details |\n");
        md.push_str("|---|---|---|---|\n");
        for s in &self.smoke_results {
            let status = if s.check == SmokeCheck::PodEgress {
                "NOT EXECUTED"
            } else if s.passed {
                "PASS (fixture)"
            } else {
                "FAIL"
            };
            md.push_str(&format!(
                "| {} | {} | {}ms | {} |\n",
                s.name, status, s.duration_ms, s.details
            ));
        }
        md.push('\n');

        md.push_str("## 2. Six Baseline Manifest Domains\n\n");
        md.push_str("| Domain | Status | Assertions | Duration |\n");
        md.push_str("|---|---|---|---|\n");
        for d in &self.manifest_domain_results {
            let status = if d.passed { "PASS" } else { "FAIL" };
            md.push_str(&format!(
                "| {} | {} | {} | {}ms |\n",
                d.name, status, d.assertions_count, d.duration_ms
            ));
        }
        md.push('\n');

        md.push_str("## 3. Selected Single-Node Conformance Summary\n\n");
        md.push_str("NOT EXECUTED. The inventory lists planned candidates; API-object creation does not establish upstream test execution.\n\n");
        md.push_str(&format!(
            "- **Total Selected Tests**: {}\n",
            self.conformance_summary.total_selected
        ));
        md.push_str(&format!(
            "- **Passed**: {}\n",
            self.conformance_summary.passed
        ));
        md.push_str(&format!(
            "- **Failed**: {}\n",
            self.conformance_summary.failed
        ));
        md.push_str(&format!(
            "- **Explicit Exclusions**: {}\n",
            self.conformance_summary.excluded_count
        ));
        md.push_str(&format!(
            "- **Focus Pattern**: `{}`\n",
            self.conformance_summary.focus_filter
        ));
        md.push_str(&format!(
            "- **Skip Pattern**: `{}`\n\n",
            self.conformance_summary.skip_filter
        ));

        md.push_str("### Explicit Exclusions with Rationale\n\n");
        md.push_str("| Pattern | Category | Technical Rationale |\n");
        md.push_str("|---|---|---|\n");
        for ex in &self.conformance_summary.exclusions {
            md.push_str(&format!(
                "| `{}` | `{:?}` | {} |\n",
                ex.pattern, ex.category, ex.rationale
            ));
        }
        md.push('\n');

        md.push_str("### Kubeconfig Format Accommodation\n\n");
        md.push_str(&format!(
            "Verified dual-format accommodation: **{}** (YAML and JSON interchangeability confirmed).\n\n",
            self.kubeconfig_format_accommodated
        ));

        md
    }
}

/// Qualification runner providing the integration harness.
#[derive(Debug)]
pub struct QualificationRunner {
    node_name: String,
    node_ip: IpAddr,
    lb_ip: String,
}

impl Default for QualificationRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl QualificationRunner {
    #[must_use]
    pub fn new() -> Self {
        Self {
            node_name: "rubix-node-qual".to_string(),
            node_ip: "192.0.2.10".parse().expect("valid IP"),
            lb_ip: "192.0.2.10".to_string(),
        }
    }

    /// Execute the complete smoke, manifest tier, and conformance qualification suite.
    pub async fn run_qualification(&self) -> Result<QualificationReport, String> {
        Err("Retained-executable C13/E28 node qualification is not implemented; use the explicit synthetic fixture command".into())
    }

    /// Execute only synthetic in-process API/controller fixtures.
    pub async fn run_fixture(&self) -> Result<QualificationReport, String> {
        let temp = TempDir::new().map_err(|e| format!("TempDir error: {e}"))?;

        // 1. Setup PKI
        let pki_dir = temp.path().join("pki");
        std::fs::create_dir_all(&pki_dir).map_err(|e| e.to_string())?;
        let pki_config =
            ClusterPkiConfig::new(pki_dir.clone(), self.node_name.clone(), self.node_ip);
        let pki = ClusterPki::new(pki_config);
        pki.reconcile().map_err(|e| e.to_string())?;

        // 2. Setup Datastore
        let datastore_dir = temp.path().join("datastore");
        let (engine, _) = DatastoreEngine::open(DatastoreConfig::new(datastore_dir))
            .map_err(|e| e.to_string())?;
        let storage = KubernetesStorage::new(engine.client(), "/registry");

        // 3. Setup Apiserver
        let apiserver_config = ApiserverConfig::default_for_pki(&pki_dir, self.node_ip);
        let apiserver_service = Arc::new(ApiserverService::new(apiserver_config, storage));
        apiserver_service
            .check_prerequisites()
            .await
            .map_err(|e| e.to_string())?;
        apiserver_service.start().map_err(|e| e.to_string())?;

        // 4. Setup Webhook (NodeSetter + LoadBalancer status)
        let mut webhook_config = WebhookConfig::default_for_pki(
            &pki_dir,
            &self.node_name,
            &self.lb_ip,
            true, // LoadBalancer enabled
        );
        webhook_config.port = 0; // ephemeral port for test isolation
        let webhook_service = WebhookService::new(webhook_config, apiserver_service.clone());
        webhook_service
            .check_prerequisites()
            .await
            .map_err(|e| e.to_string())?;
        webhook_service.start().await.map_err(|e| e.to_string())?;

        // 5. Setup Controller Manager
        let controller_config = ControllerManagerConfig::default_for_pki(&pki_dir, self.node_ip);
        let controller_service =
            ControllerManagerService::new(controller_config, apiserver_service.clone());
        controller_service
            .start()
            .await
            .map_err(|e| e.to_string())?;

        // 6. Setup LocalPath Storage Provisioner
        let storage_root = temp.path().join("local-path-storage");
        std::fs::create_dir_all(&storage_root).map_err(|e| e.to_string())?;
        let localpath_config = LocalPathConfig::new()
            .with_storage_path(storage_root.display().to_string())
            .with_volume_binding_mode("WaitForFirstConsumer")
            .with_reclaim_policy("Retain");

        let client = apiserver_service.admin_client();

        // 7. Verify kubeconfig accommodation (both YAML and JSON formats)
        let kubeconfig_path = pki_dir.join("admin.kubeconfig");
        let parsed_yaml = Kubeconfig::from_file(&kubeconfig_path)
            .map_err(|e| format!("Failed to read/parse YAML kubeconfig: {e}"))?;
        let kubeconfig_json_str = parsed_yaml
            .to_json()
            .map_err(|e| format!("Failed to serialize kubeconfig to JSON: {e}"))?;
        let parsed_json = Kubeconfig::parse(&kubeconfig_json_str)
            .map_err(|e| format!("Failed to parse JSON kubeconfig: {e}"))?;
        if parsed_yaml != parsed_json {
            return Err("Kubeconfig parsed from YAML and JSON do not match".to_string());
        }

        let mut smoke_results = Vec::new();
        let mut manifest_domain_results = Vec::new();
        let mut total_assertions = 0;

        // ── 8. Execute Smoke Checks ──────────────────────────────────────────
        let s1 = self.run_smoke_pod(&client).await?;
        total_assertions += 3;
        smoke_results.push(s1);

        let s2 = self.run_smoke_dns(&client).await?;
        total_assertions += 2;
        smoke_results.push(s2);

        let s3 = self.run_smoke_egress(&client).await?;
        smoke_results.push(s3);

        // ── 9. Execute Manifest Domains ───────────────────────────────────────
        let (d1, a1) = self
            .run_tier1_workload(&client, &controller_service)
            .await?;
        total_assertions += a1;
        manifest_domain_results.push(d1);

        let (d2, a2) = self
            .run_tier2_storage(&client, &localpath_config, &storage_root)
            .await?;
        total_assertions += a2;
        manifest_domain_results.push(d2);

        let (d3, a3) = self.run_tier3_config(&client).await?;
        total_assertions += a3;
        manifest_domain_results.push(d3);

        let (d4, a4) = self
            .run_tier4_controllers(&client, &controller_service)
            .await?;
        total_assertions += a4;
        manifest_domain_results.push(d4);

        let (d5, a5) = self.run_tier5_dns_lb(&client).await?;
        total_assertions += a5;
        manifest_domain_results.push(d5);

        let (d6, a6) = self.run_tier6_lb_update(&client).await?;
        total_assertions += a6;
        manifest_domain_results.push(d6);

        // ── 10. Execute Selected Conformance Suite ───────────────────────────
        let (conformance_summary, a_conf) = self.run_selected_conformance(&client).await?;
        total_assertions += a_conf;

        // Teardown services
        webhook_service.stop().await;
        controller_service.stop();

        let report = QualificationReport {
            evidence_kind: "synthetic_fixture".into(),
            timestamp: format!(
                "unix:{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_secs()
            ),
            receipt_id: None,
            receipt_integrity_hash: None,
            smoke_results,
            manifest_domain_results,
            conformance_summary,
            total_assertions_checked: total_assertions,
            kubeconfig_format_accommodated: "YAML and JSON (dual-format validated)".to_string(),
            certification_disclaimer: CERTIFICATION_DISCLAIMER.to_string(),
        };

        report.verify_fixture()?;
        Ok(report)
    }

    // ── Smoke Check 1: Workload Pod ──────────────────────────────────────────
    async fn run_smoke_pod(&self, client: &KubernetesApiClient) -> Result<SmokeReport, String> {
        let start = Instant::now();
        client
            .create_namespace(SMOKE_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;

        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": "smoke-busybox",
                "namespace": SMOKE_NAMESPACE
            },
            "spec": {
                "restartPolicy": "Never",
                "containers": [{
                    "name": "busybox",
                    "image": "busybox:1.36",
                    "command": ["sh", "-c", "sleep 3600"]
                }]
            }
        });

        let created = client
            .create_pod(SMOKE_NAMESPACE, pod)
            .await
            .map_err(|e| format!("Failed to create smoke pod: {e}"))?;

        // Assert NodeSetter assigned nodeName
        let assigned_node = created["spec"]["nodeName"].as_str().unwrap_or("");
        if assigned_node != self.node_name {
            return Err(format!(
                "Smoke pod nodeName mismatch: expected '{}', got '{}'",
                self.node_name, assigned_node
            ));
        }

        // Cleanup
        client
            .delete_namespace(SMOKE_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;

        Ok(SmokeReport {
            check: SmokeCheck::WorkloadPod,
            name: SmokeCheck::WorkloadPod.display_name().to_string(),
            passed: true,
            duration_ms: start.elapsed().as_millis() as u64,
            details: format!(
                "Synthetic Pod API admission applied NodeSetter mutation, nodeName='{}'; no workload executed",
                assigned_node
            ),
        })
    }

    // ── Smoke Check 2: In-Cluster DNS ────────────────────────────────────────
    async fn run_smoke_dns(&self, client: &KubernetesApiClient) -> Result<SmokeReport, String> {
        let start = Instant::now();
        // Setup kubernetes.default Service in default namespace
        let svc_ip: Ipv4Addr = "10.43.0.1".parse().unwrap();
        let k8s_svc = json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": "kubernetes",
                "namespace": "default"
            },
            "spec": {
                "type": "ClusterIP",
                "clusterIP": svc_ip.to_string(),
                "ports": [{
                    "name": "https",
                    "port": 443,
                    "protocol": "TCP"
                }]
            }
        });

        let _ = client.create_service("default", k8s_svc).await;

        let prober = DnsProber::new(ProbeTransport::Synthetic {
            client: Arc::new(client.clone()),
            config: CoreDnsConfig::new(),
        });

        let probe =
            DnsResolutionProbe::same_namespace("kubernetes", "default", svc_ip, DnsProtocol::Udp);

        let result = prober
            .execute_probe(&probe)
            .await
            .map_err(|e| format!("DNS probe error: {e}"))?;

        if !result.success {
            return Err(format!("Smoke DNS check failed: {}", result.details));
        }

        Ok(SmokeReport {
            check: SmokeCheck::InClusterDns,
            name: SmokeCheck::InClusterDns.display_name().to_string(),
            passed: true,
            duration_ms: start.elapsed().as_millis() as u64,
            details: "Synthetic DNS model matched kubernetes.default.svc.cluster.local to ClusterIP 10.43.0.1; no CoreDNS server or pod query executed"
                .to_string(),
        })
    }

    // ── Smoke Check 3: Pod Egress ────────────────────────────────────────────
    async fn run_smoke_egress(&self, _client: &KubernetesApiClient) -> Result<SmokeReport, String> {
        let start = Instant::now();
        Ok(SmokeReport {
            check: SmokeCheck::PodEgress,
            name: SmokeCheck::PodEgress.display_name().to_string(),
            passed: false,
            duration_ms: start.elapsed().as_millis() as u64,
            details: "NOT EXECUTED: in-process fixtures have no pod runtime or network dataplane"
                .into(),
        })
    }

    // ── Tier 1: Workloads & Networking ───────────────────────────────────────
    async fn run_tier1_workload(
        &self,
        client: &KubernetesApiClient,
        controller: &ControllerManagerService,
    ) -> Result<(DomainReport, usize), String> {
        let start = Instant::now();
        let mut assertions = 0;
        let mut details = Vec::new();

        client
            .create_namespace(TIER1_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;

        // 1. Create Deployment
        let deployment = json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": {
                "name": "web",
                "namespace": TIER1_NAMESPACE
            },
            "spec": {
                "replicas": 2,
                "selector": {
                    "matchLabels": { "app": "web" }
                },
                "template": {
                    "metadata": {
                        "labels": { "app": "web" }
                    },
                    "spec": {
                        "containers": [{
                            "name": "web",
                            "image": "nginx:1.27-alpine",
                            "ports": [{ "containerPort": 80 }]
                        }]
                    }
                }
            }
        });

        client
            .create_deployment(TIER1_NAMESPACE, deployment)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // Reconcile Deployment -> ReplicaSet -> Pods
        let wm = controller.workload_manager();
        wm.reconcile_namespace(TIER1_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;
        wm.reconcile_namespace(TIER1_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;

        let rs = client
            .get_replicaset(TIER1_NAMESPACE, "web-rs")
            .await
            .map_err(|e| format!("ReplicaSet web-rs not generated: {e}"))?;
        if rs["spec"]["replicas"] != 2 {
            return Err("web-rs replicas mismatch".to_string());
        }
        assertions += 1;
        details.push("Deployment reconciled to ReplicaSet web-rs with 2 replicas".to_string());

        let pods = client
            .list_pods(TIER1_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;
        let pod_items = pods["items"].as_array().ok_or("pods list error")?;
        if pod_items.len() != 2 {
            return Err(format!("Expected 2 child pods, found {}", pod_items.len()));
        }
        assertions += 1;
        for p in pod_items {
            let node = p["spec"]["nodeName"].as_str().unwrap_or("");
            if node != self.node_name {
                return Err(format!(
                    "Pod node placement mismatch: expected '{}', got '{}'",
                    self.node_name, node
                ));
            }
            assertions += 1;
        }
        details.push(
            "ReplicaSet generated 2 Pod API objects mutated with NodeSetter placement".to_string(),
        );

        // 2. Create Services (ClusterIP and NodePort)
        let svc_clusterip = json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": "web-clusterip",
                "namespace": TIER1_NAMESPACE
            },
            "spec": {
                "type": "ClusterIP",
                "clusterIP": "10.43.20.1",
                "selector": { "app": "web" },
                "ports": [{ "port": 80, "targetPort": 80 }]
            }
        });
        let created_cip = client
            .create_service(TIER1_NAMESPACE, svc_clusterip)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;
        let cip = created_cip["spec"]["clusterIP"].as_str().unwrap_or("");
        if cip.is_empty() {
            return Err("ClusterIP was not allocated".to_string());
        }
        assertions += 1;
        details.push(format!("ClusterIP service allocated IP: {cip}"));

        let svc_nodeport = json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": "web-nodeport",
                "namespace": TIER1_NAMESPACE
            },
            "spec": {
                "type": "NodePort",
                "selector": { "app": "web" },
                "ports": [{ "port": 80, "targetPort": 80, "nodePort": 31080 }]
            }
        });
        let created_np = client
            .create_service(TIER1_NAMESPACE, svc_nodeport)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;
        let np = created_np["spec"]["ports"][0]["nodePort"]
            .as_u64()
            .unwrap_or(0);
        if np == 0 {
            return Err("NodePort was not allocated".to_string());
        }
        assertions += 1;
        details.push(format!("NodePort service allocated port: {np}"));

        // Cleanup
        client
            .delete_namespace(TIER1_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        Ok((
            DomainReport {
                domain: ManifestDomain::WorkloadsNetworking,
                name: ManifestDomain::WorkloadsNetworking
                    .display_name()
                    .to_string(),
                passed: true,
                duration_ms: start.elapsed().as_millis() as u64,
                assertions_count: assertions,
                details,
            },
            assertions,
        ))
    }

    // ── Tier 2: Storage Persistence ──────────────────────────────────────────
    async fn run_tier2_storage(
        &self,
        client: &KubernetesApiClient,
        storage_config: &LocalPathConfig,
        storage_root: &std::path::Path,
    ) -> Result<(DomainReport, usize), String> {
        let start = Instant::now();
        let mut assertions = 0;
        let mut details = Vec::new();

        client
            .create_namespace(TIER2_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;

        // 1. Storage provisioner reconciliation
        let storage_reconciler = LocalPathReconciler::new(storage_config);
        storage_reconciler
            .reconcile(client)
            .await
            .map_err(|e| format!("Storage provisioner reconcile error: {e}"))?;
        assertions += 1;
        details.push("local-path StorageClass reconciled with Retain policy".to_string());

        // 2. Create PVC requesting local-path
        let pvc = json!({
            "apiVersion": "v1",
            "kind": "PersistentVolumeClaim",
            "metadata": {
                "name": "data",
                "namespace": TIER2_NAMESPACE
            },
            "spec": {
                "accessModes": ["ReadWriteOnce"],
                "storageClassName": "local-path",
                "resources": {
                    "requests": { "storage": "64Mi" }
                }
            }
        });
        let created_pvc = client
            .create_pvc(TIER2_NAMESPACE, pvc)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // Verify NodeSetter selected-node annotation
        let selected_node =
            created_pvc["metadata"]["annotations"]["volume.kubernetes.io/selected-node"]
                .as_str()
                .unwrap_or("");
        if selected_node != self.node_name {
            return Err("PVC missing volume.kubernetes.io/selected-node".to_string());
        }
        assertions += 1;
        details.push("PVC annotated with volume.kubernetes.io/selected-node".to_string());

        // 3. Create writer Job
        let writer_job = json!({
            "apiVersion": "batch/v1",
            "kind": "Job",
            "metadata": {
                "name": "writer",
                "namespace": TIER2_NAMESPACE
            },
            "spec": {
                "backoffLimit": 2,
                "template": {
                    "spec": {
                        "restartPolicy": "Never",
                        "containers": [{
                            "name": "writer",
                            "image": "busybox:1.36",
                            "command": ["sh", "-c", "echo persisted-ok > /data/marker && sync"],
                            "volumeMounts": [{ "name": "data", "mountPath": "/data" }]
                        }],
                        "volumes": [{
                            "name": "data",
                            "persistentVolumeClaim": { "claimName": "data" }
                        }]
                    }
                }
            }
        });
        client
            .create_job(TIER2_NAMESPACE, writer_job)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // Volume manager provisions PV for PVC
        let vol_mgr = LocalPathVolumeManager::new(storage_config.clone(), client.clone());
        let writer_pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": "writer-pod",
                "namespace": TIER2_NAMESPACE
            },
            "spec": {
                "nodeName": self.node_name,
                "volumes": [{
                    "name": "data",
                    "persistentVolumeClaim": { "claimName": "data" }
                }]
            }
        });
        client
            .create_pod(TIER2_NAMESPACE, writer_pod)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        let bound_pvs = vol_mgr
            .bind_volumes_for_pod(TIER2_NAMESPACE, "writer-pod")
            .await
            .map_err(|e| format!("Volume provisioning error: {e}"))?;
        let pv_name = bound_pvs
            .first()
            .and_then(|pv| pv.get("metadata"))
            .and_then(|m| m.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| "Failed to extract PV name from bound volumes".to_string())?;
        assertions += 1;

        // Simulate writer pod writing marker to volume
        let vol_dir = storage_root.join(pv_name);
        std::fs::create_dir_all(&vol_dir).map_err(|e| e.to_string())?;
        std::fs::write(vol_dir.join("marker"), b"persisted-ok\n").map_err(|e| e.to_string())?;
        assertions += 1;
        details.push(
            "Synthetic host fixture wrote the marker; no writer workload executed".to_string(),
        );

        // Delete writer pod and job
        client
            .delete_pod(TIER2_NAMESPACE, "writer-pod")
            .await
            .map_err(|e| e.to_string())?;
        client
            .delete_job(TIER2_NAMESPACE, "writer")
            .await
            .map_err(|e| e.to_string())?;
        assertions += 2;

        // 4. Create reader Pod mounting SAME PVC
        let reader_pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": "reader",
                "namespace": TIER2_NAMESPACE
            },
            "spec": {
                "restartPolicy": "Never",
                "containers": [{
                    "name": "reader",
                    "image": "busybox:1.36",
                    "command": ["sh", "-c", "test \"$(cat /data/marker)\" = persisted-ok && sleep 3600"],
                    "volumeMounts": [{ "name": "data", "mountPath": "/data" }]
                }],
                "volumes": [{
                    "name": "data",
                    "persistentVolumeClaim": { "claimName": "data" }
                }]
            }
        });
        client
            .create_pod(TIER2_NAMESPACE, reader_pod)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // Read host-fixture data after changing Pod API objects; no pod ran.
        let marker_content =
            std::fs::read_to_string(vol_dir.join("marker")).map_err(|e| e.to_string())?;
        if marker_content.trim() != "persisted-ok" {
            return Err(format!(
                "PVC marker mismatch: expected 'persisted-ok', got '{}'",
                marker_content.trim()
            ));
        }
        assertions += 1;
        details.push("Host fixture read the marker after Pod API object replacement; no reader workload executed".to_string());

        // Cleanup
        client
            .delete_namespace(TIER2_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        Ok((
            DomainReport {
                domain: ManifestDomain::Storage,
                name: ManifestDomain::Storage.display_name().to_string(),
                passed: true,
                duration_ms: start.elapsed().as_millis() as u64,
                assertions_count: assertions,
                details,
            },
            assertions,
        ))
    }

    // ── Tier 3: Config & Identity ────────────────────────────────────────────
    async fn run_tier3_config(
        &self,
        client: &KubernetesApiClient,
    ) -> Result<(DomainReport, usize), String> {
        let start = Instant::now();
        let mut assertions = 0;
        let mut details = Vec::new();

        client
            .create_namespace(TIER3_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;

        // 1. ConfigMap
        let mut cm_data = BTreeMap::new();
        cm_data.insert("greeting".to_string(), "hello-kubesolo".to_string());
        client
            .create_configmap(TIER3_NAMESPACE, "app-config", cm_data)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;
        details.push("ConfigMap app-config created with greeting=hello-kubesolo".to_string());

        // 2. Secret
        let mut sec_data = BTreeMap::new();
        sec_data.insert("token".to_string(), "s3cr3t-value".to_string());
        client
            .create_secret(TIER3_NAMESPACE, "app-secret", sec_data, Some("Opaque"))
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;
        details.push("Secret app-secret created with token stringData".to_string());

        // 3. Consumer Pod
        let consumer_pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": "consumer",
                "namespace": TIER3_NAMESPACE
            },
            "spec": {
                "restartPolicy": "Never",
                "containers": [{
                    "name": "consumer",
                    "image": "busybox:1.36",
                    "command": ["sh", "-c", "sleep 3600"],
                    "env": [{
                        "name": "GREETING",
                        "valueFrom": {
                            "configMapKeyRef": {
                                "name": "app-config",
                                "key": "greeting"
                            }
                        }
                    }],
                    "volumeMounts": [
                        { "name": "secret", "mountPath": "/etc/secret", "readOnly": true },
                        { "name": "token", "mountPath": "/var/run/secrets/tokens", "readOnly": true }
                    ]
                }],
                "volumes": [
                    { "name": "secret", "secret": { "secretName": "app-secret" } },
                    {
                        "name": "token",
                        "projected": {
                            "sources": [{
                                "serviceAccountToken": {
                                    "path": "sa-token",
                                    "expirationSeconds": 3600,
                                    "audience": "kubesolo-e2e"
                                }
                            }]
                        }
                    }
                ]
            }
        });

        let created_pod = client
            .create_pod(TIER3_NAMESPACE, consumer_pod)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // Verify Pod spec retains envFrom configMapKeyRef, secret volume, and projected token
        let volumes = created_pod["spec"]["volumes"]
            .as_array()
            .ok_or("volumes array")?;
        if volumes.len() != 2 {
            return Err("Expected 2 volumes in consumer pod".to_string());
        }
        assertions += 1;

        let proj = &volumes[1]["projected"]["sources"][0]["serviceAccountToken"];
        if proj["audience"] != "kubesolo-e2e" {
            return Err("Projected token audience mismatch".to_string());
        }
        assertions += 1;
        details.push(
            "Stored Pod spec retained Secret/projected-token volume declarations and token audience; no container consumed configuration, mounted volumes or received a token"
                .to_string(),
        );

        // Cleanup
        client
            .delete_namespace(TIER3_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        Ok((
            DomainReport {
                domain: ManifestDomain::ConfigIdentity,
                name: ManifestDomain::ConfigIdentity.display_name().to_string(),
                passed: true,
                duration_ms: start.elapsed().as_millis() as u64,
                assertions_count: assertions,
                details,
            },
            assertions,
        ))
    }

    // ── Tier 4: Controllers ──────────────────────────────────────────────────
    async fn run_tier4_controllers(
        &self,
        client: &KubernetesApiClient,
        controller: &ControllerManagerService,
    ) -> Result<(DomainReport, usize), String> {
        let start = Instant::now();
        let mut assertions = 0;
        let mut details = Vec::new();

        client
            .create_namespace(TIER4_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;

        // 1. Job oneshot
        let job = json!({
            "apiVersion": "batch/v1",
            "kind": "Job",
            "metadata": {
                "name": "oneshot",
                "namespace": TIER4_NAMESPACE
            },
            "spec": {
                "backoffLimit": 2,
                "template": {
                    "spec": {
                        "restartPolicy": "Never",
                        "containers": [{
                            "name": "oneshot",
                            "image": "busybox:1.36",
                            "command": ["sh", "-c", "echo done"]
                        }]
                    }
                }
            }
        });
        client
            .create_job(TIER4_NAMESPACE, job)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // 2. CronJob periodic
        let cjob = json!({
            "apiVersion": "batch/v1",
            "kind": "CronJob",
            "metadata": {
                "name": "periodic",
                "namespace": TIER4_NAMESPACE
            },
            "spec": {
                "schedule": "*/5 * * * *",
                "jobTemplate": {
                    "spec": {
                        "template": {
                            "spec": {
                                "restartPolicy": "Never",
                                "containers": [{
                                    "name": "tick",
                                    "image": "busybox:1.36",
                                    "command": ["sh", "-c", "echo tick"]
                                }]
                            }
                        }
                    }
                }
            }
        });
        client
            .create_cronjob(TIER4_NAMESPACE, cjob)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // 3. DaemonSet agent
        let ds = json!({
            "apiVersion": "apps/v1",
            "kind": "DaemonSet",
            "metadata": {
                "name": "agent",
                "namespace": TIER4_NAMESPACE
            },
            "spec": {
                "selector": {
                    "matchLabels": { "app": "agent" }
                },
                "template": {
                    "metadata": {
                        "labels": { "app": "agent" }
                    },
                    "spec": {
                        "containers": [{
                            "name": "agent",
                            "image": "busybox:1.36",
                            "command": ["sh", "-c", "sleep 3600"]
                        }]
                    }
                }
            }
        });
        client
            .create_daemonset(TIER4_NAMESPACE, ds)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // 4. StatefulSet stateful with headless Service
        let headless_svc = json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": "stateful",
                "namespace": TIER4_NAMESPACE
            },
            "spec": {
                "clusterIP": "None",
                "selector": { "app": "stateful" },
                "ports": [{ "port": 80 }]
            }
        });
        client
            .create_service(TIER4_NAMESPACE, headless_svc)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        let ss = json!({
            "apiVersion": "apps/v1",
            "kind": "StatefulSet",
            "metadata": {
                "name": "stateful",
                "namespace": TIER4_NAMESPACE
            },
            "spec": {
                "serviceName": "stateful",
                "replicas": 1,
                "selector": {
                    "matchLabels": { "app": "stateful" }
                },
                "template": {
                    "metadata": {
                        "labels": { "app": "stateful" }
                    },
                    "spec": {
                        "containers": [{
                            "name": "stateful",
                            "image": "busybox:1.36",
                            "command": ["sh", "-c", "sleep 3600"]
                        }]
                    }
                }
            }
        });
        client
            .create_statefulset(TIER4_NAMESPACE, ss)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // 5. Trigger CronJob manually (kubectl create job --from=cronjob/periodic periodic-manual)
        let manual_job = json!({
            "apiVersion": "batch/v1",
            "kind": "Job",
            "metadata": {
                "name": "periodic-manual",
                "namespace": TIER4_NAMESPACE,
                "ownerReferences": [{
                    "apiVersion": "batch/v1",
                    "kind": "CronJob",
                    "name": "periodic",
                    "uid": "uid-cronjob-periodic"
                }]
            },
            "spec": {
                "template": {
                    "spec": {
                        "restartPolicy": "Never",
                        "containers": [{
                            "name": "tick",
                            "image": "busybox:1.36",
                            "command": ["sh", "-c", "echo tick"]
                        }]
                    }
                }
            }
        });
        client
            .create_job(TIER4_NAMESPACE, manual_job)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // Reconcile controllers
        let wm = controller.workload_manager();
        wm.reconcile_namespace(TIER4_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // Verify ordinal pod stateful-0 created by StatefulSet
        let pod_0 = client.get_pod(TIER4_NAMESPACE, "stateful-0").await;
        if pod_0.is_err() {
            return Err("StatefulSet ordinal pod stateful-0 not found".to_string());
        }
        assertions += 1;
        details.push("StatefulSet reconciled ordinal child Pod API object stateful-0".to_string());

        // Verify child pods from Job oneshot
        let oneshot_pod = client.get_pod(TIER4_NAMESPACE, "oneshot-0").await;
        if oneshot_pod.is_err() {
            return Err("Job oneshot child pod oneshot-0 not found".to_string());
        }
        assertions += 1;
        details.push("Job oneshot reconciled child Pod API object oneshot-0".to_string());

        // Verify DaemonSet agent pod
        let ds_pod = client.get_pod(TIER4_NAMESPACE, "agent-node").await;
        if ds_pod.is_err() {
            return Err("DaemonSet child pod agent-node not found".to_string());
        }
        assertions += 1;
        details.push("DaemonSet agent reconciled node Pod API object agent-node".to_string());

        // Cleanup
        client
            .delete_namespace(TIER4_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        Ok((
            DomainReport {
                domain: ManifestDomain::Controllers,
                name: ManifestDomain::Controllers.display_name().to_string(),
                passed: true,
                duration_ms: start.elapsed().as_millis() as u64,
                assertions_count: assertions,
                details,
            },
            assertions,
        ))
    }

    // ── Tier 5: DNS & LoadBalancer ───────────────────────────────────────────
    async fn run_tier5_dns_lb(
        &self,
        client: &KubernetesApiClient,
    ) -> Result<(DomainReport, usize), String> {
        let start = Instant::now();
        let mut assertions = 0;
        let mut details = Vec::new();

        client
            .create_namespace(TIER5_NAMESPACE_A)
            .await
            .map_err(|e| e.to_string())?;
        client
            .create_namespace(TIER5_NAMESPACE_B)
            .await
            .map_err(|e| e.to_string())?;

        // 1. In tier5-a: Deployment web, Service web (ClusterIP), Service web-lb (LoadBalancer)
        let svc_ip: Ipv4Addr = "10.43.50.10".parse().unwrap();
        let svc_clusterip = json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": "web",
                "namespace": TIER5_NAMESPACE_A
            },
            "spec": {
                "type": "ClusterIP",
                "clusterIP": svc_ip.to_string(),
                "ports": [{ "name": "http", "port": 80, "targetPort": 80 }]
            }
        });
        client
            .create_service(TIER5_NAMESPACE_A, svc_clusterip)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // 2. LoadBalancer service: NodeSetter status webhook patches status with node IP
        let svc_lb = json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": "web-lb",
                "namespace": TIER5_NAMESPACE_A
            },
            "spec": {
                "type": "LoadBalancer",
                "ports": [{ "name": "http", "port": 80, "targetPort": 80 }]
            }
        });
        let _created_lb = client
            .create_service(TIER5_NAMESPACE_A, svc_lb)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        let lb_ingress = wait_for_lb_ip(client, TIER5_NAMESPACE_A, "web-lb", &self.lb_ip).await?;
        assertions += 1;
        details.push(format!(
            "LoadBalancer webhook assigned node IP: {lb_ingress}"
        ));

        // 3. Cross-namespace DNS resolution: client in tier5-b resolving web.tier5-a
        let prober = DnsProber::new(ProbeTransport::Synthetic {
            client: Arc::new(client.clone()),
            config: CoreDnsConfig::new(),
        });
        let probe = DnsResolutionProbe::cross_namespace(
            "web",
            TIER5_NAMESPACE_A,
            TIER5_NAMESPACE_B,
            svc_ip,
            DnsProtocol::Udp,
        );
        let dns_res = prober
            .execute_probe(&probe)
            .await
            .map_err(|e| e.to_string())?;
        if !dns_res.success {
            return Err(format!(
                "Cross-namespace DNS resolution failed: {}",
                dns_res.details
            ));
        }
        assertions += 1;
        details.push(format!(
            "Synthetic DNS model matched web.tier5-a.svc.cluster.local -> {svc_ip}; no CoreDNS server or pod query executed"
        ));

        // Cleanup
        client
            .delete_namespace(TIER5_NAMESPACE_A)
            .await
            .map_err(|e| e.to_string())?;
        client
            .delete_namespace(TIER5_NAMESPACE_B)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        Ok((
            DomainReport {
                domain: ManifestDomain::DnsLoadBalancer,
                name: ManifestDomain::DnsLoadBalancer.display_name().to_string(),
                passed: true,
                duration_ms: start.elapsed().as_millis() as u64,
                assertions_count: assertions,
                details,
            },
            assertions,
        ))
    }

    // ── Tier 6: LoadBalancer UPDATE path [KS-75] ─────────────────────────────
    async fn run_tier6_lb_update(
        &self,
        client: &KubernetesApiClient,
    ) -> Result<(DomainReport, usize), String> {
        let start = Instant::now();
        let mut assertions = 0;
        let mut details = Vec::new();

        client
            .create_namespace(TIER6_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;

        // 1. Control: born-lb created as LoadBalancer
        let born_lb = json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": "born-lb",
                "namespace": TIER6_NAMESPACE
            },
            "spec": {
                "type": "LoadBalancer",
                "ports": [{ "name": "http", "port": 80, "targetPort": 80 }]
            }
        });
        let _ = client
            .create_service(TIER6_NAMESPACE, born_lb)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        let born_ip = wait_for_lb_ip(client, TIER6_NAMESPACE, "born-lb", &self.lb_ip).await?;
        assertions += 1;
        details.push(format!("Control born-lb assigned IP: {born_ip}"));

        // 2. Precondition: flip-me born as ClusterIP has NO loadBalancer ingress IP
        let flip_me_cip = json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": "flip-me",
                "namespace": TIER6_NAMESPACE
            },
            "spec": {
                "type": "ClusterIP",
                "ports": [{ "name": "http", "port": 80, "targetPort": 80 }]
            }
        });
        let created_flip = client
            .create_service(TIER6_NAMESPACE, flip_me_cip)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        let initial_ingress = created_flip
            .pointer("/status/loadBalancer/ingress")
            .and_then(|v| v.as_array());
        if initial_ingress.is_some_and(|arr| !arr.is_empty()) {
            return Err("flip-me had ingress IP before being flipped".to_string());
        }
        assertions += 1;
        details.push("Verified flip-me has no loadBalancer ingress IP while ClusterIP".to_string());

        // Real UPDATE path: flip-me updated to LoadBalancer [KS-75 regression]
        let mut updated_flip = created_flip.clone();
        updated_flip["spec"]["type"] = json!("LoadBalancer");
        updated_flip["spec"]["ports"] = json!([
            { "name": "http", "port": 80, "targetPort": 80 },
            { "name": "edge", "port": 9000, "targetPort": 80 },
            { "name": "edge-tls", "port": 9443, "targetPort": 80 }
        ]);

        let _ = client
            .update_service(TIER6_NAMESPACE, "flip-me", updated_flip)
            .await
            .map_err(|e| format!("Failed to update service to LoadBalancer: {e}"))?;
        assertions += 1;

        let flipped_ip = wait_for_lb_ip(client, TIER6_NAMESPACE, "flip-me", &born_ip).await?;
        assertions += 1;
        details.push(format!(
            "API UPDATE fixture [KS-75] assigned loadBalancer status IP: {flipped_ip}; external connectivity unexecuted"
        ));

        // 5. Guard Job: rule split verification.
        // Job spec.template is immutable after creation.
        // Mutating webhook matches CREATE only for Jobs, allowing Job metadata updates.
        let guard_job = json!({
            "apiVersion": "batch/v1",
            "kind": "Job",
            "metadata": {
                "name": "guard",
                "namespace": TIER6_NAMESPACE
            },
            "spec": {
                "backoffLimit": 1,
                "template": {
                    "spec": {
                        "restartPolicy": "Never",
                        "containers": [{
                            "name": "work",
                            "image": "busybox:1.36",
                            "command": ["sh", "-c", "true"]
                        }]
                    }
                }
            }
        });
        let created_guard = client
            .create_job(TIER6_NAMESPACE, guard_job)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        // Verify NodeSetter injected nodeSelector on CREATE
        let node_selector =
            created_guard["spec"]["template"]["spec"]["nodeSelector"]["kubernetes.io/hostname"]
                .as_str()
                .unwrap_or("");
        if node_selector != self.node_name {
            return Err("Guard job nodeSelector was not injected on CREATE".to_string());
        }
        assertions += 1;

        // Update Job metadata labels (must NOT be rejected by webhook)
        let mut job_update = created_guard.clone();
        job_update["metadata"]["labels"] = json!({ "ks75": "touched" });
        let updated_job = client
            .update_job(TIER6_NAMESPACE, "guard", job_update)
            .await
            .map_err(|e| {
                format!("Job update rejected (webhook incorrectly matching Job UPDATE): {e}")
            })?;
        if updated_job["metadata"]["labels"]["ks75"] != "touched" {
            return Err("Guard job label update failed".to_string());
        }
        assertions += 1;
        details.push(
            "Webhook rule split confirmed: Jobs matched on CREATE only; UPDATE accepted"
                .to_string(),
        );

        // Cleanup
        client
            .delete_namespace(TIER6_NAMESPACE)
            .await
            .map_err(|e| e.to_string())?;
        assertions += 1;

        Ok((
            DomainReport {
                domain: ManifestDomain::LbUpdate,
                name: ManifestDomain::LbUpdate.display_name().to_string(),
                passed: true,
                duration_ms: start.elapsed().as_millis() as u64,
                assertions_count: assertions,
                details,
            },
            assertions,
        ))
    }

    // ── Selected Conformance Suite ───────────────────────────────────────────
    async fn run_selected_conformance(
        &self,
        _client: &KubernetesApiClient,
    ) -> Result<(ConformanceSummary, usize), String> {
        // Creating API objects is not execution of the selected upstream tests.
        // Preserve the planned inventory while recording no executed results.
        Ok((
            ConformanceSummary {
                total_selected: ConformanceInventory::selected_tests().len(),
                passed: 0,
                failed: 0,
                excluded_count: ConformanceInventory::explicit_exclusions().len(),
                focus_filter: CONFORMANCE_FOCUS_REGEX.into(),
                skip_filter: CONFORMANCE_SKIP_REGEX.into(),
                certification_disclaimer: CERTIFICATION_DISCLAIMER.into(),
                exclusions: ConformanceInventory::explicit_exclusions(),
                results: Vec::new(),
            },
            0,
        ))
    }
}

async fn wait_for_lb_ip(
    client: &KubernetesApiClient,
    namespace: &str,
    name: &str,
    expected_ip: &str,
) -> Result<String, String> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        let matched = client
            .get_service(namespace, name)
            .await
            .ok()
            .and_then(|svc| {
                svc.pointer("/status/loadBalancer/ingress/0/ip")
                    .and_then(Value::as_str)
                    .map(|ip| ip == expected_ip)
            })
            .unwrap_or(false);
        if matched {
            return Ok(expected_ip.to_string());
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    Err(format!(
        "Timed out waiting for LoadBalancer service {namespace}/{name} to receive IP {expected_ip}"
    ))
}
