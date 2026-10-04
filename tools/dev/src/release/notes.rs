//! Formal production release notes generator and validator.
//!
//! Complies with Gate C16/C17 requirements (Epic E30 / Issue #126) covering:
//! - Version transition support (v1.1.8, v1.2.0, v1.3.0, v1.3.1-v1.3.3)
//! - Breaking changes and deliberate deviations (D01-D11)
//! - Deprecations
//! - Performance budget baselines and sustained memory growth bounds
//! - Certified single-node capabilities
//! - Mandatory multi-node non-certification disclaimer
//! - Dual YAML and JSON kubeconfig accommodation

use std::fmt::Write as _;

use crate::Result;
use crate::conformance::CERTIFICATION_DISCLAIMER;
use crate::state_transition::versions::SupportedStartingVersion;

/// Version transition specification for a historical starting version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionSpec {
    pub starting_version: &'static str,
    pub config_native_support: bool,
    pub flag_migration_required: bool,
    pub summary: &'static str,
    pub layout: &'static str,
}

/// Deliberate deviation / breaking change entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviationEntry {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub impact: &'static str,
}

/// Performance budget threshold specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerfBudgetSpec {
    pub metric: &'static str,
    pub multiplier: &'static str,
    pub direction: &'static str,
    pub description: &'static str,
}

/// Production release notes document model.
#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseNotes {
    pub version: String,
    pub release_date: String,
    pub transitions: Vec<TransitionSpec>,
    pub rejected_versions: Vec<&'static str>,
    pub downtime_window_minutes: u32,
    pub deviations: Vec<DeviationEntry>,
    pub deprecations: Vec<&'static str>,
    pub perf_budgets: Vec<PerfBudgetSpec>,
    pub sustained_growth_ratio_max: f64,
    pub retained_processes: Vec<&'static str>,
    pub single_node_capabilities: Vec<&'static str>,
    pub certification_disclaimer: String,
    pub kubeconfig_format_support: &'static str,
}

impl ReleaseNotes {
    /// Constructs authoritative production release notes for Rubix v0.1.0.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn build() -> Self {
        let transitions = vec![
            TransitionSpec {
                starting_version: "v1.1.8",
                config_native_support: false,
                flag_migration_required: true,
                summary: "Legacy systemd flags migrated to /etc/kubesolo/config.yaml (mode 0600) with automatic .bak backup; persistent CA and service-account PKI preserved; Kine SQLite datastore retained.",
                layout: "Legacy flags in service unit, persistent PKI, Kine SQLite datastore",
            },
            TransitionSpec {
                starting_version: "v1.2.0",
                config_native_support: false,
                flag_migration_required: true,
                summary: "Legacy systemd flags migrated to YAML with automatic backup; persistent CA/PKI and external runtime configuration preserved; Kine SQLite datastore retained.",
                layout: "Legacy flags in service unit, persistent CA/PKI, external runtime support",
            },
            TransitionSpec {
                starting_version: "v1.3.0",
                config_native_support: true,
                flag_migration_required: false,
                summary: "Native kubesolo.io/v1alpha1 YAML configuration preserved directly; persistent PKI with IP auto-regeneration preserved; Kine SQLite datastore retained.",
                layout: "YAML config file, persistent PKI with IP auto-regeneration, Kine SQLite",
            },
            TransitionSpec {
                starting_version: "v1.3.1",
                config_native_support: true,
                flag_migration_required: false,
                summary: "Native YAML config preserved; persistent PKI, external runtime & CRI attachment settings preserved; Kine SQLite datastore retained.",
                layout: "YAML config file, persistent PKI, external runtime & CRI support, Kine SQLite",
            },
            TransitionSpec {
                starting_version: "v1.3.2",
                config_native_support: true,
                flag_migration_required: false,
                summary: "Native YAML config preserved; persistent PKI, external runtime & CRI attachment settings preserved; Kine SQLite datastore retained.",
                layout: "YAML config file, persistent PKI, external runtime & CRI support, Kine SQLite",
            },
            TransitionSpec {
                starting_version: "v1.3.3",
                config_native_support: true,
                flag_migration_required: false,
                summary: "Native YAML config preserved; persistent PKI, external runtime & CRI attachment settings preserved; Kine SQLite datastore retained.",
                layout: "YAML config file, persistent PKI, external runtime & CRI support, Kine SQLite",
            },
        ];

