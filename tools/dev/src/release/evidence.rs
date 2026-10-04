//! Explicitly unqualified fixture evidence; production verification fails closed.
use crate::{
    Result,
    conformance::{QualificationReport, QualificationRunner},
    perf::{GateEvaluationReport, PerformanceReport, load_report},
    provenance::LicenseInventory,
    release::{
        attribution::{AttributionRecord, verify_attribution_completeness},
        manifest::{assemble_checksum_manifest, verify_checksum_inventory},
        notes::ReleaseNotes,
    },
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, process::Command};
const STATUS: &str = "UNQUALIFIED_FIXTURE_ONLY";
const SOURCES: [&str; 4] = [
    "amd64-reference-go.json",
    "amd64-candidate-rust.json",
    "arm64-reference-go.json",
    "arm64-candidate-rust.json",
];
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerfQualificationDocument {
    pub schema_version: u32,
    pub status: String,
    pub amd64_evaluation: GateEvaluationReport,
    pub arm64_evaluation: GateEvaluationReport,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateEvidence {
    pub schema_version: u32,
    pub status: String,
    pub observed_transitions: Vec<String>,
    pub qualified: bool,
    pub note: String,
}
fn state_unobserved() -> StateEvidence {
    StateEvidence { schema_version: 2, status: STATUS.into(), observed_transitions: vec![], qualified: false, note: "No live retained-node/Kine transition, PKI/PV reconciliation or downtime measurement was executed. C16/C17 remain pending; rehearsal fixtures are not production migration evidence.".into() }
}
fn metadata(root: &Path) -> Result<String> {
    let output = Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}
fn performance(dir: &Path) -> Result<PerfQualificationDocument> {
    let reports = SOURCES
        .iter()
        .map(|name| load_report(&dir.join(name)))
        .collect::<Result<Vec<PerformanceReport>>>()?;
    for report in &reports {
        crate::perf::validation::validate_report(report)?;
    }
    let amd64_evaluation = GateEvaluationReport::evaluate(&reports[0], &reports[1]);
    let arm64_evaluation = GateEvaluationReport::evaluate(&reports[2], &reports[3]);
    if reports[0].architecture != crate::perf::Architecture::Amd64
        || reports[2].architecture != crate::perf::Architecture::Arm64
        || !amd64_evaluation.arithmetic_all_passed()
        || !arm64_evaluation.arithmetic_all_passed()
    {
        return Err("invalid paired fixture performance observations".into());
    }
    Ok(PerfQualificationDocument {
        schema_version: 2,
        status: STATUS.into(),
        amd64_evaluation,
        arm64_evaluation,
    })
}
/// Production assembly requires a live evidence importer which is not implemented.
pub async fn assemble_release_evidence(_root: &Path, _target: &Path) -> Result<()> {
    Err("production release assembly unavailable: no verified current-source live evidence importer; use assemble-fixtures for unqualified diagnostics".into())
}
/// Materialize fixture diagnostics without fabricated package digests or transitions.
pub async fn assemble_fixture_evidence(root: &Path, target: &Path) -> Result<()> {
    if target.exists() && fs::read_dir(target)?.next().is_some() {
        return Err("fixture output must be empty".into());
    }
    fs::create_dir_all(target)?;
    for name in SOURCES {
        fs::copy(
            root.join("tools/perf/fixtures").join(name),
            target.join(name),
        )?;
    }
    let perf = performance(target)?;
    fs::write(
        target.join("performance-qualification-report.json"),
        serde_json::to_vec_pretty(&perf)?,
    )?;
    let conformance = QualificationRunner::new().run_fixture().await?;
    fs::write(
        target.join("conformance-qualification-report.json"),
        conformance.to_json()?,
    )?;
    fs::write(
        target.join("conformance-qualification-report.md"),
        conformance.to_markdown(),
    )?;
    fs::write(
        target.join("state-transition-qualification-report.json"),
        serde_json::to_vec_pretty(&state_unobserved())?,
    )?;
    let attribution = AttributionRecord::build(&metadata(root)?)?;
    verify_attribution_completeness(&attribution)?;
    fs::write(
        target.join("licenses.json"),
        serde_json::to_vec_pretty(&attribution.to_license_inventory())?,
    )?;
    fs::write(target.join("attribution.md"), attribution.to_markdown())?;
    fs::write(
        target.join("release-notes.md"),
        ReleaseNotes::build().to_markdown(),
    )?;
    fs::write(
        target.join("README.md"),
        "# Release fixture diagnostics\n\nUNQUALIFIED_FIXTURE_ONLY. C13/C14/C16/C17 remain pending. No release archives or management binaries are fabricated or bound here; no OCI artifact hashes are synthesized. Performance sources are synthetic observations, conformance is in-process, and no live state transition or downtime was observed. License records are inventory drafts, not a legal compliance certification.\n\nVerify diagnostic consistency with `cargo run --locked -p rubix-dev --bin rubix-release -- verify-fixtures docs/release`. Production `verify` always fails closed until a verified current-source live evidence importer exists.\n",
    )?;
    fs::write(
        target.join("SHA256SUMS"),
        assemble_checksum_manifest(target)?,
    )?;
    verify_fixture_evidence(root, target)
}
/// Checks diagnostic integrity against raw observations and the current locked inventory.
pub fn verify_fixture_evidence(root: &Path, dir: &Path) -> Result<()> {
    verify_checksum_inventory(&fs::read_to_string(dir.join("SHA256SUMS"))?, dir)?;
    let expected_perf = performance(dir)?;
    // Compare the canonical rendered result to avoid accepting caller pass flags,
    // and avoid lossy JSON float parsing changing a recomputed threshold.
    if fs::read(dir.join("performance-qualification-report.json"))?
        != serde_json::to_vec_pretty(&expected_perf)?
    {
        return Err("performance evaluations disagree with recomputed source reports".into());
    }
    let conf: QualificationReport = serde_json::from_slice(&fs::read(
        dir.join("conformance-qualification-report.json"),
    )?)?;
    conf.verify_fixture()?;
    if fs::read_to_string(dir.join("conformance-qualification-report.md"))? != conf.to_markdown() {
        return Err("conformance text disagrees with fixture report".into());
    }
    let state: StateEvidence = serde_json::from_slice(&fs::read(
        dir.join("state-transition-qualification-report.json"),
    )?)?;
    if state != state_unobserved() {
        return Err("unobserved fixture must not claim successful state transitions".into());
    }
    let expected = AttributionRecord::build(&metadata(root)?)?;
    verify_attribution_completeness(&expected)?;
    let inventory: LicenseInventory =
        serde_json::from_slice(&fs::read(dir.join("licenses.json"))?)?;
    if inventory != expected.to_license_inventory()
        || fs::read_to_string(dir.join("attribution.md"))? != expected.to_markdown()
    {
        return Err(
            "published attribution does not match locked component/dependency inventory".into(),
        );
    }
    if fs::read_to_string(dir.join("release-notes.md"))? != ReleaseNotes::build().to_markdown() {
        return Err("release notes disagree with unqualified candidate disclosures".into());
    }
    if !fs::read_to_string(dir.join("README.md"))?.contains(STATUS) {
        return Err("fixture overview must declare unqualified status".into());
    }
    verify_kubeconfig_dual_format_accommodation()
}
/// No fixture or self-attested flags can establish production release qualification.
pub fn verify_release_evidence(_dir: &Path) -> Result<()> {
    Err("production release qualification unavailable: independent current-source Linux runtime, performance, state-transition and artifact evidence importer is not implemented; C13/C14/C16/C17 remain pending".into())
}
/// Validate the kubeconfig parser's YAML and JSON accommodation without live claims.
pub fn verify_kubeconfig_dual_format_accommodation() -> Result<()> {
    let value = serde_json::json!({"apiVersion":"v1","kind":"Config","clusters":[{"name":"rubix","cluster":{"server":"https://127.0.0.1:6443","certificate-authority-data":"Y2E="}}],"users":[{"name":"admin","user":{"client-certificate-data":"Y2VydA==","client-key-data":"a2V5"}}],"contexts":[{"name":"admin","context":{"cluster":"rubix","user":"admin"}}],"current-context":"admin"});
    let json = serde_json::to_vec(&value)?;
    let yaml = b"apiVersion: v1\nkind: Config\nclusters:\n- name: rubix\n  cluster:\n    server: https://127.0.0.1:6443\n    certificate-authority-data: Y2E=\nusers:\n- name: admin\n  user:\n    client-certificate-data: Y2VydA==\n    client-key-data: a2V5\ncontexts:\n- name: admin\n  context:\n    cluster: rubix\n    user: admin\ncurrent-context: admin\n";
    let left = crate::state_transition::parse_kubeconfig(&json)?;
    let right = crate::state_transition::parse_kubeconfig(yaml)?;
    if left.server != right.server
        || left.cluster_name != right.cluster_name
        || left.user_name != right.user_name
        || left.current_context != right.current_context
        || left.ca_cert_bytes != right.ca_cert_bytes
        || left.client_cert_bytes != right.client_cert_bytes
        || left.client_key_bytes != right.client_key_bytes
    {
        return Err("YAML and JSON kubeconfig parsing differs".into());
    }
    Ok(())
}
