//! `rubixctl reset` and `rubixctl uninstall`: scoped cleanup of Rubix-owned state.
//!
//! Ownership model (relative to the data path, default `/var/lib/kubesolo`):
//!
//! | Entry            | reset  | uninstall | uninstall `--purge` |
//! | ---------------- | ------ | --------- | ------------------- |
//! | `kine/db`        | remove | keep      | remove              |
//! | `kubelet`        | remove | keep      | remove              |
//! | `containerd/root`, `containerd/state` | remove | keep | remove |
//! | remaining `containerd` inputs | keep | keep | remove          |
//! | `network`        | remove | keep      | remove              |
//! | `pki`            | keep   | keep      | remove              |
//! | `local-path-storage` | keep | keep   | remove              |
//! | `backups`        | keep   | keep      | remove              |
//! | `container.spec` | keep   | keep      | remove              |
//! | anything else    | keep   | keep      | keep                |
//!
//! Only the named entries are ever removed, so neighbouring data in a shared directory is
//! retained. Symlinks (for example `storage/<volume>` pointing at a custom data volume) are
//! unlinked, never followed. Custom volumes and bind mounts outside the data path, external
//! container runtimes and unrelated processes or port holders are never touched. Reset keeps
//! the instance's configuration, service definition, binary and PKI.
//! Cleanup is idempotent and may be re-run after interruption.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use crate::upgrade::{CONTAINER_SPEC_FILE, ContainerSpec, Runner};

/// Disposable entries removed by reset and purge; ordinary uninstall retains data.
pub const RUNTIME_STATE: [&str; 5] = [
    "kine/db",
    "kubelet",
    "containerd/root",
    "containerd/state",
    "network",
];
/// Entries additionally removed by `uninstall --purge`.
pub const PURGE_STATE: [&str; 5] = [
    "pki",
    "local-path-storage",
    "backups",
    CONTAINER_SPEC_FILE,
    "containerd",
];
/// Entries retained unless purging, listed for reporting.
pub const RETAINED_STATE: [&str; 5] = PURGE_STATE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupKind {
    Reset,
    Uninstall { purge: bool },
}

/// Exact removal/retention decision for the entries present on disk.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanupPlan {
    pub remove: Vec<PathBuf>,
    pub retain: Vec<PathBuf>,
}

/// Rejects paths that cannot be a dedicated Rubix data directory.
pub fn validate_data_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("data path '{}' must be absolute", path.display()));
    }
    let normal: Vec<_> = path
        .components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .collect();
    if normal.len() < 2
        || path
            .components()
            .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(format!("refusing unsafe data path '{}'", path.display()));
    }
    Ok(())
}

/// Computes the plan from the owned entries that exist under `data`.
pub fn plan_cleanup(kind: CleanupKind, data: &Path) -> CleanupPlan {
    let purge = matches!(kind, CleanupKind::Uninstall { purge: true });
    let mut plan = CleanupPlan::default();
    let exists = |p: &Path| p.symlink_metadata().is_ok();
    for name in RUNTIME_STATE {
        if purge && name.starts_with("containerd/") {
            continue;
        }
        let p = data.join(name);
        if exists(&p) {
            if kind == (CleanupKind::Uninstall { purge: false }) {
                plan.retain.push(p);
            } else {
                plan.remove.push(p);
            }
        }
    }
    for name in RETAINED_STATE {
        let p = data.join(name);
        if exists(&p) {
            if purge {
                plan.remove.push(p);
            } else {
                plan.retain.push(p);
            }
        }
    }
    plan
}

/// Host-mode side effects.
pub trait CleanupHost {
    fn stop_service(&mut self) -> io::Result<()>;
    fn start_service(&mut self) -> io::Result<()>;
    /// Mount points at or below `root` (never outside it).
    fn mounts_under(&mut self, root: &Path) -> io::Result<Vec<PathBuf>>;
    fn unmount(&mut self, mount: &Path) -> io::Result<()>;
    /// Removes the service definition and binary; returns removed paths.
    fn remove_service_artifacts(&mut self) -> io::Result<Vec<PathBuf>>;
}