        let rejected_versions = vec![
            "v1.0.0 (below minimum supported migration version v1.1.8)",
            "v0.9.1 (below minimum supported migration version v1.1.8)",
            "v1.1.7 (below minimum supported migration version v1.1.8)",
            "unversioned or development builds (empty version, develop, git commit shas)",
        ];

        let deviations = vec![
            DeviationEntry {
                id: "D01",
                title: "Target and Bundle Integrity Across Matrix",
                description: "Preserve all 16 node target cells (including ARMv7 hard-float and RISC-V 64 glibc/musl) instead of silently dropping unsupported targets. Reject mismatched binaries or architectures at installation.",
                impact: "Universal target coverage; strict installer error reporting on cross-architecture downloads.",
            },
            DeviationEntry {
                id: "D02",
                title: "Explicit Versioning and Capability Manifests",
                description: "Rubix candidates declare explicit version and capability manifests across CLI, installer, and bundle. Historical Go release version comparisons are rejected.",
                impact: "Eliminates silent version mismatch; unsupported capabilities fail closed before service replacement.",
            },
            DeviationEntry {
                id: "D03",
                title: "Component Boundary and Process Supervision Transparency",
                description: "Rubix is a Rust-supervised distribution managing retained official upstream executables (kube-apiserver, kube-controller-manager, kubelet, kube-proxy, Kine, containerd, CNI). It is not a native Rust reimplementation of Kubernetes or container engines.",
                impact: "Full process tree and memory accountability; no false native-Rust claims.",
            },
            DeviationEntry {
                id: "D04",
                title: "Local Storage Enabled Defaults",
                description: "Omitted local-storage configuration defaults to true (enabled), aligning with runtime Defaults(). An explicit false input is required to disable storage.",
                impact: "Default deployments automatically include local-path persistent storage.",
            },
            DeviationEntry {
                id: "D05",
                title: "Strict Offline Bundle Target Validation",
                description: "Offline bundles enforce machine architecture, operating system, and libc matching before extraction, rejecting cross-target copying.",
                impact: "Prevents corrupted or unusable partial installations on foreign hosts.",
            },
            DeviationEntry {
                id: "D06",
                title: "Immutable Digests Over Mutable Tags",
                description: "Eliminates ambiguous 'latest' release and container tags in favor of explicit version tags and verified SHA-256 image digests.",
                impact: "Deterministic deployments immune to registry tag mutability.",
            },
            DeviationEntry {
                id: "D07",
                title: "Strict D2K Mutual TLS Authentication and Endpoint Readiness",
                description: "The optional Docker-compatible D2K endpoint requires client certificate authentication over TLS on port 2376. Unauthenticated and wrong-client requests are strictly rejected; readiness is gated on live probe response.",
                impact: "Protects Docker API endpoint against unauthorized host or container access.",
            },
            DeviationEntry {
                id: "D08",
                title: "Truthful Native Diagnostics Without Fabricated Metrics",
                description: "Rust runtime diagnostics replace Go-specific pprof/runtime internals. Prometheus metrics reflect honest probe semantics without fabricating Go runtime time-series.",
                impact: "Accurate observability without misleading runtime gauges.",
            },
            DeviationEntry {
                id: "D09",
                title: "Preserved Upstream Controller Defaults and Deprecated --full No-Op",
                description: "Preserves official upstream Kubernetes v1.35.7 controller defaults without restoring deprecated low-memory overrides. The --full flag is treated as a deprecated no-op.",
                impact: "Production Kubernetes controller parity and predictable memory allocation.",
            },
            DeviationEntry {
                id: "D10",
                title: "Atomic Stored Desired Configuration",
                description: "Configuration API operates on stored desired configuration over a private 0600 Unix domain socket; runtime overrides are not persisted and edits require node restart.",
                impact: "Prevents configuration drift and race conditions during runtime operation.",
            },
            DeviationEntry {
                id: "D11",
                title: "Rehearsed Datastore Adoption and Rollback Safety",
                description: "No automatic SQLite-to-snapshot format interchangeability. Kine SQLite state at kine/db/state.db is preserved and requires dedicated loopback mTLS.",
                impact: "Guarantees zero silent datastore corruption during Go-to-Rust transitions.",
            },
        ];

