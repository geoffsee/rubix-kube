use rubixctl::cleanup::{CleanupKind, ServiceHost, run_container_cleanup, run_host_cleanup};
use rubixctl::service::{
    CustomServicePaths, InitBackend, LifecycleAction, ServiceConfig, plan_lifecycle_action,
};
use rubixctl::upgrade::{ContainerSpec, Runner};
use std::{fs, io, path::PathBuf};

const BACKENDS: [InitBackend; 6] = [
    InitBackend::Systemd,
    InitBackend::OpenRc,
    InitBackend::SysVinit,
    InitBackend::Upstart,
    InitBackend::Runit,
    InitBackend::S6,
];

#[derive(Debug)]
struct Commands {
    data: PathBuf,
    calls: Vec<String>,
    fail_at: Option<usize>,
}
impl Runner for Commands {
    fn run(&mut self, program: &str, args: &[String]) -> io::Result<String> {
        // The same inode must remain locked through every lifecycle effect.
        let other = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.data.join(".upgrade.lock"))?;
        assert!(other.try_lock().is_err());
        self.calls.push(format!("{program} {}", args.join(" ")));
        if self.fail_at == Some(self.calls.len() - 1) {
            Err(io::Error::other("injected lifecycle failure"))
        } else {
            Ok(String::new())
        }
    }
}

fn fixture(backend: InitBackend) -> (tempfile::TempDir, ServiceConfig, Vec<PathBuf>) {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    fs::create_dir_all(data.join("kine/db")).unwrap();
    fs::write(data.join("kine/db/state.db"), "retained state").unwrap();
    let config = ServiceConfig {
        backend: Some(backend),
        binary_path: root.path().join("bin/node"),
        custom_paths: CustomServicePaths {
            root_prefix: Some(root.path().into()),
            ..CustomServicePaths::default()
        },
        ..ServiceConfig::default()
    };
    let mut artifacts = plan_lifecycle_action(LifecycleAction::Uninstall, &config)
        .unwrap()
        .cleanup_paths;
    if backend == InitBackend::OpenRc {
        artifacts.push(root.path().join("etc/conf.d/kubesolo"));
    }
    artifacts.push(config.binary_path.clone());
    for artifact in &artifacts {
        assert!(artifact.starts_with(root.path()));
        fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        fs::write(artifact, "owned artifact").unwrap();
    }
    (root, config, artifacts)
}

#[test]
fn shared_upgrade_lock_refuses_host_and_container_cleanup_before_effects() {
    let (root, config, artifacts) = fixture(InitBackend::Systemd);
    let data = root.path().join("data");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data.join(".upgrade.lock"))
        .unwrap();
    lock.lock().unwrap();
    let mut runner = Commands {
        data: data.clone(),
        calls: vec![],
        fail_at: None,
    };
    {
        let mut host = ServiceHost::new(&mut runner, config).unwrap();
        assert!(
            run_host_cleanup(
                &mut host,
                CleanupKind::Reset,
                &data,
                true,
                &mut io::empty(),
                &mut Vec::new()
            )
            .is_err()
        );
    }
    let spec = ContainerSpec::parse("name=rubix\nimage=example/node:v1.2.0\n").unwrap();
    assert!(
        run_container_cleanup(
            &mut runner,
            "docker",
            &spec,
            CleanupKind::Uninstall { purge: true },
            &data,
            true,
            &mut io::empty(),
            &mut Vec::new()
        )
        .is_err()
    );
    assert!(runner.calls.is_empty());
    assert!(data.join("kine/db/state.db").exists());
    assert!(artifacts.iter().all(|path| path.exists()));
}

#[test]
fn active_recovery_receipts_block_reset_and_uninstall_but_explicit_purge_can_discard_them() {
    for receipt in [".upgrade-pending", ".upgrade-committing"] {
        for kind in [
            CleanupKind::Reset,
            CleanupKind::Uninstall { purge: false },
            CleanupKind::Uninstall { purge: true },
        ] {
            for container in [false, true] {
                let (root, config, _) = fixture(InitBackend::Systemd);
                let data = root.path().join("data");
                fs::write(data.join(receipt), "backup=retained").unwrap();
                let mut runner = Commands {
                    data: data.clone(),
                    calls: vec![],
                    fail_at: None,
                };
                let result = if container {
                    let spec =
                        ContainerSpec::parse("name=rubix\nimage=example/node:v1.2.0\n").unwrap();
                    run_container_cleanup(
                        &mut runner,
                        "docker",
                        &spec,
                        kind,
                        &data,
                        true,
                        &mut io::empty(),
                        &mut Vec::new(),
                    )
                } else {
                    let mut host = ServiceHost::new(&mut runner, config).unwrap();
                    run_host_cleanup(
                        &mut host,
                        kind,
                        &data,
                        true,
                        &mut io::empty(),
                        &mut Vec::new(),
                    )
                };
                if kind == (CleanupKind::Uninstall { purge: true }) {
                    result.unwrap();
                    assert!(!data.join(receipt).exists());
                    assert!(data.join(".upgrade.lock").exists());
                } else {
                    assert!(
                        result
                            .unwrap_err()
                            .to_string()
                            .contains("interrupted upgrade")
                    );
                    assert!(runner.calls.is_empty());
                    assert!(data.join("kine/db/state.db").exists());
                    assert!(data.join(receipt).exists());
                }
            }
        }
    }
}

