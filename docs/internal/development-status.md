
Work item: E06.01 (https://github.com/geoffsee/rubix-kube/issues/48); Parent criteria 1, 3
Outcome: Blocked
Readiness: Blocked
Ownership: Unassigned
Stack: payload/asset-inventory
Evidence: Linux ARM64/glibc manifests validate structurally. Kubernetes v1.35.7 arm64 kube-apiserver, kube-controller-manager, kubelet, and kube-proxy hashes and sizes match dl.k8s.io. Image, CNI, containerd, crun, and shim rows use placeholder digests until encoded-byte pins exist, so the manifests are not an accepted payload cell.
Next step: Pin encoded bytes for images, CNI, containerd, crun, and the shim; timestamp: 2026-10-02T23:50:00Z

Work item: E27.01 (https://github.com/geoffsee/rubix-kube/issues/114); Parent criteria 1, 2, 3
Outcome: Completed
Readiness: Done
Ownership: Unassigned
Stack: build/node-variants
Evidence: Defined exhaustive 16-cell node variant matrix (4 architectures x 2 libcs x 2 online/offline variants), 4 management targets (Linux/Darwin amd64/arm64) with explicit Windows exclusion per E01, optional-image enforcement per cell, and clean-checkout reproducible artifact naming in rubix-assets and rubix-matrix CLI.
Next step: None; timestamp: 2026-10-02T22:54:00Z

Work item: E07.02 (https://github.com/geoffsee/rubix-kube/issues/52)
Outcome: Completed
Readiness: Done
Ownership: Unassigned
Stack: None
Evidence: All `cargo test --workspace --locked` and `cargo test -p rubix-dev --lib` pass cleanly. `rubix-pki` is integrated with `rcgen 0.14` and all parity test evidence (source inventories, qualifications, receipts, JSON tree) hashes have been cascaded correctly.
Next step: None; timestamp: 2026-09-29T16:35:00Z