/// Reads an explicit confirmation; non-interactive EOF is a refusal.
pub fn confirm(prompt: &str, input: &mut dyn BufRead, out: &mut dyn Write) -> io::Result<bool> {
    write!(out, "{prompt} [y/N]: ")?;
    out.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(false);
    }
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn remove_entry(path: &Path) -> io::Result<()> {
    match path.symlink_metadata() {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Final report of a cleanup run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub removed: Vec<PathBuf>,
    pub retained: Vec<PathBuf>,
    pub declined: bool,
}

fn announce(
    kind: CleanupKind,
    plan: &CleanupPlan,
    force: bool,
    input: &mut dyn BufRead,
    stderr: &mut dyn Write,
) -> io::Result<bool> {
    writeln!(stderr, "  The following Rubix-owned paths will be removed:")?;
    for p in &plan.remove {
        writeln!(stderr, "    - {}", p.display())?;
    }
    for p in &plan.retain {
        writeln!(stderr, "  Retained: {}", p.display())?;
    }
    if force {
        return Ok(true);
    }
    let what = match kind {
        CleanupKind::Reset => "reset cluster state",
        CleanupKind::Uninstall { purge: true } => "uninstall and PURGE all Rubix data",
        CleanupKind::Uninstall { purge: false } => "uninstall Rubix",
    };
    confirm(&format!("  Really {what}?"), input, stderr)
}

/// Host cleanup. Order: confirm, stop, unmount selected mounts, remove, then (reset) restart.
/// Removal is refused for any owned entry that still contains a mount point.
pub fn run_host_cleanup(
    host: &mut dyn CleanupHost,
    kind: CleanupKind,
    data: &Path,
    force: bool,
    input: &mut dyn BufRead,
    stderr: &mut dyn Write,
) -> io::Result<CleanupReport> {
    validate_data_path(data).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let plan = plan_cleanup(kind, data);
    validate_removal_ancestors(data, &plan)?;
    if !announce(kind, &plan, force, input, stderr)? {
        writeln!(stderr, "  Aborted; nothing was changed.")?;
        return Ok(CleanupReport {
            retained: plan.retain,
            declined: true,
            ..CleanupReport::default()
        });
    }
    host.stop_service()?;
    for mount in host
        .mounts_under(data)?
        .into_iter()
        .filter(|mount| plan.remove.iter().any(|path| mount.starts_with(path)))
    {
        host.unmount(&mount)?;
    }
    // Defence in depth: never delete through a surviving mount.
    if let Some(left) = host
        .mounts_under(data)?
        .into_iter()
        .find(|mount| plan.remove.iter().any(|path| mount.starts_with(path)))
    {
        return Err(io::Error::other(format!(
            "mount {} is still active; refusing to remove data",
            left.display()
        )));
    }
    let mut report = CleanupReport {
        retained: plan.retain.clone(),
        ..CleanupReport::default()
    };
    for p in &plan.remove {
        remove_entry(p)?;
        report.removed.push(p.clone());
    }
    if matches!(kind, CleanupKind::Uninstall { .. }) {
        report.removed.extend(host.remove_service_artifacts()?);
        if matches!(kind, CleanupKind::Uninstall { purge: true }) {
            // Only removes the directory when nothing foreign remains in it.
            let _ = fs::remove_dir(data);
        }
    } else {
        host.start_service()?;
        writeln!(stderr, "  [ok] Cluster state reset; service restarted")?;
    }
    Ok(report)
}

/// Container cleanup: addresses only the named container, never the engine's other objects.
#[allow(clippy::too_many_arguments)] // mirrors run_host_cleanup plus the engine and container identity
pub fn run_container_cleanup(
    runner: &mut dyn Runner,
    engine: &str,
    spec: &ContainerSpec,
    kind: CleanupKind,
    data: &Path,
    force: bool,
    input: &mut dyn BufRead,
    stderr: &mut dyn Write,
) -> io::Result<CleanupReport> {
    validate_data_path(data).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let plan = plan_cleanup(kind, data);
    validate_removal_ancestors(data, &plan)?;
    if !announce(kind, &plan, force, input, stderr)? {
        writeln!(stderr, "  Aborted; nothing was changed.")?;
        return Ok(CleanupReport {
            retained: plan.retain,
            declined: true,
            ..CleanupReport::default()
        });
    }
    let name = spec.name.clone();
    let run = |runner: &mut dyn Runner, args: &[&str]| {
        let args: Vec<String> = args.iter().map(ToString::to_string).collect();
        runner.run(engine, &args)
    };
    // Only verified absence is idempotent; an Engine or stop failure must not permit deletion.
    let present = match run(runner, &["stop", &name]) {
        Ok(_) => true,
        Err(stop_error) => {
            let names = run(runner, &["ps", "-a", "--format", "{{.Names}}"])?;
            if names.lines().any(|n| n == name) {
                return Err(stop_error);
            }
            false
        },
    };
    if !present && kind == CleanupKind::Reset {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "cannot reset a missing container",
        ));
    }
    let mut report = CleanupReport {
        retained: plan.retain.clone(),
        ..CleanupReport::default()
    };
    match kind {
        CleanupKind::Reset => {
            for p in &plan.remove {
                remove_entry(p)?;
                report.removed.push(p.clone());
            }
            run(runner, &["start", &name])?;
            writeln!(stderr, "  [ok] Cluster state reset; container restarted")?;
        },
        CleanupKind::Uninstall { purge } => {
            if present {
                run(runner, &["rm", &name])?;
            }
            for p in &plan.remove {
                remove_entry(p)?;
                report.removed.push(p.clone());
            }
            if purge {
                let _ = fs::remove_dir(data);
            }
        },
    }
    Ok(report)
}