fn expected_commands(config: &ServiceConfig, action: LifecycleAction) -> Vec<String> {
    let mut commands: Vec<String> = plan_lifecycle_action(action, config)
        .unwrap()
        .commands
        .into_iter()
        .map(|step| format!("{} {}", step.program, step.args.join(" ")))
        .collect();
    if config.backend == Some(InitBackend::S6) && action != LifecycleAction::Start {
        commands[0] = commands[0].replacen("s6-svc -d", "s6-svc -wD -T 30000 -d", 1);
    }
    commands
}

#[test]
fn all_detected_backends_unregister_before_artifact_deletion_and_reset_only_restarts() {
    for backend in BACKENDS {
        for kind in [CleanupKind::Reset, CleanupKind::Uninstall { purge: false }] {
            let (root, config, artifacts) = fixture(backend);
            let data = root.path().join("data");
            let mut expected = if kind == CleanupKind::Reset {
                let mut calls = expected_commands(&config, LifecycleAction::Stop);
                calls.extend(expected_commands(&config, LifecycleAction::Start));
                calls
            } else {
                expected_commands(&config, LifecycleAction::Uninstall)
            };
            if kind != CleanupKind::Reset
                && matches!(backend, InitBackend::Systemd | InitBackend::Upstart)
            {
                expected.push(expected.last().unwrap().clone());
            }
            let mut runner = Commands {
                data: data.clone(),
                calls: vec![],
                fail_at: None,
            };
            let mut host = ServiceHost::new(&mut runner, config).unwrap();
            run_host_cleanup(
                &mut host,
                kind,
                &data,
                true,
                &mut io::empty(),
                &mut Vec::new(),
            )
            .unwrap();
            assert_eq!(runner.calls, expected, "{backend:?}");
            assert_eq!(
                artifacts.iter().all(|path| path.exists()),
                kind == CleanupKind::Reset
            );
            assert_eq!(
                data.join("kine/db/state.db").exists(),
                kind != CleanupKind::Reset
            );
        }
    }
}

#[test]
fn lifecycle_failures_abort_before_state_and_artifact_removal() {
    for backend in BACKENDS {
        let (_, config, _) = fixture(backend);
        let commands = expected_commands(&config, LifecycleAction::Uninstall);
        for failure in 0..commands.len() {
            let (root, config, artifacts) = fixture(backend);
            let data = root.path().join("data");
            let mut runner = Commands {
                data: data.clone(),
                calls: vec![],
                fail_at: Some(failure),
            };
            let mut host = ServiceHost::new(&mut runner, config).unwrap();
            let result = run_host_cleanup(
                &mut host,
                CleanupKind::Uninstall { purge: true },
                &data,
                true,
                &mut io::empty(),
                &mut Vec::new(),
            );
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("injected lifecycle failure")
            );
            assert!(
                data.join("kine/db/state.db").exists(),
                "{backend:?} step {failure}"
            );
            assert!(
                artifacts.iter().all(|path| path.exists()),
                "{backend:?} step {failure}"
            );
        }
    }
}

#[test]
fn unsupported_backend_fails_before_any_cleanup_effect() {
    let (root, mut config, _) = fixture(InitBackend::Systemd);
    config.backend = None;
    let mut runner = Commands {
        data: root.path().join("data"),
        calls: vec![],
        fail_at: None,
    };
    assert!(ServiceHost::new(&mut runner, config).is_err());
    assert!(runner.calls.is_empty());
}

#[test]
fn cache_reload_failure_after_definition_removal_is_reported() {
    for backend in [InitBackend::Systemd, InitBackend::Upstart] {
        let (root, config, artifacts) = fixture(backend);
        let data = root.path().join("data");
        let failure = expected_commands(&config, LifecycleAction::Uninstall).len();
        let mut runner = Commands {
            data: data.clone(),
            calls: vec![],
            fail_at: Some(failure),
        };
        let mut host = ServiceHost::new(&mut runner, config).unwrap();
        let result = run_host_cleanup(
            &mut host,
            CleanupKind::Uninstall { purge: false },
            &data,
            true,
            &mut io::empty(),
            &mut Vec::new(),
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("injected lifecycle failure")
        );
        assert_eq!(runner.calls.len(), failure + 1);
        assert!(artifacts.iter().all(|path| !path.exists()));
        assert!(data.join("kine/db/state.db").exists());
    }
}