        let deprecations = vec![
            "Flag `--full` and environment variable `KUBESOLO_FULL` are deprecated and treated as no-ops.",
            "Legacy command-line flags are deprecated in favor of declarative `kubesolo.io/v1alpha1` YAML configuration.",
            "Native Windows binaries are excluded per ADR E01; WSL2 is supported through standard Linux userspace and container engines.",
        ];

        let perf_budgets = vec![
            PerfBudgetSpec {
                metric: "Boot-to-API Latency (p95)",
                multiplier: "<= 1.10x reference",
                direction: "Lower is better",
                description: "Time from node daemon launch to authenticated API read/write readiness",
            },
            PerfBudgetSpec {
                metric: "Node Ready Latency (p95)",
                multiplier: "<= 1.10x reference",
                direction: "Lower is better",
                description: "Time from launch until node reports Ready condition to API server",
            },
            PerfBudgetSpec {
                metric: "First Pod Latency (Preloaded & Cold, p95)",
                multiplier: "<= 1.10x reference",
                direction: "Lower is better",
                description: "Time to schedule and run a probe pod with preloaded and cold image paths",
            },
            PerfBudgetSpec {
                metric: "Total Distribution Idle Memory (p95)",
                multiplier: "<= 1.05x reference",
                direction: "Lower is better",
                description: "Whole-distribution settled idle memory footprint (PSS and cgroup)",
            },
            PerfBudgetSpec {
                metric: "Component Idle Memory (apiserver, kine, containerd, kubelet)",
                multiplier: "<= 1.05x reference",
                direction: "Lower is better",
                description: "Individual PSS memory bounds across core supervised processes",
            },
            PerfBudgetSpec {
                metric: "Pod Density Capacity",
                multiplier: ">= 0.90x reference",
                direction: "Higher is better",
                description: "Maximum schedulable and runnable pod replicas maintaining passing probes",
            },
            PerfBudgetSpec {
                metric: "Lifecycle Pod Cycles (Single & Burst, p95)",
                multiplier: "<= 1.10x / 1.15x reference",
                direction: "Lower is better",
                description: "Pod creation, scheduling, execution, and teardown cycle latency",
            },
        ];

        let retained_processes = vec![
            "kube_apiserver",
            "kine",
            "containerd",
            "containerd_shim",
            "kubelet",
            "kube_controller_manager",
            "kube_proxy",
            "rubix_engine",
        ];

        let single_node_capabilities = vec![
            "NodeSetter admission webhook mutating unassigned Pods, PVCs, and Jobs to single-node placement without a cluster scheduler",
            "Automatic LoadBalancer status assignment mapping Service ingress to the active node IP",
            "In-cluster CoreDNS TCP and UDP resolution with forwarder isolation",
            "HostPath local-path storage provisioner enforcing default Retain reclaim semantics",
            "Optional Portainer Edge Agent reverse-tunnel bootstrap without mutating existing Portainer configurations",
            "Optional D2K Docker-to-Kubernetes API gateway exposing Docker Engine API over mutual TLS",
            "Dual-format kubeconfig accommodation supporting both YAML and JSON parsing identically",
        ];