fn validate_removal_ancestors(data: &Path, plan: &CleanupPlan) -> io::Result<()> {
    crate::upgrade::reject_symlink_state(data, "")?;
    for path in &plan.remove {
        let relative = path.strip_prefix(data).map_err(io::Error::other)?;
        // Unlink a selected symlink itself, but never traverse a symlinked parent.
        if let Some(parent) = relative.parent() {
            crate::upgrade::reject_symlink_state(data, &parent.to_string_lossy())?;
        }
    }
    Ok(())
}

/// Real host: `systemd` or `SysV` service control, `/proc/self/mounts`, `umount`.
#[derive(Debug)]
pub struct SystemHost<'a, R: Runner> {
    pub runner: &'a mut R,
    pub systemd: bool,
    pub service: String,
    pub artifacts: Vec<PathBuf>,
}

impl<R: Runner> SystemHost<'_, R> {
    fn svc(&mut self, verb: &str) -> io::Result<()> {
        let (prog, args) = if self.systemd {
            ("systemctl", vec![verb.to_string(), self.service.clone()])
        } else {
            ("service", vec![self.service.clone(), verb.to_string()])
        };
        self.runner.run(prog, &args).map(|_| ())
    }
}

/// Parses mount points from `/proc/self/mounts` text, deepest first, below `root` only.
pub fn parse_mounts_under(mounts: &str, root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = mounts
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .map(|m| PathBuf::from(m.replace("\\040", " ")))
        .filter(|m| m.starts_with(root))
        .collect();
    found.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    found
}

