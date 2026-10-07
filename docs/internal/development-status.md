
Work item: E06.01 (https://github.com/geoffsee/rubix-kube/issues/48); Parent criteria 1, 3
Outcome: Blocked
Readiness: Blocked
Ownership: Unassigned
Stack: payload/asset-inventory
Evidence: Linux ARM64/glibc manifests validate structurally. Kubernetes v1.35.7 arm64 kube-apiserver, kube-controller-manager, kubelet, and kube-proxy hashes and sizes match dl.k8s.io. Image, CNI, containerd, crun, and shim rows use placeholder digests until encoded-byte pins exist, so the manifests are not an accepted payload cell.
Next step: Pin encoded bytes for images, CNI, containerd, crun, and the shim; timestamp: 2026-10-02T23:50:00Z

Work item: E27.01 (https://github.com/geoffsee/rubix-kube/issues/114); Parent criteria 1, 2, 3
Outcome: In progress
Readiness: Not done
Ownership: Unassigned
Stack: build/node-variants
Evidence: Naming accepts only arch[-musl][-offline], rejects non-canonical aliases, and rejects optional images that a cell does not bundle. Packaged digest comparison is available. Clean-checkout builds of the 16 archive cells have not been produced.
Next step: Build and compare the 16 cells from a clean checkout; timestamp: 2026-10-02T23:50:00Z

Work item: E07.02 (https://github.com/geoffsee/rubix-kube/issues/52)
Outcome: Fixture implementation
Readiness: Not qualified
Ownership: Unassigned
Stack: None
Evidence: All `cargo test --workspace --locked` and `cargo test -p rubix-dev --lib` pass cleanly. `rubix-pki` is integrated with `rcgen 0.14` and all parity test evidence (source inventories, qualifications, receipts, JSON tree) hashes have been cascaded correctly.
Next step: None; timestamp: 2026-09-29T16:35:00Z

Work item: E06.02 (https://github.com/geoffsee/rubix-kube/issues/49); Parent criteria 1, 3
Outcome: Completed
Readiness: Done
Ownership: Unassigned
Stack: materialization/verify-core
Evidence: Implemented safe, atomic, idempotent asset materializer in rubix-assets with Unix executable (0o755) and image (0o644) permissions, path-safety bounds against destination escaping, staging directory isolation, disk reverification before atomic commit, and tests covering ARM64 cell payload verification, idempotency, corrupt archive rejection, and read-only destination handling.
Next step: None; timestamp: 2026-10-02T23:59:00Z

Work item: E06.03 (https://github.com/geoffsee/rubix-kube/issues/50); Parent criteria 1, 3
Outcome: Completed
Readiness: Done
Ownership: Unassigned
Stack: runtime/selection
Evidence: Implemented `AssetSelector` and `SelectedDelivery` in `rubix-assets` resolving asset delivery across online/offline variants, external-dependency scopes (zero embedded payloads, `HostSupplied` executables), local storage toggle (skipping provisioner and helper images), target architecture feature constraints, and airgapped/egress-denied validation ensuring custom Portainer images remain explicit registry pulls and offline fixtures bundle supported images without network egress.
Next step: None; timestamp: 2026-10-02T20:20:00Z

Work item: E27.02 (https://github.com/geoffsee/rubix-kube/issues/115); Parent criteria 1, 2, 3
Outcome: In progress
Readiness: Not qualified
Ownership: Unassigned
Stack: packages/release-artifacts
Evidence: Descriptor validation and synthetic dependency materialization cover the required matrix. No real node archives, target executables, images or installer executions were produced; these checks do not qualify a release.
Next step: Assemble real artifacts, verify bytes and machine targets, and capture disposable installation evidence for every required cell; timestamp: 2026-10-03T07:17:00Z

Work item: E28.02 (https://github.com/geoffsee/rubix-kube/issues/119); Parent criteria 3
Outcome: In progress
Readiness: Fixture coverage; C13 not qualified
Ownership: Unassigned
Stack: qual/recovery-lifecycle
Evidence: Native/in-process recovery fixtures include an actually killed owned Rust runtime with exact acknowledged-object recovery, and a TERM-ignoring owned Rust child with observed bounded KILL and reaping. Mock adapters separately exercise failure policy. The historical regression test is an inventory check, not verification evidence. These tests do not qualify retained kube-apiserver/Kine, Linux reboot/power loss, real optional services, host mounts or production lifecycle interruption.
Next step: Capture current-source disposable Linux evidence for the selected production boundary and every unresolved historical recovery gate; timestamp: 2026-10-03T19:30:00Z

Work item: E28.03 (https://github.com/geoffsee/rubix-kube/issues/120); Parent criteria 1, 2, 3
Outcome: In progress / Not qualified
Readiness: Synthetic platform and soak fixture consistency; C14 execution pending
Ownership: Platform Matrix & Soak Engineer
Stack: qual/platform-soak
Evidence: Synthetic plan and arithmetic fixtures cover the matrix and record inventories. Candidate verification requires independently read files and an independent expected version; fixture reports contain no candidate observations. No retained-node platform execution, 24-hour soak, restart/state preservation or historical/per-epic execution is qualified. Qualification run/verify fail closed.
Next step: Implement and capture authenticated current-source disposable Linux execution receipts for every C14 requirement.

Work item: E30.01 (https://github.com/geoffsee/rubix-kube/issues/124)
Outcome: In progress
Readiness: Not qualified
Ownership: Unassigned
Stack: migration/state-transitions
Evidence: Isolated state-preservation fixtures classify historical versions, exercise configuration conversion, verify client signatures and required signing keys, reconcile unique workload identities, and compare PV files, directories, symlinks, ownership and permissions. Native snapshot conversion is an experiment on in-memory records; it neither adopts a production Kine database nor checkpoints its WAL. No live Linux cutover or downtime measurement was performed.
Next step: Rehearse consistent Kine SQLite backup, adoption and rollback with the retained executables and dedicated loopback mTLS, preserving identities and PV access at exact revisions before qualifying C13/E30.

Work item: E29.02 (https://github.com/geoffsee/rubix-kube/issues/122); Parent criteria 1, 2
Outcome: In progress
Readiness: Profiled fixture baselines; C14 not qualified
Ownership: Unassigned
Stack: perf/budget-regressions
Evidence: Profiling compares candidate measurements to E01 budgets across startup (10.42s boot-to-API vs 14.05s ref p95), idle memory (450.0 MiB PSS vs 540.0 MiB ref, with kube-apiserver consuming 208.0 MiB), and distribution size (148.0 MiB archive vs 165.0 MiB ref). Synthetic fixture differences illustrate daemon memory and binary comparisons; no optimization implementation, causal attribution, protocol parity or live reduction is qualified. Explicit budget decisions document refusal of unmeasured sub-200MB and under-60s claims. Verified live-capture importer remains unimplemented, failing closed.
Next step: Ingest verified paired Linux hardware captures for amd64 and arm64 to qualify E29; timestamp: 2026-10-03T23:00:00Z

Work item: E30.02 (https://github.com/geoffsee/rubix-kube/issues/125); Parent criteria 2, 3
Outcome: Fixture implementation
Readiness: Not qualified
Ownership: Unassigned
Stack: migration/recovery-rehearsal
Evidence: Synthetic filesystem fixtures cover historical starting versions and 11 lifecycle stages. Pre-receipt stages drive real run_upgrade failure handling with mocked services; later stages reconstruct interrupted disk states. Regressions compare original configuration, identity and opaque datastore bytes, complete manifests and PV inventories, and retained evidence on backup refusal. Fresh container backends exercise rollback and failed-commit retry against a stateful engine double. Snapshot checksums detect changed backups but do not establish original SQLite health or live application readiness. No production Kine transition, abrupt real-container interruption or live Linux migration was performed.
Next step: Rehearse real interrupted Kine-backed transitions with retained executables, exact inputs and mTLS, measuring readiness and verifying backup source health before qualifying C14.

Work item: E29.03 (https://github.com/geoffsee/rubix-kube/issues/123); Parent criteria Gate C14
Outcome: Completed
Readiness: Done
Ownership: Unassigned
Stack: perf/ci-regression-gates
Evidence: Implemented synthetic fixture integrity and contract arithmetic gating (not live performance qualification), with rebaseline policy enforcement in `rubix-perf gate-ci`, validating all 12 contract thresholds across amd64 and arm64, higher-is-better pod density directionality, 24-hour sustained memory growth bounds (<= 1.10 ratio with 0 OOMs, crashes, or failures) covering all 8 canonical retained process roles, strict rejection of unmeasured sub-200MB claims and invalid PSS/RSS/latency samples, provenance integrity verification, secondary target gap checks, and contract multiplier immutability.
Next step: Add verified matched live Linux captures before qualifying performance; timestamp: 2026-10-03T18:45:00Z

Work item: E30.03 (https://github.com/geoffsee/rubix-kube/issues/126)
Outcome: In progress
Readiness: Operator references; production handoff not qualified
Ownership: Operator Runbooks & Handoff Engineer
Stack: release/operator-runbooks
Evidence: Documents actual management command syntax, verified host bundle staging, Docker image import, external runtime ownership, CNI/storage identities, metrics/CPU configuration, receipt recovery and explicit evidence limitations. No universal automated host installer, production Go-to-Rust cutover, measured downtime or 24-hour workload qualification is established by these documents or their parser/schema tests.
Next step: Capture current-source disposable Linux installation, egress-denied workloads, consistent Kine backup/restore and retained-node operator handoff receipts before qualifying C16/C17.

Work item: E30.03 (#126); repository metadata audit and release qualification boundary
Outcome: Repository digest declaration, attribution row and local-link audits implemented
Readiness: Pending qualification
Ownership: Unassigned
Stack: release/acceptance-ledger
Evidence: `rubix-qualification --metadata-only` audits declared repository metadata without fetching inputs or observing candidate artifacts. Normal `rubix-qualification` and `run_release_qualification` fail closed because validated current candidate-bound completion receipt verification is unavailable. All 11 Roadmap #263 completion criteria remain pending; synthetic fixtures and document presence do not qualify live behavior or a release.
Next step: Implement trusted candidate-bound receipt readers and obtain the missing live qualification, operator rehearsal and publication evidence; timestamp: 2026-10-04T15:00:00Z

Work item: E31.01 (https://github.com/geoffsee/rubix-kube/issues/335); Parent criteria 1, 2
Outcome: Completed
Readiness: Done
Ownership: Unassigned
Stack: docs/adr-amendment
Evidence: ADR amendment recorded in experiments/component-boundary/ADR.md selecting Option B (in-process Rust control plane matching NodeRuntime with rubix-datastore, rubix-apiserver, rubix-controller and pending rubix-kubelet/rubix-proxy adapters, retaining managed containerd and runtime shims). Compatibility contract, acceptance matrix, and AGENTS.md updated accordingly, merged via PR #361.
Next step: None; timestamp: 2026-10-07T13:45:00Z

Work item: E31.02 (https://github.com/geoffsee/rubix-kube/issues/336); Parent criteria 3, 4
Outcome: In progress
Readiness: Done
Ownership: Unassigned
Stack: docs/restore-ledger
Evidence: docs/internal/development-status.md restored on main and extended with 18 ledger rows covering E31–E37 child issues. Roadmap #263 disposition written and closed with delivered vs pending gates. PR #333 closed with podman-kubelet experiment evidence preserved under experiments/podman-kubelet. Link integrity tests pass.
Next step: None; timestamp: 2026-10-07T14:15:00Z

Work item: E32.01 (https://github.com/geoffsee/rubix-kube/issues/338); Parent criteria 1
Outcome: Completed
Readiness: Complete
Ownership: Unassigned
Stack: payload/pin-digests
Evidence: Pinned deterministic encoded-byte sizes and SHA-256 digests adhering to CATALOG.md G11 encoding rules for all non-Kubernetes rows across online-amd64.json, online-arm64.json, and offline-arm64.json. Pinned official Kubernetes v1.35.7 and Kine v0.16.3 amd64 rows in online-amd64.json. Replaced all synthetic/placeholder digests in asset test fixtures; rubix-assets tests, formatting, lints, and language policy pass cleanly.
Next step: None; timestamp: 2026-10-07T17:35:00Z

Work item: E32.02 (https://github.com/geoffsee/rubix-kube/issues/339); Parent criteria 2, 3
Outcome: Pending
Readiness: Ready to start
Ownership: Unassigned
Stack: build/node-cells
Evidence: 16 node archive cells and 4 management targets have not yet been built from clean checkout; disposable-host install smoke of arm64/glibc cell pending.
Next step: Produce clean-checkout builds of all 16 cells and 4 management targets with build receipts and run disposable-host smoke install of arm64/glibc cell; timestamp: 2026-10-07T14:15:00Z

Work item: E33.01 (https://github.com/geoffsee/rubix-kube/issues/341); Parent criteria 1, 2
Outcome: Pending
Readiness: Ready to start
Ownership: Unassigned
Stack: runtime/datastore-api-boundary
Evidence: NodeRuntime in-process datastore and API components selected by Option B require startup on disposable Linux arm64 host with loopback mTLS and credential authentication proofs.
Next step: Start rubix-datastore and rubix-apiserver in NodeRuntime on disposable Linux arm64 host, verify dedicated datastore CA mTLS positive/negative cases, and produce receipt; timestamp: 2026-10-07T14:15:00Z

Work item: E33.02 (https://github.com/geoffsee/rubix-kube/issues/342); Parent criteria 1, 2
Outcome: Pending
Readiness: Blocked on E33.01, E32.01
Ownership: Unassigned
Stack: runtime/linux-workloads
Evidence: Workload path (managed containerd, CNI bridge, kubelet, kube-proxy, CoreDNS) not yet brought up on disposable Linux host; pod execution, DNS resolution, and reverse-order shutdown pending.
Next step: Bring up containerd, CNI, kubelet, proxy, and CoreDNS on Linux arm64 node, verify pod lifecycle and DNS resolution, and record cleanup inventory in receipt; timestamp: 2026-10-07T14:15:00Z

Work item: E33.03 (https://github.com/geoffsee/rubix-kube/issues/343); Parent criteria 2, 3
Outcome: Pending
Readiness: Ready to start
Ownership: Unassigned
Stack: ci/disposable-linux-job
Evidence: Disposable Linux node workflow job in .github/workflows/integration.yml on ubuntu-24.04-arm runner not yet added; receipt generation helper pending.
Next step: Add integration.yml workflow job on ubuntu-24.04-arm runner executing node lifecycle and producing schema-2 integration receipts; timestamp: 2026-10-07T14:15:00Z

Work item: E34.01 (https://github.com/geoffsee/rubix-kube/issues/345); Parent criteria 1, 3
Outcome: Pending
Readiness: Ready to start
Ownership: Unassigned
Stack: kubelet/cri-provider
Evidence: RuntimeProvider implementation over rubix_cri::CriClient for containerd not yet implemented in crates/rubix-kubelet; socket-gated reconciler suite and log parsing pending.
Next step: Implement CriRuntimeProvider in crates/rubix-kubelet forwarding to CriClient over Unix socket, with CRI log parsing and Linux socket-gated tests; timestamp: 2026-10-07T14:15:00Z

Work item: E34.02 (https://github.com/geoffsee/rubix-kube/issues/346); Parent criteria 2, 3
Outcome: Pending
Readiness: Ready to start
Ownership: Unassigned
Stack: kubelet/workload-gaps
Evidence: Workload gaps recorded in experiments/podman-kubelet/README.md (volume mounts, container ports, resource limits, init containers, probes, preStop, logs -f, Table responses, default namespaces) remain open in rubix-kubelet and rubix-apiserver.
Next step: Implement volume mounts, init containers, probes, Table responses, and default namespace bootstrap, recording any deferred items in compatibility contract; timestamp: 2026-10-07T14:15:00Z

Work item: E35.01 (https://github.com/geoffsee/rubix-kube/issues/348); Parent criteria 1, 2
Outcome: Pending
Readiness: Ready to start
Ownership: Unassigned
Stack: qual/receipt-schema-reader
Evidence: Candidate-bound receipt schema and trusted reader in tools/dev/src/release_qualification not yet implemented; check_criterion_* functions currently return pending stubs.
Next step: Implement versioned receipt schema and trusted reader validating candidate digests, binding receipts to the 11 completion criteria; timestamp: 2026-10-07T14:15:00Z

Work item: E35.02 (https://github.com/geoffsee/rubix-kube/issues/349); Parent criteria 2, 3
Outcome: Pending
Readiness: Blocked on E35.01
Ownership: Unassigned
Stack: release/receipt-report-binding
Evidence: rubix-release assemble and verify currently consume diagnostic fixtures rather than validated receipts; docs/release reports carry UNQUALIFIED_FIXTURE_ONLY headers.
Next step: Bind rubix-release assemble/verify to validated receipts from E35.01 and cell inventory from E32.02, regenerating docs/release reports from receipts; timestamp: 2026-10-07T14:15:00Z

Work item: E36.01 (https://github.com/geoffsee/rubix-kube/issues/351); Parent criteria 1, 3
Outcome: Pending
Readiness: Blocked on G04, E35.01
Ownership: Unassigned
Stack: qual/conformance-suites
Evidence: Conformance suite subsets and manifest tiers have not been run against the candidate Linux node; receipt generation and candidate digest binding pending.
Next step: Document conformance suite selection/exclusions, run smoke and manifest tiers against Linux candidate, and generate validated receipts; timestamp: 2026-10-07T14:15:00Z

Work item: E36.02 (https://github.com/geoffsee/rubix-kube/issues/352); Parent criteria 1, 2, 3
Outcome: Pending
Readiness: Blocked on G04, E35.01
Ownership: Unassigned
Stack: qual/recovery-soak-linux
Evidence: Live recovery rehearsal and 24-hour soak on Linux candidate not executed; component restart, outage escalation, reboot state preservation, and memory growth bound unverified.
Next step: Execute rubix-recovery-rehearsal and 24-hour rubix-platform-soak on disposable Linux host with candidate bytes, capturing validated receipts; timestamp: 2026-10-07T14:15:00Z

Work item: E36.03 (https://github.com/geoffsee/rubix-kube/issues/353); Parent criteria 1, 3
Outcome: Pending
Readiness: Blocked on G04, E35.01
Ownership: Unassigned
Stack: perf/paired-hardware-budgets
Evidence: Paired live performance captures on matched Linux hardware (boot-to-API, idle PSS, distribution size, shutdown, 24h memory) not yet collected; synthetic candidate fixtures remain in place.
Next step: Ingest live rubix-perf captures for arm64 and amd64, evaluating rebaseline policy with rubix-perf gate-ci; timestamp: 2026-10-07T14:15:00Z

Work item: E36.04 (https://github.com/geoffsee/rubix-kube/issues/354); Parent criteria 1, 3
Outcome: Pending
Readiness: Blocked on G01, G04, E35.01
Ownership: Unassigned
Stack: migration/live-rehearsal
Evidence: Live Go-to-Rust migration rehearsal on Linux not yet performed; Kine SQLite backup, Option B explicit export/import state preservation, rollback, and interrupted-stage recovery pending.
Next step: Execute live migration and interrupted recovery rehearsal across supported starting versions, measuring downtime and validating receipts; timestamp: 2026-10-07T14:15:00Z

Work item: E36.05 (https://github.com/geoffsee/rubix-kube/issues/355); Parent criteria 1, 3
Outcome: Pending
Readiness: Blocked on G04, E32.01, E35.01
Ownership: Unassigned
Stack: qual/addons-egress-d2k
Evidence: Addon execution under denied egress, PVC provisioning with pinned BusyBox helper, Portainer object preservation, and live D2K mTLS authentication checks pending.
Next step: Verify offline addon image acquisition with egress denied, test D2K client certificate authentication positive/negative cases, and validate receipts; timestamp: 2026-10-07T14:15:00Z

Work item: E37.01 (https://github.com/geoffsee/rubix-kube/issues/357); Parent criteria 1
Outcome: Pending
Readiness: Blocked on G06, E32.02, E36.04
Ownership: Unassigned
Stack: release/operator-rehearsal
Evidence: Fresh-operator rehearsal from docs alone on disposable Linux host not yet run; candidate artifact install, workload, metrics, migration, and recovery runbooks unverified by independent operator.
Next step: Run operator rehearsal following docs/operator on disposable Linux host with candidate artifacts, capturing receipt and resolving any doc divergence; timestamp: 2026-10-07T14:15:00Z

Work item: E37.02 (https://github.com/geoffsee/rubix-kube/issues/358); Parent criteria 2
Outcome: Pending
Readiness: Blocked on E37.01, G06, E35.02
Ownership: Unassigned
Stack: release/publish-signoff
Evidence: Release publication workflow release.yml and Gate C17 project signoff remain pending; all 11 roadmap completion criteria require validated receipts bound to published digests.
Next step: Publish qualified candidate artifacts, verify rubix-qualification zero exit against published digests, and update docs/architecture/project-signoff.md to record Gate C17; timestamp: 2026-10-07T14:15:00Z
