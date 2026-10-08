//! Integration tests for rubix-disposable-node CLI (E33.03 / Issue #343).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rubix_dev::disposable_node::{
    AssertionRecord, BinaryDigest, CandidateIdentity, CleanupInventory, CommandRecord,
    ComponentVersions, DisposableNodeReceipt, EnvironmentFacts, ReceiptTimestamps, SkipRecord,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn valid_test_receipt() -> DisposableNodeReceipt {
    let mut binaries = BTreeMap::new();
    binaries.insert(
        "rubix-kube".to_string(),
        BinaryDigest {
            sha256: "1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            bytes: 12345,
        },
    );
    binaries.insert(
        "rubixctl".to_string(),
        BinaryDigest {
            sha256: "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            bytes: 67890,
        },
    );

    DisposableNodeReceipt {
        schema_version: 2,
        status: "passed".to_string(),
        qualified: false,
        qualification_reason:
            "Scaffold run only; full live cluster qualification requires E33.02 integration"
                .to_string(),
        timestamps: ReceiptTimestamps {
            started_at: "2026-10-08T20:00:00Z".to_string(),
            completed_at: "2026-10-08T20:00:05Z".to_string(),
            duration_ms: 5000,
        },
        candidate: CandidateIdentity {
            source_revision: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            source_tree_hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            binaries,
        },
        environment: EnvironmentFacts {
            os: "linux".to_string(),
            arch: "aarch64".to_string(),
            kernel: "6.8.0-1014-azure".to_string(),
            cpu_count: 4,
            runner: "ubuntu-24.04-arm".to_string(),
            hostname: "test-runner".to_string(),
        },
        component_versions: ComponentVersions {
            rubix_kube: "0.1.0".to_string(),
            rubixctl: "0.1.0".to_string(),
            rustc: "rustc 1.97.1".to_string(),
            cargo: "cargo 1.97.1".to_string(),
        },
        commands: vec![CommandRecord {
            name: "test_cmd".to_string(),
            command: "rubix-kube --print-config".to_string(),
            exit_code: 0,
            duration_ms: 100,
            stdout_log: "logs/test.stdout.log".to_string(),
            stderr_log: "logs/test.stderr.log".to_string(),
        }],
        assertions: vec![AssertionRecord {
            name: "test_assertion".to_string(),
            passed: true,
            details: "all good".to_string(),
        }],
        skips: vec![SkipRecord {
            name: "e33_02_workload".to_string(),
            reason: "pending merge".to_string(),
        }],
        cleanup: CleanupInventory {
            state_directory_removed: true,
            owned_directories_removed: vec!["/tmp/state".to_string()],
            owned_processes_terminated: vec![],
            leftover_owned_resources: vec![],
        },
    }
}

#[test]
fn cli_help_succeeds() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubix-disposable-node"))
        .arg("--help")
        .output()
        .expect("failed to execute rubix-disposable-node");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: rubix-disposable-node"));
}

#[test]
fn cli_verify_valid_fixture() {
    let temp = tempfile::tempdir().expect("tempdir");
    let logs_dir = temp.path().join("logs");
    fs::create_dir_all(&logs_dir).expect("create logs dir");
    fs::write(logs_dir.join("test.stdout.log"), b"ok").expect("write stdout");
    fs::write(logs_dir.join("test.stderr.log"), b"").expect("write stderr");

    let receipt = valid_test_receipt();
    let json_bytes = serde_json::to_vec_pretty(&receipt).expect("serialize receipt");
    fs::write(temp.path().join("receipt.json"), json_bytes).expect("write receipt");

    let status = Command::new(env!("CARGO_BIN_EXE_rubix-disposable-node"))
        .arg("verify")
        .arg(temp.path())
        .status()
        .expect("run verify");

    assert!(status.success());
}

#[test]
fn cli_verify_rejects_malformed_json() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("receipt.json"), b"{ invalid json }").expect("write invalid json");

    let output = Command::new(env!("CARGO_BIN_EXE_rubix-disposable-node"))
        .arg("verify")
        .arg(temp.path())
        .output()
        .expect("run verify");

    assert!(!output.status.success());
}

#[test]
fn cli_verify_rejects_duplicate_json_keys() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dup_json = br#"{"schema_version": 2, "schema_version": 2}"#;
    fs::write(temp.path().join("receipt.json"), dup_json).expect("write dup json");

    let output = Command::new(env!("CARGO_BIN_EXE_rubix-disposable-node"))
        .arg("verify")
        .arg(temp.path())
        .output()
        .expect("run verify");

    assert!(!output.status.success());
}