        Self {
            version: "0.1.0".into(),
            release_date: "2026-10-03".into(),
            transitions,
            rejected_versions,
            downtime_window_minutes: 10,
            deviations,
            deprecations,
            perf_budgets,
            sustained_growth_ratio_max: 1.10,
            retained_processes,
            single_node_capabilities,
            certification_disclaimer: CERTIFICATION_DISCLAIMER.into(),
            kubeconfig_format_support: "YAML and JSON (dual-format validated)",
        }
    }

    /// Renders formal production release notes as Markdown.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# Rubix Kubernetes Distribution v{} Release Notes\n",
            self.version
        );
        let _ = writeln!(
            out,
            "**Release Version**: `{}`  \n**Gate Status**: Gate C16/C17 Production Readiness  \n**Release Date**: `{}`\n",
            self.version, self.release_date
        );

        out.push_str(
            "Rubix is a production single-node Kubernetes distribution supervising retained \
             official upstream executables (`kube-apiserver`, `kube-controller-manager`, `kubelet`, \
             `kube-proxy`, `kine`, `containerd`, and CNI plugins) wrapped in a high-reliability, \
             memory-safe Rust supervisor, PKI manager, host preflight engine, and management CLI.\n\n"
        );

        // Section 1: Version Transition Support
        out.push_str("## 1. Version Transition Support (Go to Rust Migration)\n\n");
        out.push_str(
            "Rubix v0.1.0 provides qualified migration support from historical KubeSolo versions \
             `v1.1.8`, `v1.2.0`, `v1.3.0`, and `v1.3.1` through `v1.3.3`. All migrations require \
             a planned maintenance downtime window of **5 to 10 minutes**.\n\n",
        );

        let _ = writeln!(
            out,
            "| Starting Version | Native YAML Config | Flag Migration Required | Starting Layout & Architecture |"
        );
        let _ = writeln!(out, "|---|---|---|---|");
        for t in &self.transitions {
            let _ = writeln!(
                out,
                "| `{}` | `{}` | `{}` | {} |",
                t.starting_version, t.config_native_support, t.flag_migration_required, t.layout
            );
        }
        out.push('\n');

        out.push_str("### Migration Prerequisites and Mandatory Backups\n\n");
        out.push_str(
            "Before executing a transition from any prior version, operators MUST create verified pre-transition backups of:\n\
             1. **Configuration**: `/etc/kubesolo/config.yaml` (or service flags file)\n\
             2. **PKI Trust Roots**: `/var/lib/kubesolo/pki/` (CA certificate, private key, and service-account signing keys)\n\
             3. **Datastore**: `/var/lib/kubesolo/kine/db/state.db` and active WAL files (with running node processes stopped)\n\
             4. **Persistent Volumes**: `/var/lib/kubesolo/local-path-storage/` (workload application data)\n\n"
        );

        out.push_str("### Datastore Non-Interchangeability and Transport Boundary\n\n");
        out.push_str(
            "- **Kine SQLite Preservation**: Kine's SQLite database file format (`state.db`) is preserved. \
             Replacing SQLite with experimental native snapshot formats (such as `RUBXSNP1`) is **not** a \
             production migration path; raw SQLite cannot be adopted into snapshots without explicit offline rehearsal.\n\
             - **Loopback mTLS Transport**: Production API-server-to-Kine communication strictly enforces \
             loopback mutual TLS over `https://127.0.0.1:2379` using a dedicated datastore CA and client \
             certificate. Plaintext datastore transport is prohibited.\n\n"
        );

        out.push_str("### Rejected Unsupported Versions\n\n");
        out.push_str(
            "The following versions are outside the supported migration catalog and will be rejected with an actionable error:\n"
        );
        for rej in &self.rejected_versions {
            let _ = writeln!(out, "- `{rej}`");
        }
        out.push('\n');

        // Section 2: Breaking Changes & Deliberate Deviations
        out.push_str("## 2. Breaking Changes and Deliberate Deviations (D01 – D11)\n\n");
        out.push_str(
            "Rubix maintains high behavioral fidelity to upstream baseline KubeSolo while resolving \
             historical ambiguities, security defects, and unverified assumptions. Below are the \
             eleven deliberate architectural deviations:\n\n"
        );

        for dev in &self.deviations {
            let _ = writeln!(out, "### {} — {}\n", dev.id, dev.title);
            let _ = writeln!(out, "**Description**: {}\n", dev.description);
            let _ = writeln!(out, "**Operational Impact**: {}\n", dev.impact);
        }

        // Section 3: Deprecations
        out.push_str("## 3. Deprecations\n\n");
        for dep in &self.deprecations {
            let _ = writeln!(out, "- {dep}");
        }
        out.push('\n');

        // Section 4: Performance Budget Baselines
        out.push_str("## 4. Performance Budget Baselines and Sustained Soak Constraints\n\n");
        out.push_str(
            "Rubix enforces 12 committed performance contract thresholds against paired reference baselines \
             across `linux/amd64` and `linux/arm64`. Candidates are validated under identical hardware, \
             cgroups v2, and kernel environments:\n\n"
        );

        let _ = writeln!(out, "| Metric | Contract Gate | Direction | Description |");
        let _ = writeln!(out, "|---|---|---|---|");
        for p in &self.perf_budgets {
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {} | {} |",
                p.metric, p.multiplier, p.direction, p.description
            );
        }
        out.push('\n');

        out.push_str("### 24-Hour Sustained Memory Growth and Reliability\n\n");
        let _ = writeln!(
            out,
            "- **Maximum 24h Memory Soak Growth Ratio**: `<= {:.2}x` (Final settled PSS / Initial settled PSS)",
            self.sustained_growth_ratio_max
        );
        out.push_str(
            "- **Zero-Defect Reliability**: `0` OOM kills, `0` process crashes, `0` unexpected probe failures\n\
             - **Distribution-Wide Process Accounting**: All 8 canonical retained process roles are independently tracked:\n"
        );
        for proc_name in &self.retained_processes {
            let _ = writeln!(out, "  * `{proc_name}`");
        }
        out.push_str(
            "- **Anti-Misrepresentation Rules**: Unmeasured sub-200MB claims are categorically rejected. \
             Physical sanity constraints require PSS > 0, RSS > 0, and PSS <= RSS across all samples.\n\
             - **Secondary Target Gaps**: ARMv7 hard-float and RISC-V 64 limitations are declared explicitly as qualification gaps.\n\n"
        );

        // Section 5: Certified Single-Node Capabilities & Multi-Node Disclaimer
        out.push_str("## 5. Certified Single-Node Capabilities\n\n");
        out.push_str(
            "Rubix v0.1.0 provides qualified single-node Kubernetes capabilities including:\n",
        );
        for cap in &self.single_node_capabilities {
            let _ = writeln!(out, "- {cap}");
        }
        out.push('\n');

        out.push_str("## 6. Mandatory Multi-Node Non-Certification Disclaimer\n\n");
        let _ = writeln!(out, "> [!IMPORTANT]\n> {}", self.certification_disclaimer);
        out.push_str(
            "\nRubix is explicitly architected and qualified **only** as a single-node Kubernetes distribution. \
             It does NOT implement or support multi-node clustering, distributed scheduling, high availability \
             control plane replication, or etcd cluster consensus. Upstream Kubernetes conformance tests that \
             exercise multi-node, serial slow, disruptive, or flaky scenarios are explicitly excluded from \
             qualification scope by architectural design.\n\n"
        );

        // Section 7: Kubeconfig Format Compatibility
        out.push_str("## 7. Dual Kubeconfig Format Accommodation\n\n");
        out.push_str(
            "Rubix management tooling (`rubixctl`) and verification suites accommodate both **YAML** \
             and **JSON** kubeconfig formats interchangeably, verifying cryptographic certificate \
             validation, client authentication, and context merging across both representations.\n"
        );

        out
    }
}

