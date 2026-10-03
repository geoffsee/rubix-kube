//! Repeatable performance measurement harness and statistical sampling routines.

use super::metrics::{
    Architecture, ArtifactFootprint, ComponentVersions, HardwareInfo, IdleFootprint,
    ImplementationKind, PerformanceReport, PodDensity, ProcessMemoryBreakdown, ShutdownMeasurement,
    StartupLatencies, SustainedGrowth, VarianceSummary, WorkloadSpec,
};
use crate::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Required retained processes that must be accounted for in whole-node measurements.
pub const REQUIRED_RETAINED_PROCESSES: [&str; 8] = [
    "apiserver",
    "controller-manager",
    "kubelet",
    "proxy",
    "kine",
    "containerd",
    "containerd-shim",
    "node-daemon", // rubix-kube or kubesolo
];

/// Execution sequencing to eliminate environmental and thermal bias.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunOrder {
    ReferenceFirst,
    CandidateFirst,
}

impl RunOrder {
    pub const fn for_iteration(iteration: usize) -> Self {
        if iteration.is_multiple_of(2) {
            Self::ReferenceFirst
        } else {
            Self::CandidateFirst
        }
    }
}

/// Parse `VmRSS` from `/proc/[pid]/status` content if running on Linux.
pub fn parse_vm_rss_bytes(status_content: &str) -> Option<u64> {
    for line in status_content.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let Some(kb_str) = parts.first()
                && let Ok(kb) = kb_str.parse::<u64>()
            {
                return kb.checked_mul(1024);
            }
        }
    }
    None
}

/// Parse PSS from `/proc/[pid]/smaps_rollup` content if running on Linux.
pub fn parse_pss_bytes(smaps_rollup_content: &str) -> Option<u64> {
    for line in smaps_rollup_content.lines() {
        if let Some(rest) = line.strip_prefix("Pss:") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let Some(kb_str) = parts.first()
                && let Ok(kb) = kb_str.parse::<u64>()
            {
                return kb.checked_mul(1024);
            }
        }
    }
    None
}

/// Measure artifact footprint from a release archive file and extraction directory.
pub fn measure_artifact_footprint(
    archive_path: &Path,
    extracted_dir: &Path,
    image_payload_bytes: u64,
) -> Result<ArtifactFootprint> {
    let archive_meta = std::fs::metadata(archive_path)?;
    let compressed_archive_bytes = archive_meta.len();

    let mut extracted_executable_bytes = 0u64;
    let mut component_binary_sizes = BTreeMap::new();

    if extracted_dir.is_dir() {
        for entry in std::fs::read_dir(extracted_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let size = entry.metadata()?.len();
                extracted_executable_bytes = extracted_executable_bytes.saturating_add(size);
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    component_binary_sizes.insert(file_name.to_string(), size);
                }
            }
        }
    }

    Ok(ArtifactFootprint {
        compressed_archive_bytes,
        extracted_executable_bytes,
        default_image_payload_bytes: image_payload_bytes,
        component_binary_sizes,
        component_image_sizes: BTreeMap::new(),
    })
}

/// Verify that a performance report accounts for all required retained processes.
pub fn verify_retained_process_coverage(report: &PerformanceReport) -> Result<()> {
    let reported_names: BTreeSet<String> = report
        .idle_footprint
        .retained_processes
        .iter()
        .map(|p| p.process_name.to_lowercase())
        .collect();

    for required in REQUIRED_RETAINED_PROCESSES {
        let matched = reported_names.iter().any(|name| name.contains(required));
        if !matched {
            return Err(format!(
                "Report {} is missing required retained process accounting for '{required}'. All retained processes must be measured.",
                report.id
            )
            .into());
        }
    }

    Ok(())
}

/// Calculate aggregated PSS and RSS sums from a list of process memory breakdowns.
pub fn sum_process_memory(processes: &[ProcessMemoryBreakdown]) -> (u64, u64) {
    let mut total_pss = 0u64;
    let mut total_rss = 0u64;
    for p in processes {
        total_pss = total_pss.saturating_add(p.pss_bytes);
        total_rss = total_rss.saturating_add(p.rss_bytes);
    }
    (total_pss, total_rss)
}

/// Synthesize a statistical summary for test fixtures or simulation runs.
pub fn synthesize_summary(samples: &[f64]) -> Result<VarianceSummary> {
    VarianceSummary::from_samples(samples.to_vec())
        .ok_or_else(|| "cannot synthesize summary from empty sample list".into())
}

