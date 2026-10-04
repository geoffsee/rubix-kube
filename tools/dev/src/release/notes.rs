//! Candidate disclosures derived from the authoritative contracts.
use crate::Result;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseNotes;
impl ReleaseNotes {
    #[must_use]
    pub const fn build() -> Self {
        Self
    }
    /// Fixture diagnostics establish neither production readiness nor measured downtime.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        format!(
            "# Rubix v0.1.0 candidate release notes\n\n**Status: UNQUALIFIED_FIXTURE_ONLY. C13/C14/C16/C17 remain pending.**\n\nRubix develops a single-node distribution supervising retained kube-apiserver, kube-controller-manager, kubelet, kube-proxy, Kine and containerd executables. Runtime shims, CNI programs, snapshotter helpers and default addon images remain part of distribution accounting. These diagnostics do not qualify an installed retained node, authentication, DNS/storage, container support, or platform behavior.\n\n## Migration and recovery boundaries\n\nThe modeled starting versions are v1.1.8, v1.2.0, v1.3.0 and v1.3.1 through v1.3.3. No live Go-to-Rust migration or downtime was measured. There is no qualified production cutover or promised downtime window. Preserve verified configuration, PKI, Kine SQLite/WAL and persistent-volume backups. Experimental RUBXSNP1 snapshots do not replace or adopt a Kine SQLite database. The selected production API-server/Kine contract requires loopback mTLS with a dedicated datastore CA and client identity; plaintext spike evidence cannot qualify that transport.\n\n## Compatibility, deprecations and deviations\n\nPreserve kubesolo.io/v1alpha1, KUBESOLO_* inputs, /etc/kubesolo and /var/lib/kubesolo defaults. KUBESOLO_FULL/--full are deprecated no-ops. NodeSetter and YAML/JSON kubeconfig parser fixtures are implementation diagnostics, not proof of live authentication or service behavior. D01 through D11 are defined by the compatibility contract reproduced below; they are deliberate design obligations, not qualified release capabilities.\n\n## Performance\n\nThe authoritative twelve gates use default 1.10 bounds for startup, aggregate idle memory and artifact sizes, density >= 0.90 of reference, >=24h sustained aggregate growth <=1.10, zero failures, and shutdown deadlines of30/35 seconds. Synthetic arithmetic passing does not qualify live performance. The authoritative policy and exact gate table are reproduced below.\n\n## Certification\n\n{}\n\n## Authoritative compatibility contract\n\n{}\n\n## Authoritative performance policy\n\n{}",
            crate::conformance::CERTIFICATION_DISCLAIMER,
            include_str!("../../../../docs/architecture/compatibility-contract.md"),
            include_str!("../../../../docs/architecture/performance-rebaseline-policy.md")
        )
    }
}
pub fn verify_release_notes_completeness(notes: &ReleaseNotes) -> Result<()> {
    if !notes.to_markdown().contains("UNQUALIFIED_FIXTURE_ONLY") {
        return Err("missing unqualified status".into());
    }
    Ok(())
}
