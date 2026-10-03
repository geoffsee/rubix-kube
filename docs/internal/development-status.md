
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