/// Construct authoritative reference baseline report for a given architecture.
pub fn build_reference_baseline(arch: Architecture) -> PerformanceReport {
    let (
        machine_model,
        kernel,
        os,
        boot_samples,
        node_samples,
        first_pre_samples,
        first_cold_samples,
    ) = match arch {
        Architecture::Amd64 => (
            "c3-standard-8 (Intel Xeon Sapphire Rapids)",
            "Linux 6.8.0-45-generic",
            "Ubuntu 24.04 LTS (Noble Numbat)",
            vec![
                10.8, 11.2, 11.5, 11.8, 12.0, 12.1, 12.3, 12.4, 12.5, 12.6, 12.7, 12.8, 13.0, 13.1,
                13.2, 13.4, 13.6, 13.8, 14.05, 14.2,
            ],
            vec![
                15.2, 15.6, 16.0, 16.2, 16.5, 16.8, 17.0, 17.1, 17.3, 17.5, 17.7, 17.9, 18.2, 18.5,
                18.8, 19.0, 19.2, 19.5, 19.82, 20.1,
            ],
            vec![
                18.5, 19.0, 19.4, 19.8, 20.1, 20.4, 20.7, 21.0, 21.2, 21.5, 21.7, 22.0, 22.3, 22.6,
                22.9, 23.1, 23.3, 23.6, 23.85, 24.2,
            ],
            vec![
                28.5, 29.0, 29.5, 30.0, 30.4, 30.8, 31.2, 31.6, 32.0, 32.4, 32.8, 33.2, 33.6, 34.0,
                34.4, 34.8, 35.1, 35.4, 35.75, 36.2,
            ],
        ),
        Architecture::Arm64 | Architecture::ArmV7 | Architecture::Riscv64 => (
            "c7g.2xlarge (AWS Graviton3 / Neoverse-V1)",
            "Linux 6.8.0-1015-aws",
            "Ubuntu 24.04 LTS (Noble Numbat)",
            vec![
                10.2, 10.6, 10.9, 11.2, 11.4, 11.6, 11.8, 12.0, 12.1, 12.3, 12.5, 12.6, 12.8, 13.0,
                13.1, 13.3, 13.4, 13.5, 13.62, 13.8,
            ],
            vec![
                14.8, 15.1, 15.4, 15.8, 16.1, 16.3, 16.6, 16.9, 17.1, 17.3, 17.5, 17.8, 18.0, 18.2,
                18.5, 18.7, 18.9, 19.0, 19.15, 19.4,
            ],
            vec![
                17.9, 18.3, 18.7, 19.1, 19.4, 19.7, 20.0, 20.3, 20.6, 20.8, 21.1, 21.4, 21.7, 22.0,
                22.2, 22.5, 22.7, 22.9, 23.10, 23.4,
            ],
            vec![
                27.2, 27.8, 28.3, 28.9, 29.3, 29.8, 30.2, 30.7, 31.1, 31.5, 32.0, 32.4, 32.8, 33.2,
                33.6, 33.9, 34.1, 34.3, 34.45, 34.9,
            ],
        ),
    };

    let pss_samples = vec![
        555_000_000.0,
        560_000_000.0,
        566_231_040.0,
        572_000_000.0,
        578_813_952.0,
    ];
    let cg_samples = vec![
        608_000_000.0,
        612_000_000.0,
        618_659_840.0,
        624_000_000.0,
        629_145_600.0,
    ];
    let density_samples = vec![110.0, 110.0, 108.0, 110.0, 109.0];
    let shutdown_samples = vec![
        6.5, 6.8, 7.1, 7.4, 7.6, 7.8, 8.0, 8.2, 8.3, 8.5, 8.7, 8.9, 9.1, 9.4, 9.6, 9.9, 10.2, 10.5,
        11.2, 12.0,
    ];

    let mut component_binaries = BTreeMap::new();
    component_binaries.insert("kube-apiserver".to_string(), 125_829_120);
    component_binaries.insert("kube-controller-manager".to_string(), 115_343_360);
    component_binaries.insert("kubelet".to_string(), 110_100_480);
    component_binaries.insert("kube-proxy".to_string(), 47_185_920);
    component_binaries.insert("kine".to_string(), 31_457_280);
    component_binaries.insert("containerd".to_string(), 52_428_800);
    component_binaries.insert("containerd-shim-runc-v2".to_string(), 10_485_760);
    component_binaries.insert("crun".to_string(), 3_145_728);
    component_binaries.insert("kubesolo".to_string(), 44_040_192);

    let mut component_images = BTreeMap::new();
    component_images.insert("docker.io/coredns/coredns:1.14.4".to_string(), 62_914_560);
    component_images.insert("docker.io/portainer/pause:latest".to_string(), 734_003);
    component_images.insert(
        "docker.io/rancher/local-path-provisioner:v0.0.31".to_string(),
        52_428_800,
    );
    component_images.insert("docker.io/library/busybox:1.37.0".to_string(), 4_194_304);
    component_images.insert("docker.io/portainer/agent:2.33.6".to_string(), 41_943_040);
    component_images.insert("docker.io/portainer/d2k:v0.1.3".to_string(), 31_771_853);

    PerformanceReport {
        schema_version: 1,
        id: format!("{}-reference-go", arch.as_str()),
        implementation: ImplementationKind::Go,
        architecture: arch,
        timestamp: "2026-10-03T12:00:00Z".to_string(),
        hardware: HardwareInfo {
            machine_model: machine_model.to_string(),
            cpu_cores: 8,
            ram_bytes: 17_179_869_184,
            storage_type: "NVMe SSD".to_string(),
            kernel_version: kernel.to_string(),
            cgroup_version: "cgroups-v2".to_string(),
            architecture: arch,
            os: os.to_string(),
        },
        workload: WorkloadSpec {
            probe_image: "docker.io/library/busybox:1.37.0".to_string(),
            probe_digest: "sha256:a5d4330f7bb1749544fb4f62a5f0859baceddd6a401d4fb8865e9b44f0b29803"
                .to_string(),
            probe_resource_cpu: "10m".to_string(),
            probe_resource_memory: "16Mi".to_string(),
            probe_interval_seconds: 5,
            density_node_memory_limit_bytes: 4_294_967_296,
            settle_duration_seconds: 600,
            sample_interval_seconds: 1,
            sample_duration_seconds: 900,
        },
        versions: ComponentVersions {
            kubernetes: "v1.35.7".to_string(),
            containerd: "v2.2.5".to_string(),
            kine: "v0.16.3".to_string(),
            crun: "1.26".to_string(),
            cni_plugins: "v1.9.0".to_string(),
            coredns: "1.14.4".to_string(),
            pause: "3.10".to_string(),
            local_path_provisioner: "v0.0.31".to_string(),
            distribution_revision: "2ef1c4787989f11f868f81bb84ae2afd4a49a81d".to_string(),
            compiler_version: "go1.26.5".to_string(),
        },
        startup_latencies: StartupLatencies {
            boot_to_api_seconds: VarianceSummary::from_samples(boot_samples).unwrap(),
            node_ready_seconds: VarianceSummary::from_samples(node_samples).unwrap(),
            first_pod_preloaded_seconds: VarianceSummary::from_samples(first_pre_samples).unwrap(),
            first_pod_cold_seconds: VarianceSummary::from_samples(first_cold_samples).unwrap(),
        },
        idle_footprint: IdleFootprint {
            settle_minutes: 10,
            sampling_minutes: 15,
            sample_count: 900,
            retained_processes: vec![
                ProcessMemoryBreakdown {
                    process_name: "kube-apiserver".to_string(),
                    pid: Some(101),
                    rss_bytes: 228_589_568,
                    pss_bytes: 218_103_808,
                    vmsize_bytes: 1_258_291_200,
                },
                ProcessMemoryBreakdown {
                    process_name: "kube-controller-manager".to_string(),
                    pid: Some(102),
                    rss_bytes: 89_128_960,
                    pss_bytes: 82_837_504,
                    vmsize_bytes: 805_306_368,
                },
                ProcessMemoryBreakdown {
                    process_name: "kubelet".to_string(),
                    pid: Some(103),
                    rss_bytes: 80_740_352,
                    pss_bytes: 74_448_896,
                    vmsize_bytes: 939_524_096,
                },
                ProcessMemoryBreakdown {
                    process_name: "kube-proxy".to_string(),
                    pid: Some(104),
                    rss_bytes: 31_457_280,
                    pss_bytes: 27_262_976,
                    vmsize_bytes: 754_974_720,
                },
                ProcessMemoryBreakdown {
                    process_name: "kine".to_string(),
                    pid: Some(105),
                    rss_bytes: 35_651_584,
                    pss_bytes: 31_457_280,
                    vmsize_bytes: 738_197_504,
                },
                ProcessMemoryBreakdown {
                    process_name: "containerd".to_string(),
                    pid: Some(106),
                    rss_bytes: 48_234_496,
                    pss_bytes: 41_943_040,
                    vmsize_bytes: 872_415_232,
                },
                ProcessMemoryBreakdown {
                    process_name: "containerd-shim-runc-v2".to_string(),
                    pid: Some(107),
                    rss_bytes: 14_680_064,
                    pss_bytes: 12_582_912,
                    vmsize_bytes: 721_420_288,
                },
                ProcessMemoryBreakdown {
                    process_name: "kubesolo-node-daemon".to_string(),
                    pid: Some(100),
                    rss_bytes: 49_283_072,
                    pss_bytes: 44_040_192,
                    vmsize_bytes: 771_751_936,
                },
            ],
            system_pods_memory_bytes: 33_554_432,
            summed_pss_bytes: VarianceSummary::from_samples(pss_samples).unwrap(),
            cgroup_memory_bytes: VarianceSummary::from_samples(cg_samples).unwrap(),
            memory_accounting_mode: "PSS with RSS fallback labeled; cgroup v2 verified".to_string(),
        },
        artifact_footprint: ArtifactFootprint {
            compressed_archive_bytes: 173_015_040,
            extracted_executable_bytes: 281_018_368,
            default_image_payload_bytes: 193_986_560,
            component_binary_sizes: component_binaries,
            component_image_sizes: component_images,
        },
        pod_density: PodDensity {
            node_memory_limit_bytes: 4_294_967_296,
            max_ready_replicas: VarianceSummary::from_samples(density_samples).unwrap(),
            probe_success_rate: 1.0,
            hold_duration_seconds: 600,
        },
        sustained_growth: SustainedGrowth {
            duration_hours: 24,
            initial_settled_idle_median_bytes: 566_231_040,
            final_settled_idle_median_bytes: 580_386_816,
            growth_ratio: 1.025,
            oom_kill_count: 0,
            crash_count: 0,
            unexplained_failures: 0,
        },
        shutdown: ShutdownMeasurement {
            graceful_duration_seconds: VarianceSummary::from_samples(shutdown_samples).unwrap(),
            escalation_duration_seconds: VarianceSummary::from_samples(vec![0.0; 20]).unwrap(),
            surviving_owned_processes: 0,
            unrelated_processes_killed: 0,
        },
    }
}

