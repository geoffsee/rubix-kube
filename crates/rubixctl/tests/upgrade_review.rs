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
