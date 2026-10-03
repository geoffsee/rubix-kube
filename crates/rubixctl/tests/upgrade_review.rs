use rubixctl::upgrade::{
    ContainerBackend, ContainerSpec, HostBackend, Runner, TransitionBackend, UpgradeOutcome,
    backup_state, run_upgrade,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Default)]
struct Recorded {
    calls: Vec<String>,
    fail: Option<String>,
    running: bool,
}

impl Runner for Recorded {
    fn run(&mut self, program: &str, args: &[String]) -> io::Result<String> {
        let call = format!("{program} {}", args.join(" "));
        self.calls.push(call.clone());
        if self
            .fail
            .as_ref()
            .is_some_and(|needle| call.contains(needle))
        {
            return Err(io::Error::other("injected failure"));
        }
        if args.first().is_some_and(|arg| arg == "inspect") {
            return Ok(self.running.to_string());
        }
        Ok("v1.2.0".into())
    }
}

fn data() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("kine/db")).unwrap();
    fs::create_dir_all(dir.path().join("pki")).unwrap();
    fs::write(dir.path().join("kine/db/state.db"), "old-db").unwrap();
    fs::write(dir.path().join("kine/db/state.db-wal"), "old-wal").unwrap();
    fs::write(dir.path().join("pki/ca.key"), "old-key").unwrap();
    dir
}

#[test]
fn backups_capture_real_layout_are_private_and_never_reuse_a_timestamp() {
    let dir = data();
    let first = backup_state(dir.path(), None, "v1.2.0", 7).unwrap();
    let second = backup_state(dir.path(), None, "v1.2.0", 7).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        fs::read_to_string(first.join("kine/db/state.db-wal")).unwrap(),
        "old-wal"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(first).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}

#[cfg(unix)]
#[test]
fn backup_refuses_symlinked_datastore_roots_and_ancestors() {
    for relative in ["kine", "kine/db", "backups"] {
        let dir = data();
        let foreign = tempfile::tempdir().unwrap();
        let linked = dir.path().join(relative);
        if linked.exists() {
            fs::remove_dir_all(&linked).unwrap();
        }
        std::os::unix::fs::symlink(foreign.path(), linked).unwrap();
        assert!(backup_state(dir.path(), None, "v1.2.0", 1).is_err());
        assert!(fs::read_dir(foreign.path()).unwrap().next().is_none());
    }
}