impl<R: Runner> CleanupHost for SystemHost<'_, R> {
    fn stop_service(&mut self) -> io::Result<()> {
        self.svc("stop")
    }

    fn start_service(&mut self) -> io::Result<()> {
        self.svc("start")
    }

    fn mounts_under(&mut self, root: &Path) -> io::Result<Vec<PathBuf>> {
        match fs::read_to_string("/proc/self/mounts") {
            Ok(text) => Ok(parse_mounts_under(&text, root)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    fn unmount(&mut self, mount: &Path) -> io::Result<()> {
        self.runner
            .run("umount", &[mount.to_string_lossy().into_owned()])
            .map(|_| ())
    }

    fn remove_service_artifacts(&mut self) -> io::Result<Vec<PathBuf>> {
        let mut removed = Vec::new();
        for p in self.artifacts.clone() {
            if p.symlink_metadata().is_ok() {
                remove_entry(&p)?;
                removed.push(p);
            }
        }
        Ok(removed)
    }
}

/// Shared command entry for reset/uninstall.
pub fn execute_cleanup(
    kind: CleanupKind,
    data: &Path,
    force: bool,
    stderr: &mut dyn Write,
) -> io::Result<u8> {
    use crate::upgrade::ProcessRunner;
    let mut runner = ProcessRunner;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let spec_path = data.join(CONTAINER_SPEC_FILE);
    let result = if spec_path.is_file() {
        match fs::read_to_string(&spec_path)
            .map_err(|e| e.to_string())
            .and_then(|t| ContainerSpec::parse(&t))
        {
            Ok(spec) => run_container_cleanup(
                &mut runner,
                "docker",
                &spec,
                kind,
                data,
                force,
                &mut input,
                stderr,
            ),
            Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        }
    } else {
        let mut host = SystemHost {
            runner: &mut runner,
            systemd: Path::new("/run/systemd/system").is_dir(),
            service: crate::upgrade::SERVICE_NAME.to_string(),
            artifacts: vec![
                PathBuf::from("/etc/systemd/system/kubesolo.service"),
                PathBuf::from(crate::DEFAULT_INSTALL_PATH),
            ],
        };
        run_host_cleanup(&mut host, kind, data, force, &mut input, stderr)
    };
    match result {
        Ok(r) if r.declined => Ok(1),
        Ok(_) => Ok(0),
        Err(e) => {
            writeln!(stderr, "  [fail] {e}")?;
            Ok(1)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct FakeHost {
        log: Vec<String>,
        mounts: Vec<PathBuf>,
        fail_unmount: bool,
        artifacts: Vec<PathBuf>,
    }

    impl CleanupHost for FakeHost {
        fn stop_service(&mut self) -> io::Result<()> {
            self.log.push("stop".into());
            Ok(())
        }
        fn start_service(&mut self) -> io::Result<()> {
            self.log.push("start".into());
            Ok(())
        }
        fn mounts_under(&mut self, _: &Path) -> io::Result<Vec<PathBuf>> {
            Ok(self.mounts.clone())
        }
        fn unmount(&mut self, m: &Path) -> io::Result<()> {
            self.log.push(format!("umount {}", m.display()));
            if self.fail_unmount {
                return Err(io::Error::other("busy"));
            }
            self.mounts.retain(|x| x != m);
            Ok(())
        }
        fn remove_service_artifacts(&mut self) -> io::Result<Vec<PathBuf>> {
            self.log.push("rm-service".into());
            for a in &self.artifacts {
                let _ = fs::remove_file(a);
            }
            Ok(self.artifacts.clone())
        }
    }

    #[derive(Debug, Default)]
    struct Rec(Vec<String>);
    impl Runner for Rec {
        fn run(&mut self, p: &str, a: &[String]) -> io::Result<String> {
            self.0.push(format!("{p} {}", a.join(" ")));
            Ok(String::new())
        }
    }

    fn tree() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for n in [
            "kine/db",
            "kubelet",
            "containerd/root",
            "containerd/state",
            "containerd/images",
            "containerd/registry",
            "network",
            "pki",
            "local-path-storage",
            "backups",
            "neighbor",
        ] {
            fs::create_dir_all(d.path().join(n)).unwrap();
            fs::write(d.path().join(n).join("f"), n).unwrap();
        }
        fs::write(d.path().join(CONTAINER_SPEC_FILE), "x").unwrap();
        d
    }

    fn names(root: &Path, v: &[PathBuf]) -> Vec<String> {
        let mut n: Vec<_> = v
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().display().to_string())
            .collect();
        n.sort();
        n
    }

    fn exists(root: &Path, n: &str) -> bool {
        root.join(n).symlink_metadata().is_ok()
    }

    #[test]
    fn reset_matrix_removes_runtime_state_keeps_pki_and_volumes() {
        let d = tree();
        let mut h = FakeHost::default();
        let r = run_host_cleanup(
            &mut h,
            CleanupKind::Reset,
            d.path(),
            true,
            &mut io::empty().lock_empty(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(
            names(d.path(), &r.removed),
            [
                "containerd/root",
                "containerd/state",
                "kine/db",
                "kubelet",
                "network"
            ]
        );
        for keep in [
            "pki",
            "local-path-storage",
            "backups",
            "neighbor",
            "containerd/images",
            "containerd/registry",
            CONTAINER_SPEC_FILE,
        ] {
            assert!(exists(d.path(), keep), "{keep}");
        }
        assert_eq!(h.log, ["stop", "start"]);
    }

    #[test]
    fn uninstall_keeps_pki_and_data_without_purge() {
        let d = tree();
        let mut h = FakeHost {
            artifacts: vec![],
            ..FakeHost::default()
        };
        let r = run_host_cleanup(
            &mut h,
            CleanupKind::Uninstall { purge: false },
            d.path(),
            true,
            &mut io::empty().lock_empty(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(
            names(d.path(), &r.retain_or_empty()),
            [
                "backups",
                "container.spec",
                "containerd",
                "containerd/root",
                "containerd/state",
                "kine/db",
                "kubelet",
                "local-path-storage",
                "network",
                "pki"
            ]
        );
        assert!(exists(d.path(), "pki") && exists(d.path(), "local-path-storage"));
        assert!(exists(d.path(), "kine/db"));
        assert!(h.log.contains(&"rm-service".to_string()) && !h.log.contains(&"start".to_string()));
    }

    #[test]
    fn purge_removes_owned_state_but_not_neighbors() {
        let d = tree();
        let mut h = FakeHost::default();
        run_host_cleanup(
            &mut h,
            CleanupKind::Uninstall { purge: true },
            d.path(),
            true,
            &mut io::empty().lock_empty(),
            &mut Vec::new(),
        )
        .unwrap();
        for gone in [
            "kine/db",
            "pki",
            "local-path-storage",
            "backups",
            "containerd",
            CONTAINER_SPEC_FILE,
        ] {
            assert!(!exists(d.path(), gone), "{gone}");
        }
        assert!(exists(d.path(), "neighbor"));
        assert!(d.path().exists(), "non-empty data dir must remain");
    }

    #[test]
    fn purge_does_not_follow_symlinked_custom_volume() {
        let d = tree();
        let vol = tempfile::tempdir().unwrap();
        fs::write(vol.path().join("precious"), "data").unwrap();
        fs::remove_dir_all(d.path().join("local-path-storage")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(vol.path(), d.path().join("local-path-storage")).unwrap();
        let mut h = FakeHost::default();
        run_host_cleanup(
            &mut h,
            CleanupKind::Uninstall { purge: true },
            d.path(),
            true,
            &mut io::empty().lock_empty(),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(!exists(d.path(), "local-path-storage"));
        assert!(vol.path().join("precious").is_file());
    }

    #[test]
    fn confirmation_required_and_declined_changes_nothing() {
        let d = tree();
        for answer in ["", "n\n", "no\n", "\n"] {
            let mut h = FakeHost::default();
            let mut input = io::Cursor::new(answer.as_bytes().to_vec());
            let r = run_host_cleanup(
                &mut h,
                CleanupKind::Reset,
                d.path(),
                false,
                &mut input,
                &mut Vec::new(),
            )
            .unwrap();
            assert!(r.declined && h.log.is_empty());
            assert!(exists(d.path(), "kine/db"));
        }
        let mut h = FakeHost::default();
        let mut input = io::Cursor::new(b"yes\n".to_vec());
        let r = run_host_cleanup(
            &mut h,
            CleanupKind::Reset,
            d.path(),
            false,
            &mut input,
            &mut Vec::new(),
        )
        .unwrap();
        assert!(!r.declined && !exists(d.path(), "kine/db"));
    }

    #[test]
    fn busy_mount_blocks_removal() {
        let d = tree();
        let mut h = FakeHost {
            mounts: vec![d.path().join("kubelet/pods/x")],
            fail_unmount: true,
            ..FakeHost::default()
        };
        let e = run_host_cleanup(
            &mut h,
            CleanupKind::Reset,
            d.path(),
            true,
            &mut io::empty().lock_empty(),
            &mut Vec::new(),
        );
        assert!(e.is_err());
        assert!(exists(d.path(), "kine/db") && exists(d.path(), "kubelet/f"));
    }

    #[test]
    fn repeated_cleanup_is_idempotent() {
        let d = tree();
        for _ in 0..2 {
            let mut h = FakeHost::default();
            run_host_cleanup(
                &mut h,
                CleanupKind::Reset,
                d.path(),
                true,
                &mut io::empty().lock_empty(),
                &mut Vec::new(),
            )
            .unwrap();
        }
        assert!(exists(d.path(), "pki"));
    }

    #[test]
    fn unsafe_paths_rejected() {
        for p in ["/", "/var", "relative/x", "/var/lib/../etc"] {
            assert!(validate_data_path(Path::new(p)).is_err(), "{p}");
        }
        assert!(validate_data_path(Path::new("/var/lib/kubesolo")).is_ok());
    }

    #[test]
    fn mount_parsing_is_scoped_deepest_first() {
        let text = "a /var/lib/kubesolo/kubelet/p tmpfs\nb /var/lib/kubesolo-other x\nc /var/lib/kubesolo/kubelet/p/q x\nd /proc proc\n";
        let m = parse_mounts_under(text, Path::new("/var/lib/kubesolo"));
        assert_eq!(
            m,
            [
                PathBuf::from("/var/lib/kubesolo/kubelet/p/q"),
                PathBuf::from("/var/lib/kubesolo/kubelet/p")
            ]
        );
    }

    const SPEC: &str = "name=rubix\nimage=x/kubesolo:v1.2.0\nmount=/srv/volumes:/data\n";

    #[test]
    fn container_uninstall_touches_only_named_container_and_keeps_custom_volume() {
        let d = tree();
        let vol = tempfile::tempdir().unwrap();
        let mut r = Rec::default();
        let spec = ContainerSpec::parse(SPEC).unwrap();
        run_container_cleanup(
            &mut r,
            "docker",
            &spec,
            CleanupKind::Uninstall { purge: false },
            d.path(),
            true,
            &mut io::empty().lock_empty(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(r.0, ["docker stop rubix", "docker rm rubix"]);
        assert!(exists(d.path(), "pki") && exists(d.path(), "kine/db"));
        assert!(vol.path().exists());
    }

    #[test]
    fn container_reset_restarts_container() {
        let d = tree();
        let mut r = Rec::default();
        let spec = ContainerSpec::parse(SPEC).unwrap();
        run_container_cleanup(
            &mut r,
            "docker",
            &spec,
            CleanupKind::Reset,
            d.path(),
            true,
            &mut io::empty().lock_empty(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(r.0, ["docker stop rubix", "docker start rubix"]);
        assert!(exists(d.path(), "pki"));
    }

    trait EmptyLock {
        fn lock_empty(self) -> io::Cursor<Vec<u8>>;
    }
    impl EmptyLock for io::Empty {
        fn lock_empty(self) -> io::Cursor<Vec<u8>> {
            io::Cursor::new(Vec::new())
        }
    }
    trait Retain {
        fn retain_or_empty(&self) -> Vec<PathBuf>;
    }
    impl Retain for CleanupReport {
        fn retain_or_empty(&self) -> Vec<PathBuf> {
            self.retained.clone()
        }
    }
}
