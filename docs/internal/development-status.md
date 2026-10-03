
Work item: E06.01 (https://github.com/geoffsee/rubix-kube/issues/48); Parent criteria 1, 3
Outcome: Completed
Readiness: Done
Ownership: Unassigned
Stack: payload/asset-inventory
Evidence: Defined Linux ARM64/glibc online and offline payload cells with exact upstream SHA-256 binary/image evidence and optional feature support contract in rubix-assets.
Next step: None; timestamp: 2026-10-02T22:45:00Z

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

Work item: E06.02 (https://github.com/geoffsee/rubix-kube/issues/49); Parent criteria 1, 3
Outcome: Completed
Readiness: Done
Ownership: Unassigned
Stack: materialization/verify-core
Evidence: Implemented safe, atomic, idempotent asset materializer in rubix-assets with Unix executable (0o755) and image (0o644) permissions, path-safety bounds against destination escaping, staging directory isolation, disk reverification before atomic commit, and tests covering ARM64 cell payload verification, idempotency, corrupt archive rejection, and read-only destination handling.
Next step: None; timestamp: 2026-10-02T23:59:00Z