#[test]
fn cli_verify_rejects_nonzero_exit_code() {
    let temp = tempfile::tempdir().expect("tempdir");
    let logs_dir = temp.path().join("logs");
    fs::create_dir_all(&logs_dir).expect("create logs dir");
    fs::write(logs_dir.join("test.stdout.log"), b"fail").expect("write stdout");
    fs::write(logs_dir.join("test.stderr.log"), b"err").expect("write stderr");

    let mut receipt = valid_test_receipt();
    receipt.commands[0].exit_code = 1;
    let json_bytes = serde_json::to_vec_pretty(&receipt).expect("serialize receipt");
    fs::write(temp.path().join("receipt.json"), json_bytes).expect("write receipt");

    let output = Command::new(env!("CARGO_BIN_EXE_rubix-disposable-node"))
        .arg("verify")
        .arg(temp.path())
        .output()
        .expect("run verify");

    assert!(!output.status.success());
}

#[test]
fn cli_verify_rejects_missing_log() {
    let temp = tempfile::tempdir().expect("tempdir");
    let receipt = valid_test_receipt();
    let json_bytes = serde_json::to_vec_pretty(&receipt).expect("serialize receipt");
    fs::write(temp.path().join("receipt.json"), json_bytes).expect("write receipt");

    let output = Command::new(env!("CARGO_BIN_EXE_rubix-disposable-node"))
        .arg("verify")
        .arg(temp.path())
        .output()
        .expect("run verify");

    assert!(!output.status.success());
}

#[test]
fn cli_verify_rejects_leftovers() {
    let temp = tempfile::tempdir().expect("tempdir");
    let logs_dir = temp.path().join("logs");
    fs::create_dir_all(&logs_dir).expect("create logs dir");
    fs::write(logs_dir.join("test.stdout.log"), b"ok").expect("write stdout");
    fs::write(logs_dir.join("test.stderr.log"), b"").expect("write stderr");

    let mut receipt = valid_test_receipt();
    receipt.cleanup.leftover_owned_resources = vec!["/var/lib/kubesolo/uncleaned".to_string()];
    let json_bytes = serde_json::to_vec_pretty(&receipt).expect("serialize receipt");
    fs::write(temp.path().join("receipt.json"), json_bytes).expect("write receipt");

    let output = Command::new(env!("CARGO_BIN_EXE_rubix-disposable-node"))
        .arg("verify")
        .arg(temp.path())
        .output()
        .expect("run verify");

    assert!(!output.status.success());
}

#[test]
fn cli_capture_and_verify_live_scaffold() {
    let repo_root = root();
    let kube_bin = repo_root.join("target/debug/rubix-kube");
    let ctl_bin = repo_root.join("target/debug/rubixctl");

    if !kube_bin.is_file() || !ctl_bin.is_file() {
        eprintln!("target debug binaries not found; skipping live capture test");
        return;
    }

    let temp = tempfile::tempdir().expect("tempdir");
    let capture_output = Command::new(env!("CARGO_BIN_EXE_rubix-disposable-node"))
        .arg("capture")
        .arg("--output")
        .arg(temp.path())
        .arg("--kube-bin")
        .arg(&kube_bin)
        .arg("--ctl-bin")
        .arg(&ctl_bin)
        .output()
        .expect("run capture");

    assert!(
        capture_output.status.success(),
        "capture failed: {}",
        String::from_utf8_lossy(&capture_output.stderr)
    );

    // Verify receipt and log files exist on disk
    let receipt_file = temp.path().join("receipt.json");
    assert!(receipt_file.is_file());

    assert!(
        temp.path()
            .join("logs/01_rubix_kube_print_config.stdout.log")
            .is_file()
    );
    assert!(
        temp.path()
            .join("logs/01_rubix_kube_print_config.stderr.log")
            .is_file()
    );
    assert!(
        temp.path()
            .join("logs/02_rubixctl_version.stdout.log")
            .is_file()
    );
    assert!(
        temp.path()
            .join("logs/02_rubixctl_version.stderr.log")
            .is_file()
    );
    assert!(
        temp.path()
            .join("logs/03_teardown_and_cleanup.stdout.log")
            .is_file()
    );
    assert!(
        temp.path()
            .join("logs/03_teardown_and_cleanup.stderr.log")
            .is_file()
    );

    // Verify through CLI
    let verify_output = Command::new(env!("CARGO_BIN_EXE_rubix-disposable-node"))
        .arg("verify")
        .arg(temp.path())
        .output()
        .expect("run verify");

    assert!(
        verify_output.status.success(),
        "verify failed: {}",
        String::from_utf8_lossy(&verify_output.stderr)
    );

    // Check receipt content
    let receipt_raw = fs::read_to_string(&receipt_file).expect("read receipt");
    let parsed: DisposableNodeReceipt = serde_json::from_str(&receipt_raw).expect("parse receipt");
    assert_eq!(parsed.schema_version, 2);
    assert_eq!(parsed.status, "passed");
    assert!(!parsed.qualified);
    assert_eq!(parsed.commands.len(), 3);
    assert!(parsed.cleanup.leftover_owned_resources.is_empty());
}