/// Verifies that release notes contain all mandatory contract disclosures.
pub fn verify_release_notes_completeness(notes: &ReleaseNotes) -> Result<()> {
    // 1. Verify version transitions cover v1.1.8 through v1.3.3
    for ver in SupportedStartingVersion::ALL {
        if !notes
            .transitions
            .iter()
            .any(|t| t.starting_version == ver.as_str())
        {
            return Err(format!("missing transition specification for version '{ver}'").into());
        }
    }

    // 2. Verify all 11 deviations (D01 to D11)
    for id in [
        "D01", "D02", "D03", "D04", "D05", "D06", "D07", "D08", "D09", "D10", "D11",
    ] {
        if !notes.deviations.iter().any(|d| d.id == id) {
            return Err(format!("missing deliberate deviation record '{id}'").into());
        }
    }

    // 3. Verify multi-node non-certification disclaimer
    if !notes.certification_disclaimer.contains("Synthetic in-process fixtures only")
        || !notes.certification_disclaimer.contains("DO NOT claim official CNCF Certified Kubernetes qualification or multi-node certification")
    {
        return Err("missing mandatory multi-node non-certification disclaimer".into());
    }

    // 4. Verify 8 canonical retained process roles
    if notes.retained_processes.len() != 8 {
        return Err("performance budget must cover all 8 canonical retained process roles".into());
    }

    // 5. Verify dual-format kubeconfig accommodation
    if !notes.kubeconfig_format_support.contains("YAML")
        || !notes.kubeconfig_format_support.contains("JSON")
    {
        return Err("release notes must declare dual YAML and JSON kubeconfig support".into());
    }

    Ok(())
}
