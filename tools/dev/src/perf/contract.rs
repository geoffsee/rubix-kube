//! Contract gate evaluation comparing candidate against reference baselines.

use super::metrics::PerformanceReport;
use serde::{Deserialize, Serialize};

/// Direction and threshold rule for an individual gate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GateResult {
    pub name: String,
    pub reference_value: f64,
    pub candidate_value: f64,
    pub threshold_multiplier: f64,
    pub target_threshold: f64,
    pub higher_is_better: bool,
    pub passed: bool,
    pub details: String,
}

/// Comprehensive evaluation result comparing a candidate report against reference baseline.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GateEvaluationReport {
    pub architecture: String,
    pub reference_id: String,
    pub candidate_id: String,
    pub all_passed: bool,
    pub validation_errors: Vec<String>,
    pub results: Vec<GateResult>,
}

impl GateEvaluationReport {
    /// Evaluate all measurable engineering gates from the compatibility contract.
    pub fn evaluate(reference: &PerformanceReport, candidate: &PerformanceReport) -> Self {
        if let Err(error) = super::validation::validate_pair(reference, candidate) {
            return Self {
                architecture: candidate.architecture.as_str().into(),
                reference_id: reference.id.clone(),
                candidate_id: candidate.id.clone(),
                all_passed: false,
                results: Vec::new(),
                validation_errors: vec![error.to_string()],
            };
        }
        let mut results = Vec::new();

        // 1. Boot-to-API latency: candidate nearest-rank p95 <= 1.10 * ref p95
        let ref_boot = reference.startup_latencies.boot_to_api_seconds.p95;
        let cand_boot = candidate.startup_latencies.boot_to_api_seconds.p95;
        let boot_thresh = 1.10 * ref_boot;
        results.push(GateResult {
            name: "Boot-to-API Latency (p95)".to_string(),
            reference_value: ref_boot,
            candidate_value: cand_boot,
            threshold_multiplier: 1.10,
            target_threshold: boot_thresh,
            higher_is_better: false,
            passed: cand_boot <= boot_thresh,
            details: format!(
                "Candidate p95 ({cand_boot:.3}s) vs Reference p95 ({ref_boot:.3}s); limit: {boot_thresh:.3}s"
            ),
        });

        // 2. Node Ready latency: candidate p95 <= 1.10 * ref p95
        let ref_node = reference.startup_latencies.node_ready_seconds.p95;
        let cand_node = candidate.startup_latencies.node_ready_seconds.p95;
        let node_thresh = 1.10 * ref_node;
        results.push(GateResult {
            name: "Node Ready Latency (p95)".to_string(),
            reference_value: ref_node,
            candidate_value: cand_node,
            threshold_multiplier: 1.10,
            target_threshold: node_thresh,
            higher_is_better: false,
            passed: cand_node <= node_thresh,
            details: format!(
                "Candidate p95 ({cand_node:.3}s) vs Reference p95 ({ref_node:.3}s); limit: {node_thresh:.3}s"
            ),
        });

        // 3. First Pod (preloaded): candidate p95 <= 1.10 * ref p95
        let ref_first_pre = reference.startup_latencies.first_pod_preloaded_seconds.p95;
        let cand_first_pre = candidate.startup_latencies.first_pod_preloaded_seconds.p95;
        let first_pre_thresh = 1.10 * ref_first_pre;
        results.push(GateResult {
            name: "First Pod Latency (Preloaded, p95)".to_string(),
            reference_value: ref_first_pre,
            candidate_value: cand_first_pre,
            threshold_multiplier: 1.10,
            target_threshold: first_pre_thresh,
            higher_is_better: false,
            passed: cand_first_pre <= first_pre_thresh,
            details: format!(
                "Candidate p95 ({cand_first_pre:.3}s) vs Reference p95 ({ref_first_pre:.3}s); limit: {first_pre_thresh:.3}s"
            ),
        });

        // 4. First Pod (cold): candidate p95 <= 1.10 * ref p95
        let ref_first_cold = reference.startup_latencies.first_pod_cold_seconds.p95;
        let cand_first_cold = candidate.startup_latencies.first_pod_cold_seconds.p95;
        let first_cold_thresh = 1.10 * ref_first_cold;
        results.push(GateResult {
            name: "First Pod Latency (Cold Image, p95)".to_string(),
            reference_value: ref_first_cold,
            candidate_value: cand_first_cold,
            threshold_multiplier: 1.10,
            target_threshold: first_cold_thresh,
            higher_is_better: false,
            passed: cand_first_cold <= first_cold_thresh,
            details: format!(
                "Candidate p95 ({cand_first_cold:.3}s) vs Reference p95 ({ref_first_cold:.3}s); limit: {first_cold_thresh:.3}s"
            ),
        });

        // 5. Idle footprint: Median of per-run p95 PSS <= 1.10 * ref
        let ref_pss = reference.idle_footprint.summed_pss_bytes.p50;
        let cand_pss = candidate.idle_footprint.summed_pss_bytes.p50;
        let pss_thresh = 1.10 * ref_pss;
        results.push(GateResult {
            name: "Idle Footprint (Summed PSS, Median)".to_string(),
            reference_value: ref_pss,
            candidate_value: cand_pss,
            threshold_multiplier: 1.10,
            target_threshold: pss_thresh,
            higher_is_better: false,
            passed: cand_pss <= pss_thresh,
            details: format!(
                "Candidate median ({cand_pss:.0} bytes) vs Reference ({ref_pss:.0} bytes); limit: {pss_thresh:.0} bytes"
            ),
        });

        // 6. Idle footprint: Cgroup memory median <= 1.10 * ref
        let ref_cg = reference.idle_footprint.cgroup_memory_bytes.p50;
        let cand_cg = candidate.idle_footprint.cgroup_memory_bytes.p50;
        let cg_thresh = 1.10 * ref_cg;
        results.push(GateResult {
            name: "Idle Footprint (Cgroup Memory, Median)".to_string(),
            reference_value: ref_cg,
            candidate_value: cand_cg,
            threshold_multiplier: 1.10,
            target_threshold: cg_thresh,
            higher_is_better: false,
            passed: cand_cg <= cg_thresh,
            details: format!(
                "Candidate cgroup median ({cand_cg:.0} bytes) vs Reference ({ref_cg:.0} bytes); limit: {cg_thresh:.0} bytes"
            ),
        });

        // 7. Distribution size: compressed release archive bytes <= 1.10 * ref
        let ref_arch = reference.artifact_footprint.compressed_archive_bytes as f64;
        let cand_arch = candidate.artifact_footprint.compressed_archive_bytes as f64;
        let arch_thresh = 1.10 * ref_arch;
        results.push(GateResult {
            name: "Distribution Size (Compressed Archive)".to_string(),
            reference_value: ref_arch,
            candidate_value: cand_arch,
            threshold_multiplier: 1.10,
            target_threshold: arch_thresh,
            higher_is_better: false,
            passed: cand_arch <= arch_thresh,
            details: format!(
                "Candidate archive ({cand_arch:.0} bytes) vs Reference ({ref_arch:.0} bytes); limit: {arch_thresh:.0} bytes"
            ),
        });

        // 8. Distribution size: extracted executable/helpers bytes <= 1.10 * ref
        let ref_exec = reference.artifact_footprint.extracted_executable_bytes as f64;
        let cand_exec = candidate.artifact_footprint.extracted_executable_bytes as f64;
        let exec_thresh = 1.10 * ref_exec;
        results.push(GateResult {
            name: "Distribution Size (Extracted Executables)".to_string(),
            reference_value: ref_exec,
            candidate_value: cand_exec,
            threshold_multiplier: 1.10,
            target_threshold: exec_thresh,
            higher_is_better: false,
            passed: cand_exec <= exec_thresh,
            details: format!(
                "Candidate executables ({cand_exec:.0} bytes) vs Reference ({ref_exec:.0} bytes); limit: {exec_thresh:.0} bytes"
            ),
        });

        // 9. Distribution size: default image payload bytes <= 1.10 * ref
        let ref_payload = reference.artifact_footprint.default_image_payload_bytes as f64;
        let cand_payload = candidate.artifact_footprint.default_image_payload_bytes as f64;
        let payload_thresh = 1.10 * ref_payload;
        results.push(GateResult {
            name: "Distribution Size (Default Image Payload)".to_string(),
            reference_value: ref_payload,
            candidate_value: cand_payload,
            threshold_multiplier: 1.10,
            target_threshold: payload_thresh,
            higher_is_better: false,
            passed: cand_payload <= payload_thresh,
            details: format!(
                "Candidate images ({cand_payload:.0} bytes) vs Reference ({ref_payload:.0} bytes); limit: {payload_thresh:.0} bytes"
            ),
        });

        // 10. Pod density: Median capacity >= 0.90 * reference (HIGHER IS BETTER)
        let ref_density = reference.pod_density.max_ready_replicas.p50;
        let cand_density = candidate.pod_density.max_ready_replicas.p50;
        let density_thresh = 0.90 * ref_density;
        results.push(GateResult {
            name: "Pod Density Capacity (Median Replicas)".to_string(),
            reference_value: ref_density,
            candidate_value: cand_density,
            threshold_multiplier: 0.90,
            target_threshold: density_thresh,
            higher_is_better: true,
            passed: cand_density >= density_thresh,
            details: format!(
                "Candidate density ({cand_density:.1} pods) vs Reference ({ref_density:.1} pods); minimum: {density_thresh:.1} pods"
            ),
        });

        // 11. Sustained growth: final median <= 1.10 * initial median, 0 OOMs, 0 crashes
        let init_mem = candidate.sustained_growth.initial_settled_idle_median_bytes as f64;
        let final_mem = candidate.sustained_growth.final_settled_idle_median_bytes as f64;
        let growth_ratio = final_mem / init_mem;
        let growth_passed = init_mem > 0.0
            && candidate.sustained_growth.duration_hours >= 24
            && growth_ratio <= 1.10
            && candidate.sustained_growth.oom_kill_count == 0
            && candidate.sustained_growth.crash_count == 0
            && candidate.sustained_growth.unexplained_failures == 0;
        results.push(GateResult {
            name: "Sustained Growth (24h Soak)".to_string(),
            reference_value: init_mem,
            candidate_value: final_mem,
            threshold_multiplier: 1.10,
            target_threshold: init_mem * 1.10,
            higher_is_better: false,
            passed: growth_passed,
            details: format!(
                "Initial {init_mem:.0} bytes -> Final {final_mem:.0} bytes ({growth_ratio:.3}x), {}h, {} OOMs, {} crashes, {} unexplained failures",
                candidate.sustained_growth.duration_hours, candidate.sustained_growth.oom_kill_count,
                candidate.sustained_growth.crash_count, candidate.sustained_growth.unexplained_failures
            ),
        });

        // 12. Shutdown: graceful <= 30s, escalation <= 35s, 0 surviving processes
        let graceful_p95 = candidate.shutdown.graceful_duration_seconds.p95;
        let surviving = candidate.shutdown.surviving_owned_processes;
        let shutdown_passed = graceful_p95 <= 30.0
            && candidate.shutdown.escalation_duration_seconds.p95 <= 35.0
            && candidate.shutdown.graceful_duration_seconds.max <= 30.0
            && candidate.shutdown.escalation_duration_seconds.max <= 35.0
            && surviving == 0
            && candidate.shutdown.unrelated_processes_killed == 0;
        results.push(GateResult {
            name: "Shutdown & Process Cleanup".to_string(),
            reference_value: 30.0,
            candidate_value: graceful_p95,
            threshold_multiplier: 1.0,
            target_threshold: 30.0,
            higher_is_better: false,
            passed: shutdown_passed,
            details: format!(
                "Graceful p95: {graceful_p95:.2}s <= 30s, Surviving owned processes: {surviving}"
            ),
        });

        // Raw 900-sample run captures, soak logs, process receipts, libc/filesystem/variant
        // identity and source hashes have no verified importer in this slice. Fail closed
        // even if a caller labels supplied numbers as live data. This is arithmetic only.
        let validation_errors = vec![format!(
            "Not qualified: reference {:?}, candidate {:?}; verified live-capture provenance/import is not implemented",
            reference.evidence_kind, candidate.evidence_kind
        )];
        let all_passed = false;
        Self {
            architecture: candidate.architecture.as_str().to_string(),
            reference_id: reference.id.clone(),
            candidate_id: candidate.id.clone(),
            all_passed,
            validation_errors,
            results,
        }
    }
}