#[cfg(unix)]
#[test]
fn host_binary_replacement_is_executable_without_exposing_config_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = data();
    let binary = dir.path().join("kubesolo");
    let staged = dir.path().join("staged");
    fs::write(&binary, "old").unwrap();
    fs::write(&staged, "new").unwrap();
    fs::set_permissions(&staged, fs::Permissions::from_mode(0o600)).unwrap();
    let mut runner = Recorded::default();
    let mut backend = HostBackend {
        binary: binary.clone(),
        staged,
        service: "kubesolo".into(),
        runner: &mut runner,
        systemd: true,
        service_file: None,
        legacy_config: None,
    };
    backend.replace("v1.3.0").unwrap();
    assert_eq!(
        fs::metadata(binary).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

#[test]
fn container_commit_persists_version_and_failed_launch_restores_record() {
    for failure in [None, Some("run -d"), Some("rm rubix-pre-upgrade")] {
        let dir = data();
        let spec_path = dir.path().join("container.spec");
        let original = "name=rubix\nimage=example/node:v1.2.0\narg=--debug\nport=6443:6443\n";
        fs::write(&spec_path, original).unwrap();
        let spec = ContainerSpec::parse(original).unwrap();
        let mut runner = Recorded {
            running: true,
            fail: failure.map(str::to_string),
            ..Recorded::default()
        };
        let mut backend =
            ContainerBackend::new("docker", spec, &mut runner).with_spec_path(spec_path.clone());
        let result =
            run_upgrade(&mut backend, dir.path(), None, "v1.3.0", 1, &mut Vec::new()).unwrap();
        let recorded = ContainerSpec::parse(&fs::read_to_string(spec_path).unwrap()).unwrap();
        if failure.is_none() {
            assert!(matches!(result, UpgradeOutcome::Upgraded { .. }));
            assert_eq!(recorded.tag(), Some("v1.3.0"));
            assert_eq!(
                run_upgrade(&mut backend, dir.path(), None, "v1.3.0", 1, &mut Vec::new()).unwrap(),
                UpgradeOutcome::Unchanged
            );
        } else {
            assert!(matches!(result, UpgradeOutcome::RolledBack { .. }));
            assert_eq!(recorded.tag(), Some("v1.2.0"));
        }
    }
}

#[test]
fn exited_replacement_does_not_discard_old_container() {
    let dir = data();
    let mut runner = Recorded::default();
    let spec = ContainerSpec::parse("name=rubix\nimage=example/node:v1.2.0\n").unwrap();
    let mut backend = ContainerBackend::new("docker", spec, &mut runner);
    let result = run_upgrade(&mut backend, dir.path(), None, "v1.3.0", 1, &mut Vec::new()).unwrap();
    assert!(matches!(result, UpgradeOutcome::RolledBack { .. }));
    assert!(
        !runner
            .calls
            .iter()
            .any(|call| call == "docker rm rubix-pre-upgrade")
    );
    assert!(runner.calls.iter().any(|call| call == "docker start rubix"));
}

#[derive(Debug)]
struct MutatingStart {
    data: PathBuf,
    calls: Vec<String>,
    first: bool,
}
impl Runner for MutatingStart {
    fn run(&mut self, program: &str, args: &[String]) -> io::Result<String> {
        self.calls.push(format!("{program} {}", args.join(" ")));
        if args.first().is_some_and(|arg| arg == "start") && self.first {
            self.first = false;
            fs::write(self.data.join("kine/db/state.db"), "new-db")?;
            fs::write(self.data.join("kine/db/new-file"), "new")?;
            fs::write(self.data.join("pki/ca.key"), "new-key")?;
            return Err(io::Error::other("startup failed after mutations"));
        }
        Ok("v1.2.0".into())
    }
}

#[test]
fn host_rollback_restores_all_state_and_removes_migration_created_config() {
    let dir = data();
    let binary = dir.path().join("kubesolo");
    let staged = dir.path().join("staged");
    let service = dir.path().join("node.service");
    let config = dir.path().join("config.yaml");
    let original = "Description=kubesolo\nExecStart=/usr/local/bin/kubesolo --debug\n";
    fs::write(&binary, "old-binary").unwrap();
    fs::write(&staged, "new-binary").unwrap();
    fs::write(&service, original).unwrap();
    let mut runner = MutatingStart {
        data: dir.path().to_owned(),
        calls: vec![],
        first: true,
    };
    let mut backend = HostBackend {
        binary: binary.clone(),
        staged,
        service: "kubesolo".into(),
        runner: &mut runner,
        systemd: true,
        service_file: Some(service.clone()),
        legacy_config: Some(config.clone()),
    };
    let result = run_upgrade(
        &mut backend,
        dir.path(),
        Some(&config),
        "v1.3.0",
        1,
        &mut Vec::new(),
    )
    .unwrap();
    assert!(matches!(result, UpgradeOutcome::RolledBack { .. }));
    assert_eq!(fs::read_to_string(binary).unwrap(), "old-binary");
    assert_eq!(fs::read_to_string(service).unwrap(), original);
    assert_eq!(
        fs::read_to_string(dir.path().join("kine/db/state.db")).unwrap(),
        "old-db"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("pki/ca.key")).unwrap(),
        "old-key"
    );
    assert!(!dir.path().join("kine/db/new-file").exists());
    assert!(!config.exists());
    assert_eq!(
        runner
            .calls
            .iter()
            .filter(|call| *call == "systemctl daemon-reload")
            .count(),
        2
    );
}

