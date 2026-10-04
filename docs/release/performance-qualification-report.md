# Performance Baseline Comparison: AMD64 (Go) vs AMD64 (Rust)

**Generated:** 2026-10-03T14:30:00Z | **Schema Version:** 1

## 1. Contract Gates Evaluation Summary

**Overall Gate Status:** NOT QUALIFIED (Invalid, synthetic or unverified evidence)
- Not qualified: reference SyntheticFixture, candidate SyntheticFixture; verified live-capture provenance/import is not implemented

| Gate Metric | Direction | Reference (go) | Candidate (rust) | Threshold Limit | Status |
|---|---|---|---|---|---|
| Boot-to-API Latency (p95) | <= 1.10x | 14.050 | 10.420 | 15.455 | PASS |
| Node Ready Latency (p95) | <= 1.10x | 19.820 | 15.650 | 21.802 | PASS |
| First Pod Latency (Preloaded, p95) | <= 1.10x | 23.850 | 18.850 | 26.235 | PASS |
| First Pod Latency (Cold Image, p95) | <= 1.10x | 35.750 | 29.150 | 39.325 | PASS |
| Idle Footprint (Summed PSS, Median) | <= 1.10x | 566231040.000 | 471859200.000 | 622854144.000 | PASS |
| Idle Footprint (Cgroup Memory, Median) | <= 1.10x | 618659840.000 | 519045120.000 | 680525824.000 | PASS |
| Distribution Size (Compressed Archive) | <= 1.10x | 173015040.000 | 155189248.000 | 190316544.000 | PASS |
| Distribution Size (Extracted Executables) | <= 1.10x | 281018368.000 | 256901120.000 | 309120204.800 | PASS |
| Distribution Size (Default Image Payload) | <= 1.10x | 193986560.000 | 193986560.000 | 213385216.000 | PASS |
| Pod Density Capacity (Median Replicas) | >= 0.90x | 110.000 | 110.000 | 99.000 | PASS |
| Sustained Growth (24h Soak) | <= 1.10x | 471859200.000 | 479199232.000 | 519045120.000 | PASS |
| Shutdown & Process Cleanup | <= 1.10x | 30.000 | 5.400 | 30.000 | PASS |

## 2. Declared Hardware & Workload Specification

- **Machine Model:** c3-standard-8 (Intel Xeon Sapphire Rapids) (amd64)
- **CPU Cores / RAM:** 8 cores, 16.00 GiB RAM
- **Kernel & Cgroups:** Linux 6.8.0-45-generic, cgroups-v2
- **Workload:** Probe image `docker.io/library/busybox:1.37.0`, Memory Limit: 4.0 GiB

## 3. Startup Latencies (20 Fresh Boots)

| Latency Phase | Go Reference p95 | Go Mean | Rust Candidate p95 | Rust Mean | p95 Ratio |
|---|---|---|---|---|---|
| Boot-to-API | 14.050s | 12.652s | 10.420s | 9.549s | 0.74x |
| Node Ready | 19.820s | 17.696s | 15.650s | 14.402s | 0.79x |
| First Pod (Preloaded) | 23.850s | 21.558s | 18.850s | 17.343s | 0.79x |
| First Pod (Cold Image) | 35.750s | 32.532s | 29.150s | 27.242s | 0.82x |

## 4. Whole-Distribution Idle Footprint & Retained Processes

**Settling Period:** 10 minutes | **Sampling:** 1 sample/sec for 15 minutes (900 samples)

| Retained Process | Reference PSS | Reference RSS | Candidate PSS | Candidate RSS |
|---|---|---|---|---|
| kube-apiserver | 208.00 MiB | 218.00 MiB | 208.00 MiB | 218.00 MiB |
| kube-controller-manager | 79.00 MiB | 85.00 MiB | 79.00 MiB | 85.00 MiB |
| kubelet | 71.00 MiB | 77.00 MiB | 71.00 MiB | 77.00 MiB |
| kube-proxy | 26.00 MiB | 30.00 MiB | 26.00 MiB | 30.00 MiB |
| kine | 30.00 MiB | 34.00 MiB | 30.00 MiB | 34.00 MiB |
| containerd | 40.00 MiB | 46.00 MiB | 40.00 MiB | 46.00 MiB |
| containerd-shim-runc-v2 | 12.00 MiB | 14.00 MiB | 12.00 MiB | 14.00 MiB |
| distribution-node-daemon | 42.00 MiB | 47.00 MiB | 18.00 MiB | 21.00 MiB |

- **Summed Whole-Distribution PSS Median:** Reference 540.00 MiB vs Candidate 450.00 MiB (0.83x)
- **Cgroup Memory Median:** Reference 590.00 MiB vs Candidate 495.00 MiB (0.84x)

## 5. Artifact Footprint

| Artifact Item | Reference | Candidate | Ratio |
|---|---|---|---|
| Compressed Release Archive | 165.00 MiB | 148.00 MiB | 0.90x |
| Extracted Executable Payload | 268.00 MiB | 245.00 MiB | 0.91x |
| Default Image Payload | 185.00 MiB | 185.00 MiB | 1.00x |

## 6. Pod Density & Soak Stability

- **Pod Density (Median Replicas):** Reference 110.0 pods vs Candidate 110.0 pods (Ratio: 1.00x >= 0.90x)
- **Sustained Growth (Declared 24h Soak):** Initial settled 450.00 MiB -> Final settled 457.00 MiB (1.016x derived growth, 0 OOMs, 0 crashes, 0 unexplained failures)
- **Shutdown:** Graceful p95 5.40s (<= 30s deadline), Surviving owned processes: 0

