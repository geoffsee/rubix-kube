
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
Outcome: Completed
Readiness: Done
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