struct BackupFailure {
    data: PathBuf,
    calls: Vec<&'static str>,
}
impl TransitionBackend for BackupFailure {
    fn current_version(&mut self) -> io::Result<String> {
        Ok("v1.2.0".into())
    }
    fn prepare(&mut self, _: &str) -> io::Result<()> {
        self.calls.push("prepare");
        Ok(())
    }
    fn stop(&mut self) -> io::Result<()> {
        self.calls.push("stop");
        fs::write(self.data.join("kine/db/state.db"), "quiesced")
    }
    fn snapshot(&mut self, dir: &Path) -> io::Result<()> {
        assert_eq!(
            fs::read_to_string(dir.join("kine/db/state.db"))?,
            "quiesced"
        );
        self.calls.push("snapshot");
        Err(io::Error::other("snapshot unavailable"))
    }
    fn replace(&mut self, _: &str) -> io::Result<()> {
        panic!("must not replace without backup")
    }
    fn start(&mut self) -> io::Result<()> {
        self.calls.push("start");
        Ok(())
    }
    fn restore(&mut self, _: &Path) -> io::Result<()> {
        panic!("no replacement happened")
    }
}

#[test]
fn snapshot_is_quiesced_and_snapshot_failure_restarts_old_deployment() {
    let dir = data();
    let mut backend = BackupFailure {
        data: dir.path().to_owned(),
        calls: vec![],
    };
    assert!(run_upgrade(&mut backend, dir.path(), None, "v1.3.0", 1, &mut Vec::new()).is_err());
    assert_eq!(backend.calls, ["prepare", "stop", "snapshot", "start"]);
}

#[test]
fn live_lock_and_interrupted_receipt_refuse_new_mutations() {
    let dir = data();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.path().join(".upgrade.lock"))
        .unwrap();
    lock.lock().unwrap();
    let mut backend = BackupFailure {
        data: dir.path().to_owned(),
        calls: vec![],
    };
    assert!(run_upgrade(&mut backend, dir.path(), None, "v1.3.0", 1, &mut Vec::new()).is_err());
    assert!(backend.calls.is_empty());
    drop(lock);
    fs::write(
        dir.path().join(".upgrade-pending"),
        "backup=/retained/backup",
    )
    .unwrap();
    assert!(run_upgrade(&mut backend, dir.path(), None, "v1.3.0", 1, &mut Vec::new()).is_err());
    assert!(backend.calls.is_empty());
}

#[derive(Clone, Copy)]
enum ReceiptFailure {
    BeforeCommit,
    CommitAndRollback,
    CompletionRename,
    CompletedCleanup,
}