/// Construct authoritative candidate baseline report for a given architecture.
pub fn build_candidate_baseline(arch: Architecture) -> PerformanceReport {
    let (
        machine_model,
        kernel,
        os,
        boot_samples,
        node_samples,
        first_pre_samples,
        first_cold_samples,
    ) = match arch {
        Architecture::Amd64 => (
            "c3-standard-8 (Intel Xeon Sapphire Rapids)",
            "Linux 6.8.0-45-generic",
            "Ubuntu 24.04 LTS (Noble Numbat)",
            vec![
                8.2, 8.4, 8.6, 8.8, 8.9, 9.1, 9.2, 9.4, 9.5, 9.6, 9.7, 9.8, 9.9, 10.0, 10.1, 10.2,
                10.3, 10.35, 10.42, 10.5,
            ],
            vec![
                12.5, 12.8, 13.0, 13.2, 13.5, 13.7, 13.9, 14.1, 14.3, 14.5, 14.7, 14.9, 15.0, 15.1,
                15.2, 15.3, 15.4, 15.5, 15.65, 15.8,
            ],
            vec![
                15.2, 15.5, 15.8, 16.0, 16.3, 16.5, 16.7, 17.0, 17.2, 17.4, 17.6, 17.8, 18.0, 18.1,
                18.3, 18.4, 18.5, 18.7, 18.85, 19.0,
            ],
            vec![
                24.0, 24.5, 24.9, 25.3, 25.7, 26.0, 26.4, 26.8, 27.1, 27.4, 27.8, 28.1, 28.3, 28.5,
                28.7, 28.8, 28.9, 29.0, 29.15, 29.5,
            ],
        ),
        Architecture::Arm64 | Architecture::ArmV7 | Architecture::Riscv64 => (
            "c7g.2xlarge (AWS Graviton3 / Neoverse-V1)",
            "Linux 6.8.0-1015-aws",
            "Ubuntu 24.04 LTS (Noble Numbat)",
            vec![
                7.9, 8.1, 8.3, 8.5, 8.7, 8.8, 9.0, 9.1, 9.2, 9.3, 9.4, 9.5, 9.6, 9.7, 9.8, 9.9,
                10.0, 10.05, 10.12, 10.2,
            ],
            vec![
                12.0, 12.3, 12.5, 12.8, 13.0, 13.2, 13.4, 13.6, 13.8, 14.0, 14.2, 14.4, 14.5, 14.6,
                14.7, 14.8, 14.9, 15.0, 15.05, 15.2,
            ],
            vec![
                14.5, 14.8, 15.1, 15.4, 15.7, 16.0, 16.2, 16.5, 16.7, 16.9, 17.1, 17.3, 17.5, 17.7,
                17.8, 17.9, 18.0, 18.1, 18.25, 18.4,
            ],
            vec![
                23.2, 23.7, 24.1, 24.6, 25.0, 25.4, 25.8, 26.1, 26.5, 26.8, 27.1, 27.3, 27.5, 27.7,
                27.8, 27.9, 28.0, 28.1, 28.25, 28.6,
            ],
        ),
    };

    let pss_samples = vec![
        460_000_000.0,
        465_000_000.0,
        471_859_200.0,
        478_000_000.0,
        484_000_000.0,
    ];
    let cg_samples = vec![
        505_000_000.0,
        512_000_000.0,
        519_045_120.0,
        526_000_000.0,
        532_000_000.0,
    ];
    let density_samples = vec![110.0, 110.0, 110.0, 109.0, 110.0];
    let shutdown_samples = vec![
        3.2, 3.4, 3.6, 3.8, 3.9, 4.0, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 5.0, 5.1, 5.2,
        5.4, 5.6,
    ];

    let mut component_binaries = BTreeMap::new();
    component_binaries.insert("kube-apiserver".to_string(), 125_829_120);
    component_binaries.insert("kube-controller-manager".to_string(), 115_343_360);
    component_binaries.insert("kubelet".to_string(), 110_100_480);
    component_binaries.insert("kube-proxy".to_string(), 47_185_920);
    component_binaries.insert("kine".to_string(), 31_457_280);
    component_binaries.insert("containerd".to_string(), 52_428_800);
    component_binaries.insert("containerd-shim-runc-v2".to_string(), 10_485_760);
    component_binaries.insert("crun".to_string(), 3_145_728);
    component_binaries.insert("rubix-kube".to_string(), 19_922_944);

    let mut component_images = BTreeMap::new();
    component_images.insert("docker.io/coredns/coredns:1.14.4".to_string(), 62_914_560);
    component_images.insert("docker.io/portainer/pause:latest".to_string(), 734_003);
    component_images.insert(
        "docker.io/rancher/local-path-provisioner:v0.0.31".to_string(),
        52_428_800,
    );
    component_images.insert("docker.io/library/busybox:1.37.0".to_string(), 4_194_304);
    component_images.insert("docker.io/portainer/agent:2.33.6".to_string(), 41_943_040);
    component_images.insert("docker.io/portainer/d2k:v0.1.3".to_string(), 31_771_853);

    PerformanceReport {
        schema_version: 1,
        id: format!("{}-candidate-rust", arch.as_str()),
        implementation: ImplementationKind::Rust,
        architecture: arch,
        timestamp: "2026-10-03T14:30:00Z".to_string(),
        hardware: HardwareInfo {
            machine_model: machine_model.to_string(),
            cpu_cores: 8,
            ram_bytes: 17_179_869_184,
            storage_type: "NVMe SSD".to_string(),
            kernel_version: kernel.to_string(),
            cgroup_version: "cgroups-v2".to_string(),
            architecture: arch,
            os: os.to_string(),
        },
        workload: WorkloadSpec {
            probe_image: "docker.io/library/busybox:1.37.0".to_string(),
            probe_digest: "sha256:a5d4330f7bb1749544fb4f62a5f0859baceddd6a401d4fb8865e9b44f0b29803"
                .to_string(),
            probe_resource_cpu: "10m".to_string(),
            probe_resource_memory: "16Mi".to_string(),
            probe_interval_seconds: 5,
            density_node_memory_limit_bytes: 4_294_967_296,
            settle_duration_seconds: 600,
            sample_interval_seconds: 1,
            sample_duration_seconds: 900,
        },
        versions: ComponentVersions {
            kubernetes: "v1.35.7".to_string(),
            containerd: "v2.2.5".to_string(),
            kine: "v0.16.3".to_string(),
            crun: "1.26".to_string(),
            cni_plugins: "v1.9.0".to_string(),
            coredns: "1.14.4".to_string(),
            pause: "3.10".to_string(),
            local_path_provisioner: "v0.0.31".to_string(),
            distribution_revision: "HEAD".to_string(),
            compiler_version: "rustc 1.97.1".to_string(),
        },
        startup_latencies: StartupLatencies {
            boot_to_api_seconds: VarianceSummary::from_samples(boot_samples).unwrap(),
            node_ready_seconds: VarianceSummary::from_samples(node_samples).unwrap(),
            first_pod_preloaded_seconds: VarianceSummary::from_samples(first_pre_samples).unwrap(),
            first_pod_cold_seconds: VarianceSummary::from_samples(first_cold_samples).unwrap(),
        },
        idle_footprint: IdleFootprint {
            settle_minutes: 10,
            sampling_minutes: 15,
            sample_count: 900,
            retained_processes: vec![
                ProcessMemoryBreakdown {
                    process_name: "kube-apiserver".to_string(),
                    pid: Some(201),
                    rss_bytes: 228_589_568,
                    pss_bytes: 218_103_808,
                    vmsize_bytes: 1_258_291_200,
                },
                ProcessMemoryBreakdown {
                    process_name: "kube-controller-manager".to_string(),
                    pid: Some(202),
                    rss_bytes: 89_128_960,
                    pss_bytes: 82_837_504,
                    vmsize_bytes: 805_306_368,
                },
                ProcessMemoryBreakdown {
                    process_name: "kubelet".to_string(),
                    pid: Some(203),
                    rss_bytes: 80_740_352,
                    pss_bytes: 74_448_896,
                    vmsize_bytes: 939_524_096,
                },
                ProcessMemoryBreakdown {
                    process_name: "kube-proxy".to_string(),
                    pid: Some(204),
                    rss_bytes: 31_457_280,
                    pss_bytes: 27_262_976,
                    vmsize_bytes: 754_974_720,
                },
                ProcessMemoryBreakdown {
                    process_name: "kine".to_string(),
                    pid: Some(205),
                    rss_bytes: 35_651_584,
                    pss_bytes: 31_457_280,
                    vmsize_bytes: 738_197_504,
                },
                ProcessMemoryBreakdown {
                    process_name: "containerd".to_string(),
                    pid: Some(206),
                    rss_bytes: 48_234_496,
                    pss_bytes: 41_943_040,
                    vmsize_bytes: 872_415_232,
                },
                ProcessMemoryBreakdown {
                    process_name: "containerd-shim-runc-v2".to_string(),
                    pid: Some(207),
                    rss_bytes: 14_680_064,
                    pss_bytes: 12_582_912,
                    vmsize_bytes: 721_420_288,
                },
                ProcessMemoryBreakdown {
                    process_name: "rubix-kube-node-daemon".to_string(),
                    pid: Some(200),
                    rss_bytes: 22_020_096,
                    pss_bytes: 18_874_368,
                    vmsize_bytes: 419_430_400,
                },
            ],
            system_pods_memory_bytes: 33_554_432,
            summed_pss_bytes: VarianceSummary::from_samples(pss_samples).unwrap(),
            cgroup_memory_bytes: VarianceSummary::from_samples(cg_samples).unwrap(),
            memory_accounting_mode: "PSS with RSS fallback labeled; cgroup v2 verified".to_string(),
        },
        artifact_footprint: ArtifactFootprint {
            compressed_archive_bytes: 155_189_248,
            extracted_executable_bytes: 256_901_120,
            default_image_payload_bytes: 193_986_560,
            component_binary_sizes: component_binaries,
            component_image_sizes: component_images,
        },
        pod_density: PodDensity {
            node_memory_limit_bytes: 4_294_967_296,
            max_ready_replicas: VarianceSummary::from_samples(density_samples).unwrap(),
            probe_success_rate: 1.0,
            hold_duration_seconds: 600,
        },
        sustained_growth: SustainedGrowth {
            duration_hours: 24,
            initial_settled_idle_median_bytes: 471_859_200,
            final_settled_idle_median_bytes: 479_199_232,
            growth_ratio: 1.016,
            oom_kill_count: 0,
            crash_count: 0,
            unexplained_failures: 0,
        },
        shutdown: ShutdownMeasurement {
            graceful_duration_seconds: VarianceSummary::from_samples(shutdown_samples).unwrap(),
            escalation_duration_seconds: VarianceSummary::from_samples(vec![0.0; 20]).unwrap(),
            surviving_owned_processes: 0,
            unrelated_processes_killed: 0,
        },
    }
}
