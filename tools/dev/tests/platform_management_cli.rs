use std::process::Command;
#[test]
fn historical_oracles_run_through_rust_cli() {
    for family in ["platform", "management"] {
        assert!(
            Command::new(env!("CARGO_BIN_EXE_rubix-platform-management"))
                .args(["verify", family])
                .status()
                .unwrap()
                .success()
        );
    }
}
#[test]
fn linux_runtime_cannot_run_without_disposable_marker() {
    for args in [
        vec!["management-runtime"],
        vec!["__chroot", "/tmp/rubix-check-forbidden", "0", "check"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_rubix-platform-management"))
            .args(args)
            .env_remove("RUBIX_MANAGEMENT_DISPOSABLE")
            .output()
            .unwrap();
        assert!(!result.status.success());
    }
}
#[test]
fn existing_output_is_not_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("marker");
    std::fs::write(&marker, b"owned").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rubix-platform-management"))
        .args(["capture-go", "platform", "--output"])
        .arg(directory.path())
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(std::fs::read(marker).unwrap(), b"owned");
}