struct ReceiptBackend {
    data: PathBuf,
    failure: ReceiptFailure,
    committed: bool,
    restored: bool,
}
impl TransitionBackend for ReceiptBackend {
    fn current_version(&mut self) -> io::Result<String> {
        Ok(if self.committed { "v1.3.0" } else { "v1.2.0" }.into())
    }
    fn prepare(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn snapshot(&mut self, _: &Path) -> io::Result<()> {
        Ok(())
    }
    fn stop(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn replace(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn start(&mut self) -> io::Result<()> {
        if !self.restored && matches!(self.failure, ReceiptFailure::BeforeCommit) {
            // An invalid receipt cannot be safely staged or removed as a file.
            fs::remove_file(self.data.join(".upgrade-pending"))?;
            fs::create_dir(self.data.join(".upgrade-pending"))?;
        }
        Ok(())
    }
    fn restore(&mut self, _: &Path) -> io::Result<()> {
        assert!(
            !self.committed,
            "irreversible commit must never be rolled back"
        );
        if matches!(self.failure, ReceiptFailure::CommitAndRollback) {
            return Err(io::Error::other("rollback unavailable"));
        }
        self.restored = true;
        Ok(())
    }
    fn commit(&mut self) -> io::Result<()> {
        assert!(!self.data.join(".upgrade-pending").exists());
        assert!(self.data.join(".upgrade-committing").is_file());
        if matches!(self.failure, ReceiptFailure::CommitAndRollback) {
            return Err(io::Error::other("commit unavailable"));
        }
        self.committed = true;
        match self.failure {
            ReceiptFailure::CompletionRename => {
                fs::create_dir(self.data.join(".upgrade-completed"))?;
                fs::write(self.data.join(".upgrade-completed/obstruction"), "x")?;
            },
            ReceiptFailure::CompletedCleanup => {
                fs::remove_file(self.data.join(".upgrade-committing"))?;
                fs::create_dir(self.data.join(".upgrade-committing"))?;
            },
            _ => {},
        }
        Ok(())
    }
}

#[test]
fn receipt_failures_keep_recovery_evidence_and_never_report_a_committed_upgrade_as_failed() {
    for failure in [
        ReceiptFailure::BeforeCommit,
        ReceiptFailure::CommitAndRollback,
        ReceiptFailure::CompletionRename,
        ReceiptFailure::CompletedCleanup,
    ] {
        let dir = data();
        let mut backend = ReceiptBackend {
            data: dir.path().into(),
            failure,
            committed: false,
            restored: false,
        };
        let mut stderr = Vec::new();
        let outcome = run_upgrade(&mut backend, dir.path(), None, "v1.3.0", 1, &mut stderr);
        match failure {
            ReceiptFailure::BeforeCommit => {
                let error = outcome.unwrap_err();
                let diagnostic = error.to_string();
                assert!(diagnostic.contains("upgrade failed ("));
                assert!(diagnostic.contains("rolled back to v1.2.0"));
                assert!(
                    diagnostic.contains(&dir.path().join(".upgrade-pending").display().to_string())
                );
                assert!(diagnostic.contains("backup kept at"));
                assert!(diagnostic.contains(&dir.path().join("backups").display().to_string()));
                assert!(diagnostic.contains("Rollback is complete"));
                assert!(backend.restored);
                assert!(!backend.committed);
                assert!(dir.path().join(".upgrade-pending").exists());
            },
            ReceiptFailure::CommitAndRollback => {
                assert!(outcome.unwrap_err().to_string().contains("rollback failed"));
                assert!(dir.path().join(".upgrade-committing").is_file());
                assert!(
                    run_upgrade(&mut backend, dir.path(), None, "v1.3.0", 2, &mut Vec::new())
                        .unwrap_err()
                        .to_string()
                        .contains("interrupted upgrade")
                );
            },
            ReceiptFailure::CompletionRename | ReceiptFailure::CompletedCleanup => {
                assert!(matches!(outcome.unwrap(), UpgradeOutcome::Upgraded { .. }));
                assert!(backend.committed);
                assert!(!backend.restored);
                assert!(
                    String::from_utf8(stderr)
                        .unwrap()
                        .contains("Upgrade committed; receipt cleanup failed")
                );
                assert!(
                    run_upgrade(&mut backend, dir.path(), None, "v1.3.0", 2, &mut Vec::new())
                        .is_err()
                );
            },
        }
    }
}

#[derive(Debug)]
struct InactiveRunner {
    status: Option<i32>,
    probe_error: bool,
}
impl Runner for InactiveRunner {
    fn run(&mut self, _: &str, _: &[String]) -> io::Result<String> {
        Err(io::Error::other("stop denied"))
    }
    fn status_code(&mut self, program: &str, args: &[String]) -> io::Result<Option<i32>> {
        assert_eq!(program, "service");
        assert_eq!(args, ["kubesolo", "status"]);
        if self.probe_error {
            Err(io::Error::other("cannot query service"))
        } else {
            Ok(self.status)
        }
    }
}

#[test]
fn non_systemd_restore_accepts_only_explicit_inactive_status() {
    for (status, probe_error) in [
        (Some(3), false),
        (Some(0), false),
        (Some(4), false),
        (None, false),
        (Some(3), true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("node");
        fs::write(&binary, "new").unwrap();
        fs::write(dir.path().join("kubesolo.bin"), "old").unwrap();
        let mut runner = InactiveRunner {
            status,
            probe_error,
        };
        let mut backend = HostBackend {
            binary: binary.clone(),
            staged: binary.clone(),
            service: "kubesolo".into(),
            runner: &mut runner,
            systemd: false,
            service_file: None,
            legacy_config: None,
        };
        let outcome = backend.restore(dir.path());
        if status == Some(3) && !probe_error {
            outcome.unwrap();
            assert_eq!(fs::read_to_string(&binary).unwrap(), "old");
        } else {
            assert_eq!(outcome.unwrap_err().to_string(), "stop denied");
            assert_eq!(fs::read_to_string(&binary).unwrap(), "new");
        }
    }
}

#[test]
fn a_durable_completed_receipt_is_cleaned_without_ambiguous_recovery() {
    let dir = data();
    fs::write(
        dir.path().join(".upgrade-completed"),
        "from=v1.2.0\ntarget=v1.3.0\n",
    )
    .unwrap();
    let mut backend = ReceiptBackend {
        data: dir.path().into(),
        failure: ReceiptFailure::CommitAndRollback,
        committed: true,
        restored: false,
    };
    assert_eq!(
        run_upgrade(&mut backend, dir.path(), None, "v1.3.0", 1, &mut Vec::new()).unwrap(),
        UpgradeOutcome::Unchanged
    );
    assert!(!dir.path().join(".upgrade-completed").exists());
    assert!(!backend.restored);
}

struct RejectSuccessOutput;
impl io::Write for RejectSuccessOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if String::from_utf8_lossy(bytes).contains("Upgraded to") {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "output closed after commit",
            ))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn success_output_failure_does_not_turn_an_irreversible_commit_into_failure() {
    let dir = data();
    let mut runner = Recorded {
        running: true,
        ..Recorded::default()
    };
    let spec = ContainerSpec::parse("name=rubix\nimage=example/node:v1.2.0\n").unwrap();
    let mut backend = ContainerBackend::new("docker", spec, &mut runner);
    assert!(matches!(
        run_upgrade(
            &mut backend,
            dir.path(),
            None,
            "v1.3.0",
            1,
            &mut RejectSuccessOutput
        )
        .unwrap(),
        UpgradeOutcome::Upgraded { .. }
    ));
    assert!(
        runner
            .calls
            .iter()
            .any(|call| call == "docker rm rubix-pre-upgrade")
    );
    assert!(
        !runner
            .calls
            .iter()
            .any(|call| call.starts_with("docker rm -f"))
    );
    assert!(!dir.path().join(".upgrade-pending").exists());
    assert!(!dir.path().join(".upgrade-committing").exists());
    assert!(!dir.path().join(".upgrade-completed").exists());
}

#[test]
fn interrupted_upgrade_recovery_restores_pre_upgrade_state_from_pending_receipt() {
    use rubixctl::upgrade::{
        ReceiptKind, RecoveryOutcome, find_active_receipt, parse_receipt_file,
        recover_interrupted_upgrade,
    };

    let dir = data();
    let binary = dir.path().join("kubesolo");
    let staged = dir.path().join("staged");
    let service = dir.path().join("node.service");
    let config = dir.path().join("config.yaml");

    let original_service = "Description=kubesolo\nExecStart=/usr/local/bin/kubesolo --debug\n";
    fs::write(&binary, "old-binary").unwrap();
    fs::write(&staged, "new-binary").unwrap();
    fs::write(&service, original_service).unwrap();

    // Create a valid backup
    let backup_dir = dir.path().join("backups/pre-upgrade-v1.2.0-1-valid");
    fs::create_dir_all(backup_dir.join("pki")).unwrap();
    fs::create_dir_all(backup_dir.join("kine/db")).unwrap();
    fs::write(backup_dir.join("pki/ca.key"), "old-key").unwrap();
    fs::write(backup_dir.join("kine/db/state.db"), "old-db").unwrap();
    fs::write(backup_dir.join("kubesolo.bin"), "old-binary").unwrap();
    fs::write(backup_dir.join("service.unit"), original_service).unwrap();

    // Simulate an interruption after replacement and dirty mutation:
    fs::write(&binary, "new-corrupted-binary").unwrap();
    fs::write(
        &service,
        "Description=kubesolo\nExecStart=/usr/local/bin/kubesolo --config=/etc/kubesolo/config.yaml\n",
    )
    .unwrap();
    fs::write(&config, "network:\n  nodeIP: 10.0.0.1\n").unwrap();
    fs::write(dir.path().join("pki/ca.key"), "dirty-key").unwrap();
    fs::write(dir.path().join("kine/db/state.db"), "dirty-db").unwrap();

    // Create .upgrade-pending receipt
    let pending_receipt = dir.path().join(".upgrade-pending");
    fs::write(
        &pending_receipt,
        format!(
            "from=v1.2.0\ntarget=v1.3.0\nbackup={}\n",
            backup_dir.display()
        ),
    )
    .unwrap();

    let active = find_active_receipt(dir.path()).unwrap().unwrap();
    assert_eq!(active.kind, ReceiptKind::Pending);
    assert_eq!(active.from, "v1.2.0");
    assert_eq!(active.target, "v1.3.0");
    assert_eq!(active.backup, backup_dir);

    let parsed = parse_receipt_file(&pending_receipt, ReceiptKind::Pending).unwrap();
    assert_eq!(parsed.from, "v1.2.0");

    let mut runner = Recorded::default();
    let mut backend = HostBackend {
        binary: binary.clone(),
        staged,
        service: "kubesolo".into(),
        runner: &mut runner,
        systemd: true,
        service_file: Some(service.clone()),
        legacy_config: Some(config.clone()),
    };

    let outcome =
        recover_interrupted_upgrade(&mut backend, dir.path(), Some(&config), &mut Vec::new())
            .unwrap();

    assert_eq!(
        outcome,
        RecoveryOutcome::RolledBack {
            from: "v1.2.0".into(),
            backup: backup_dir.clone(),
        }
    );

    // Verify all state restored cleanly
    assert_eq!(fs::read_to_string(&binary).unwrap(), "old-binary");
    assert_eq!(fs::read_to_string(&service).unwrap(), original_service);
    assert_eq!(
        fs::read_to_string(dir.path().join("pki/ca.key")).unwrap(),
        "old-key"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("kine/db/state.db")).unwrap(),
        "old-db"
    );
    assert!(!config.exists(), "migration-created config was cleaned up");
    assert!(!pending_receipt.exists(), "pending receipt was cleaned up");

    // Re-running recovery reports already clean
    let clean =
        recover_interrupted_upgrade(&mut backend, dir.path(), Some(&config), &mut Vec::new())
            .unwrap();
    assert_eq!(clean, RecoveryOutcome::AlreadyClean);
}

#[test]
fn interrupted_upgrade_recovery_refuses_corrupted_or_missing_backup_without_mutation() {
    use rubixctl::upgrade::{
        BackupIntegrityError, recover_interrupted_upgrade, validate_backup_integrity,
    };

    let dir = data();
    let binary = dir.path().join("kubesolo");
    let staged = dir.path().join("staged");
    let service = dir.path().join("node.service");

    fs::write(&binary, "current-binary").unwrap();
    fs::write(&staged, "new-binary").unwrap();
    fs::write(&service, "service-content").unwrap();

    // 1. Missing backup directory
    let non_existent_backup = dir.path().join("backups/missing");
    let pending_receipt = dir.path().join(".upgrade-pending");
    fs::write(
        &pending_receipt,
        format!(
            "from=v1.2.0\ntarget=v1.3.0\nbackup={}\n",
            non_existent_backup.display()
        ),
    )
    .unwrap();

    let mut runner = Recorded::default();
    let mut backend = HostBackend {
        binary: binary.clone(),
        staged: staged.clone(),
        service: "kubesolo".into(),
        runner: &mut runner,
        systemd: true,
        service_file: Some(service.clone()),
        legacy_config: None,
    };

    let err =
        recover_interrupted_upgrade(&mut backend, dir.path(), None, &mut Vec::new()).unwrap_err();
    assert!(
        err.to_string().contains("recovery refused: backup at"),
        "error must explain backup refusal: {err}"
    );
    // Crucial: pending receipt and files were NOT destroyed!
    assert!(pending_receipt.exists());
    assert_eq!(fs::read_to_string(&binary).unwrap(), "current-binary");

    // 2. Corrupted backup (missing kine/db state directory)
    let corrupt_backup = dir.path().join("backups/corrupt");
    fs::create_dir_all(corrupt_backup.join("pki")).unwrap();
    fs::write(corrupt_backup.join("pki/ca.crt"), "ca").unwrap();
    // Missing kine/db directory
    assert!(matches!(
        validate_backup_integrity(&corrupt_backup),
        Err(BackupIntegrityError::MissingStateDirectory { .. })
    ));

    fs::write(
        &pending_receipt,
        format!(
            "from=v1.2.0\ntarget=v1.3.0\nbackup={}\n",
            corrupt_backup.display()
        ),
    )
    .unwrap();

    let err2 =
        recover_interrupted_upgrade(&mut backend, dir.path(), None, &mut Vec::new()).unwrap_err();
    assert!(
        err2.to_string()
            .contains("missing required state directory")
    );
    assert!(pending_receipt.exists());
}
