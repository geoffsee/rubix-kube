//! Version-aware host and container upgrades (`rubixctl upgrade`).
//!
//! The engine in [`run_upgrade`] sequences prepare → backup → stop → replace → start and rolls
//! back automatically when replace or start fails. Backends own artifact-specific effects:
//!
//! - [`HostBackend`] replaces only the configured Rubix binary and its service.
//! - [`ContainerBackend`] replaces only the named container, preserving its image repository,
//!   arguments, ports, MTU, PKI and registry mounts from the recorded container spec.
//!
//! External runtimes, the running installer and neighbouring installations are never touched:
//! container names are validated and all commands address exactly one named unit.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use rubix_config::semver::{compare_versions, parse_version, supports_config_file};

/// Directory under the data path holding pre-upgrade backups.
pub const BACKUP_DIR: &str = "backups";
/// Container spec file recorded at install time under the data path.
pub const CONTAINER_SPEC_FILE: &str = "container.spec";
/// State directories (relative to the data path) captured before an upgrade.
pub const BACKED_UP_STATE_DIRS: [&str; 3] = ["pki", "db", "kine"];
/// Default service unit name.
pub const SERVICE_NAME: &str = "kubesolo";

/// Direction of a requested version transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Same,
    Upgrade,
    Downgrade,
}

/// Classifies `from` → `to`; versions that do not parse are rejected.
pub fn classify_transition(from: &str, to: &str) -> Result<Transition, String> {
    for v in [from, to] {
        if parse_version(v).is_none() {
            return Err(format!(
                "unsupported version '{v}': expected vMAJOR.MINOR.PATCH"
            ));
        }
    }
    Ok(match compare_versions(from, to) {
        Some(std::cmp::Ordering::Less) => Transition::Upgrade,
        Some(std::cmp::Ordering::Greater) => Transition::Downgrade,
        _ => Transition::Same,
    })
}

/// Executes external programs; abstracted so lifecycle ordering is testable.
pub trait Runner: std::fmt::Debug {
    fn run(&mut self, program: &str, args: &[String]) -> io::Result<String>;
}

/// Runs real processes.
#[derive(Debug, Default)]
pub struct ProcessRunner;

impl Runner for ProcessRunner {
    fn run(&mut self, program: &str, args: &[String]) -> io::Result<String> {
        let out = std::process::Command::new(program).args(args).output()?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(io::Error::other(format!(
                "{program} {} failed: {}",
                args.first().map_or("", String::as_str),
                String::from_utf8_lossy(&out.stderr).trim()
            )))
        }
    }
}

/// Artifact-specific steps of a transition.
pub trait TransitionBackend {
    fn current_version(&mut self) -> io::Result<String>;
    /// Fetch/verify the new artifact without mutating the running installation.
    fn prepare(&mut self, target: &str) -> io::Result<()>;
    /// Record artifact-specific rollback material in `dir`.
    fn snapshot(&mut self, dir: &Path) -> io::Result<()>;
    fn stop(&mut self) -> io::Result<()>;
    fn replace(&mut self, target: &str) -> io::Result<()>;
    fn start(&mut self) -> io::Result<()>;
    /// Restore the pre-upgrade artifact from `dir` (does not start it).
    fn restore(&mut self, dir: &Path) -> io::Result<()>;
    /// Drop temporary rollback material after a successful start.
    fn commit(&mut self) -> io::Result<()> {
        Ok(())
    }
    /// Config-file migration hook run after replacement and before start.
    fn migrate_config(&mut self, _target: &str, _stderr: &mut dyn Write) -> io::Result<bool> {
        Ok(false)
    }
}

/// Outcome of a completed run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpgradeOutcome {
    Unchanged,
    Upgraded { backup: PathBuf },
    RolledBack { backup: PathBuf, cause: String },
}

fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &to)?;
        }
        // Symlinks are not followed so the backup cannot escape the data path.
    }
    Ok(())
}

/// Copies PKI/datastore state and the config file into a fresh backup directory.
pub fn backup_state(
    data_path: &Path,
    config: Option<&Path>,
    from: &str,
    stamp: u64,
) -> io::Result<PathBuf> {
    let dir = data_path
        .join(BACKUP_DIR)
        .join(format!("pre-upgrade-{from}-{stamp}"));
    fs::create_dir_all(&dir)?;
    for name in BACKED_UP_STATE_DIRS {
        let src = data_path.join(name);
        if src.is_dir() {
            copy_tree(&src, &dir.join(name))?;
        }
    }
    if let Some(cfg) = config.filter(|c| c.is_file()) {
        fs::copy(cfg, dir.join("config.yaml"))?;
    }
    Ok(dir)
}

/// Runs a transition; failures after the service is stopped restore and restart the old unit.
pub fn run_upgrade(
    backend: &mut dyn TransitionBackend,
    data_path: &Path,
    config: Option<&Path>,
    target: &str,
    stamp: u64,
    stderr: &mut dyn Write,
) -> io::Result<UpgradeOutcome> {
    let from = backend.current_version()?;
    match classify_transition(&from, target).map_err(io::Error::other)? {
        Transition::Same => {
            writeln!(stderr, "  [ok] Already at {target}; nothing to do")?;
            return Ok(UpgradeOutcome::Unchanged);
        },
        Transition::Downgrade => writeln!(
            stderr,
            "  [warn] Downgrading {from} -> {target}; datastore compatibility is not verified"
        )?,
        Transition::Upgrade => writeln!(stderr, "  > Upgrading {from} -> {target}")?,
    }

    // Stage 1: fetch with the old version still running; failure changes nothing.
    backend
        .prepare(target)
        .map_err(|e| io::Error::new(e.kind(), format!("prepare failed: {e}")))?;
    // Stage 2: backup before any mutation.
    let backup = backup_state(data_path, config, &from, stamp)?;
    backend.snapshot(&backup)?;
    writeln!(stderr, "  [ok] State backed up to {}", backup.display())?;

    // Stage 3: stop/replace/start with rollback.
    if let Err(cause) = backend.stop() {
        // Nothing replaced; ensure the old unit is running again.
        let _ = backend.start();
        return Err(io::Error::new(
            cause.kind(),
            format!("stop failed: {cause}"),
        ));
    }
    let attempt = backend
        .replace(target)
        .and_then(|()| {
            backend.migrate_config(target, stderr)?;
            Ok(())
        })
        .and_then(|()| backend.start());
    match attempt {
        Ok(()) => {
            backend.commit()?;
            writeln!(stderr, "  [ok] Upgraded to {target}")?;
            if supports_config_file(target) {
                writeln!(
                    stderr,
                    "  Restart required for configuration changes to take effect."
                )?;
            }
            Ok(UpgradeOutcome::Upgraded { backup })
        },
        Err(cause) => {
            writeln!(stderr, "  [fail] {cause}; rolling back to {from}")?;
            backend
                .restore(&backup)
                .and_then(|()| backend.start())
                .map_err(|e| {
                    io::Error::other(format!(
                        "upgrade failed ({cause}) and rollback failed ({e}); backup kept at {}",
                        backup.display()
                    ))
                })?;
            writeln!(stderr, "  [ok] Rolled back to {from}")?;
            Ok(UpgradeOutcome::RolledBack {
                backup,
                cause: cause.to_string(),
            })
        },
    }
}

/// Host-mode backend: one binary and one service unit.
#[derive(Debug)]
pub struct HostBackend<'a, R: Runner> {
    pub binary: PathBuf,
    pub staged: PathBuf,
    pub service: String,
    pub runner: &'a mut R,
    pub systemd: bool,
    pub service_file: Option<PathBuf>,
    pub legacy_config: Option<PathBuf>,
}

impl<R: Runner> HostBackend<'_, R> {
    fn svc(&mut self, verb: &str) -> io::Result<()> {
        let (prog, args) = if self.systemd {
            ("systemctl", vec![verb.to_string(), self.service.clone()])
        } else {
            ("service", vec![self.service.clone(), verb.to_string()])
        };
        self.runner.run(prog, &args).map(|_| ())
    }
}

impl<R: Runner> TransitionBackend for HostBackend<'_, R> {
    fn current_version(&mut self) -> io::Result<String> {
        let out = self
            .runner
            .run(&self.binary.to_string_lossy(), &["--version".to_string()])?;
        out.split_whitespace()
            .find(|w| parse_version(w).is_some())
            .map(str::to_string)
            .ok_or_else(|| io::Error::other("cannot determine installed version"))
    }

    fn prepare(&mut self, _target: &str) -> io::Result<()> {
        if self.staged.is_file() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "staged binary missing",
            ))
        }
    }

    fn snapshot(&mut self, dir: &Path) -> io::Result<()> {
        fs::copy(&self.binary, dir.join("kubesolo.bin"))?;
        if let Some(unit) = self.service_file.as_ref().filter(|p| p.is_file()) {
            fs::copy(unit, dir.join("service.unit"))?;
        }
        Ok(())
    }

    fn stop(&mut self) -> io::Result<()> {
        self.svc("stop")
    }

    fn replace(&mut self, _target: &str) -> io::Result<()> {
        // Same-directory temp + rename: a crash leaves either the old or the new binary.
        let tmp = self.binary.with_extension("upgrade-new");
        fs::copy(&self.staged, &tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
        }
        fs::rename(&tmp, &self.binary)
    }

    fn start(&mut self) -> io::Result<()> {
        self.svc("start")
    }

    fn restore(&mut self, dir: &Path) -> io::Result<()> {
        let tmp = self.binary.with_extension("upgrade-restore");
        fs::copy(dir.join("kubesolo.bin"), &tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
        }
        fs::rename(&tmp, &self.binary)?;
        if let (Some(unit), true) = (&self.service_file, dir.join("service.unit").is_file()) {
            fs::copy(dir.join("service.unit"), unit)?;
        }
        Ok(())
    }

    fn migrate_config(&mut self, target: &str, stderr: &mut dyn Write) -> io::Result<bool> {
        let Some(unit) = self.service_file.clone().filter(|p| p.is_file()) else {
            return Ok(false);
        };
        let result = crate::migrate::migrate_legacy_service(
            &unit,
            self.legacy_config.as_deref(),
            target,
            None,
            stderr,
        )?;
        Ok(matches!(
            result,
            crate::ServiceMigrationResult::Migrated { .. }
        ))
    }
}

/// Recorded container deployment; everything except the image tag is preserved on upgrade.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContainerSpec {
    pub name: String,
    pub image: String,
    pub args: Vec<String>,
    pub ports: Vec<String>,
    pub mounts: Vec<String>,
    pub env: Vec<String>,
    pub mtu: Option<String>,
}

fn valid_container_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

impl ContainerSpec {
    /// Parses `key=value` lines (`name`, `image`, `arg`, `port`, `mount`, `env`, `mtu`).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut spec = Self::default();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (k, v) = line
                .split_once('=')
                .ok_or_else(|| format!("line {}: expected key=value", n + 1))?;
            match k {
                "name" => spec.name = v.to_string(),
                "image" => spec.image = v.to_string(),
                "arg" => spec.args.push(v.to_string()),
                "port" => spec.ports.push(v.to_string()),
                "mount" => spec.mounts.push(v.to_string()),
                "env" => spec.env.push(v.to_string()),
                "mtu" => spec.mtu = Some(v.to_string()),
                other => return Err(format!("line {}: unknown key '{other}'", n + 1)),
            }
        }
        if !valid_container_name(&spec.name) {
            return Err(format!("invalid container name '{}'", spec.name));
        }
        if spec.image.is_empty() || spec.image.starts_with('-') {
            return Err("invalid or missing image".to_string());
        }
        Ok(spec)
    }

    /// Image repository without tag or digest.
    pub fn repository(&self) -> &str {
        let no_digest = self.image.split('@').next().unwrap_or(&self.image);
        match no_digest.rfind(':') {
            Some(i) if !no_digest[i..].contains('/') => &no_digest[..i],
            _ => no_digest,
        }
    }

    pub fn tag(&self) -> Option<&str> {
        let no_digest = self.image.split('@').next().unwrap_or(&self.image);
        no_digest
            .rfind(':')
            .filter(|&i| !no_digest[i..].contains('/'))
            .map(|i| &no_digest[i + 1..])
    }

    /// Spec with the image tag replaced and every other setting preserved.
    #[must_use]
    pub fn with_version(&self, version: &str) -> Self {
        Self {
            image: format!("{}:{version}", self.repository()),
            ..self.clone()
        }
    }

    pub fn run_args(&self, name: &str) -> Vec<String> {
        let mut a: Vec<String> = vec!["run".into(), "-d".into(), "--name".into(), name.into()];
        for p in &self.ports {
            a.extend(["-p".into(), p.clone()]);
        }
        for m in &self.mounts {
            a.extend(["-v".into(), m.clone()]);
        }
        for e in &self.env {
            a.extend(["-e".into(), e.clone()]);
        }
        if let Some(mtu) = &self.mtu {
            a.extend(["-e".into(), format!("KUBESOLO_MTU={mtu}")]);
        }
        a.push(self.image.clone());
        a.extend(self.args.iter().cloned());
        a
    }

    pub fn render(&self) -> String {
        let mut m = BTreeMap::new();
        m.insert("name", vec![self.name.clone()]);
        m.insert("image", vec![self.image.clone()]);
        let mut lines: Vec<String> = Vec::new();
        for (k, vs) in m {
            for v in vs {
                lines.push(format!("{k}={v}"));
            }
        }
        for (k, vs) in [
            ("arg", &self.args),
            ("port", &self.ports),
            ("mount", &self.mounts),
            ("env", &self.env),
        ] {
            for v in vs {
                lines.push(format!("{k}={v}"));
            }
        }
        if let Some(mtu) = &self.mtu {
            lines.push(format!("mtu={mtu}"));
        }
        lines.push(String::new());
        lines.join("\n")
    }
}

/// Container-mode backend: replaces exactly the named container.
#[derive(Debug)]
pub struct ContainerBackend<'a, R: Runner> {
    pub engine: String,
    pub spec: ContainerSpec,
    pub runner: &'a mut R,
    replaced: bool,
}

impl<'a, R: Runner> ContainerBackend<'a, R> {
    pub fn new(engine: &str, spec: ContainerSpec, runner: &'a mut R) -> Self {
        Self {
            engine: engine.to_string(),
            spec,
            runner,
            replaced: false,
        }
    }

    fn old_name(&self) -> String {
        format!("{}-pre-upgrade", self.spec.name)
    }

    fn eng(&mut self, args: &[&str]) -> io::Result<String> {
        let args: Vec<String> = args.iter().map(ToString::to_string).collect();
        let engine = self.engine.clone();
        self.runner.run(&engine, &args)
    }
}

impl<R: Runner> TransitionBackend for ContainerBackend<'_, R> {
    fn current_version(&mut self) -> io::Result<String> {
        self.spec
            .tag()
            .map(str::to_string)
            .ok_or_else(|| io::Error::other("container image has no version tag"))
    }

    fn prepare(&mut self, target: &str) -> io::Result<()> {
        let image = self.spec.with_version(target).image;
        self.eng(&["pull", &image]).map(|_| ())
    }

    fn snapshot(&mut self, dir: &Path) -> io::Result<()> {
        fs::write(dir.join(CONTAINER_SPEC_FILE), self.spec.render())
    }

    fn stop(&mut self) -> io::Result<()> {
        let name = self.spec.name.clone();
        self.eng(&["stop", &name]).map(|_| ())
    }

    fn replace(&mut self, target: &str) -> io::Result<()> {
        let (name, old) = (self.spec.name.clone(), self.old_name());
        self.eng(&["rename", &name, &old])?;
        self.replaced = true;
        let args = self.spec.with_version(target).run_args(&name);
        let engine = self.engine.clone();
        self.runner.run(&engine, &args).map(|_| ())
    }

    fn start(&mut self) -> io::Result<()> {
        // After replace the new container is already running; before it, restart the old one.
        if self.replaced {
            Ok(())
        } else {
            let name = self.spec.name.clone();
            self.eng(&["start", &name]).map(|_| ())
        }
    }

    fn restore(&mut self, _dir: &Path) -> io::Result<()> {
        let (name, old) = (self.spec.name.clone(), self.old_name());
        if self.replaced {
            let _ = self.eng(&["rm", "-f", &name]);
            self.eng(&["rename", &old, &name])?;
            self.replaced = false;
        }
        Ok(())
    }

    fn commit(&mut self) -> io::Result<()> {
        let old = self.old_name();
        self.eng(&["rm", &old]).map(|_| ())
    }
}

/// Command entry point: chooses container mode when a recorded spec exists under the data path.
pub fn execute_upgrade(
    options: &crate::UpgradeOptions,
    inputs: &mut dyn crate::CheckInputs,
    _stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<u8> {
    use std::time::{SystemTime, UNIX_EPOCH};
    writeln!(stderr, "\n  rubixctl  upgrade\n")?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut runner = ProcessRunner;
    let spec_path = options.path.join(CONTAINER_SPEC_FILE);
    let result = if spec_path.is_file() {
        let spec = fs::read_to_string(&spec_path)
            .map_err(|e| e.to_string())
            .and_then(|t| ContainerSpec::parse(&t));
        match spec {
            Ok(spec) => {
                let mut backend = ContainerBackend::new("docker", spec, &mut runner);
                run_upgrade(
                    &mut backend,
                    &options.path,
                    None,
                    &options.version,
                    stamp,
                    stderr,
                )
            },
            Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        }
    } else {
        let stage = tempfile::tempdir()?;
        let staged = match stage_host_artifact(options, inputs, stage.path()) {
            Ok(p) => p,
            Err(e) => {
                writeln!(stderr, "  [fail] staging artifact: {e}")?;
                return Ok(1);
            },
        };
        let mut backend = HostBackend {
            binary: PathBuf::from(crate::DEFAULT_INSTALL_PATH),
            staged,
            service: SERVICE_NAME.to_string(),
            runner: &mut runner,
            systemd: Path::new("/run/systemd/system").is_dir(),
            service_file: Some(PathBuf::from("/etc/systemd/system/kubesolo.service")),
            legacy_config: None,
        };
        run_upgrade(
            &mut backend,
            &options.path,
            Some(Path::new(rubix_config::DEFAULT_CONFIG_PATH)),
            &options.version,
            stamp,
            stderr,
        )
    };
    match result {
        Ok(UpgradeOutcome::RolledBack { cause, .. }) => {
            writeln!(stderr, "  upgrade failed and was rolled back: {cause}")?;
            Ok(1)
        },
        Ok(_) => Ok(0),
        Err(e) => {
            writeln!(stderr, "  [fail] {e}")?;
            Ok(1)
        },
    }
}

fn stage_host_artifact(
    options: &crate::UpgradeOptions,
    inputs: &mut dyn crate::CheckInputs,
    stage: &Path,
) -> io::Result<PathBuf> {
    let source = if let Some(path) = &options.offline_install {
        path.clone()
    } else {
        let evidence = inputs.discover().map_err(io::Error::other)?;
        let target =
            crate::resolve_target(None, None, Some(&evidence)).map_err(io::Error::other)?;
        let name = crate::artifact_archive_name(&options.version, target, false);
        let url =
            crate::artifact_download_url(&options.version, &name, options.custom_url.as_deref());
        let dest = stage.join(&name);
        inputs.download_file(&url, &dest, options.proxy.as_deref(), None)?;
        dest
    };
    let is_archive = source
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| {
            n.to_ascii_lowercase().ends_with(".tar.gz") || n.to_ascii_lowercase().ends_with(".tgz")
        });
    if !is_archive {
        return Ok(source);
    }
    let out = stage.join("extract");
    fs::create_dir_all(&out)?;
    ProcessRunner.run(
        "tar",
        &[
            "-xzf".into(),
            source.to_string_lossy().into_owned(),
            "-C".into(),
            out.to_string_lossy().into_owned(),
        ],
    )?;
    let bin = out.join("kubesolo");
    if bin.is_file() {
        Ok(bin)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "archive does not contain 'kubesolo'",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct Fake {
        log: Vec<String>,
        fail_on: Option<String>,
        version: String,
    }

    impl Runner for Fake {
        fn run(&mut self, program: &str, args: &[String]) -> io::Result<String> {
            let line = format!("{program} {}", args.join(" "));
            self.log.push(line.clone());
            if self
                .fail_on
                .as_ref()
                .is_some_and(|f| line.contains(f.as_str()))
            {
                return Err(io::Error::other(format!("injected: {line}")));
            }
            Ok(self.version.clone())
        }
    }

    #[derive(Debug)]
    struct FailFirstStart(Fake, bool);

    impl Runner for FailFirstStart {
        fn run(&mut self, p: &str, a: &[String]) -> io::Result<String> {
            if a.first().is_some_and(|v| v == "start") && !self.1 {
                self.1 = true;
                self.0.log.push("start-failed".into());
                return Err(io::Error::other("boom"));
            }
            self.0.run(p, a)
        }
    }

    fn data_dir() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("pki")).unwrap();
        fs::write(d.path().join("pki/ca.crt"), "CA").unwrap();
        fs::create_dir_all(d.path().join("db")).unwrap();
        fs::write(d.path().join("db/state.db"), "DB").unwrap();
        d
    }

    fn host<'a>(d: &Path, runner: &'a mut Fake) -> HostBackend<'a, Fake> {
        let bin = d.join("kubesolo");
        fs::write(&bin, "old").unwrap();
        let staged = d.join("staged");
        fs::write(&staged, "new").unwrap();
        HostBackend {
            binary: bin,
            staged,
            service: SERVICE_NAME.into(),
            runner,
            systemd: true,
            service_file: None,
            legacy_config: None,
        }
    }

    #[test]
    fn transition_classification() {
        assert_eq!(
            classify_transition("v1.1.8", "v1.2.0"),
            Ok(Transition::Upgrade)
        );
        assert_eq!(
            classify_transition("v1.2.0", "v1.1.8"),
            Ok(Transition::Downgrade)
        );
        assert_eq!(classify_transition("v1.2.0", "1.2.0"), Ok(Transition::Same));
        assert!(classify_transition("v1.2.0", "latest").is_err());
    }

    #[test]
    fn host_upgrade_replaces_binary_and_backs_up_state() {
        let d = data_dir();
        let bins = tempfile::tempdir().unwrap();
        let mut r = Fake {
            version: "kubesolo v1.1.8".into(),
            ..Fake::default()
        };
        let mut b = host(bins.path(), &mut r);
        let mut err = Vec::new();
        let out = run_upgrade(&mut b, d.path(), None, "v1.3.0", 7, &mut err).unwrap();
        let UpgradeOutcome::Upgraded { backup } = out else {
            panic!("{out:?}")
        };
        assert_eq!(
            fs::read_to_string(bins.path().join("kubesolo")).unwrap(),
            "new"
        );
        assert_eq!(fs::read_to_string(backup.join("pki/ca.crt")).unwrap(), "CA");
        assert_eq!(
            fs::read_to_string(backup.join("db/state.db")).unwrap(),
            "DB"
        );
        assert_eq!(
            fs::read_to_string(backup.join("kubesolo.bin")).unwrap(),
            "old"
        );
        let log = r.log.join("|");
        assert!(
            log.contains("systemctl stop kubesolo|systemctl start kubesolo"),
            "{log}"
        );
        assert!(String::from_utf8(err).unwrap().contains("Restart required"));
    }

    #[test]
    fn host_start_failure_rolls_back() {
        let d = data_dir();
        let bins = tempfile::tempdir().unwrap();
        let mut r = Fake {
            version: "v1.1.8".into(),
            ..Fake::default()
        };
        r.version = "v1.1.8".into();
        let mut runner = FailFirstStart(r, false);
        let bin = bins.path().join("kubesolo");
        fs::write(&bin, "old").unwrap();
        let staged = bins.path().join("staged");
        fs::write(&staged, "new").unwrap();
        let mut b = HostBackend {
            binary: bin.clone(),
            staged,
            service: SERVICE_NAME.into(),
            runner: &mut runner,
            systemd: true,
            service_file: None,
            legacy_config: None,
        };
        let out = run_upgrade(&mut b, d.path(), None, "v1.2.0", 1, &mut Vec::new()).unwrap();
        assert!(matches!(out, UpgradeOutcome::RolledBack { .. }), "{out:?}");
        assert_eq!(fs::read_to_string(bin).unwrap(), "old");
        assert_eq!(
            runner
                .0
                .log
                .iter()
                .filter(|l| l.contains("start kubesolo"))
                .count(),
            1
        );
    }

    #[test]
    fn prepare_failure_changes_nothing() {
        let d = data_dir();
        let bins = tempfile::tempdir().unwrap();
        let mut r = Fake {
            version: "v1.1.8".into(),
            ..Fake::default()
        };
        let mut b = host(bins.path(), &mut r);
        fs::remove_file(&b.staged).unwrap();
        assert!(run_upgrade(&mut b, d.path(), None, "v1.2.0", 1, &mut Vec::new()).is_err());
        assert!(!d.path().join(BACKUP_DIR).exists());
        assert!(!r.log.iter().any(|l| l.contains("stop")));
    }

    #[test]
    fn same_version_is_noop() {
        let d = data_dir();
        let bins = tempfile::tempdir().unwrap();
        let mut r = Fake {
            version: "v1.2.0".into(),
            ..Fake::default()
        };
        let mut b = host(bins.path(), &mut r);
        let out = run_upgrade(&mut b, d.path(), None, "v1.2.0", 1, &mut Vec::new()).unwrap();
        assert_eq!(out, UpgradeOutcome::Unchanged);
    }

    const SPEC: &str = "name=rubix\nimage=ghcr.io/example/kubesolo:v1.1.8\narg=--debug\nport=6443:6443\nport=9090:9090\nmount=/var/lib/kubesolo/pki:/var/lib/kubesolo/pki\nmount=/etc/kubesolo/registries.yaml:/etc/kubesolo/registries.yaml:ro\nmtu=1400\n";

    #[test]
    fn container_spec_roundtrip_and_preservation() {
        let spec = ContainerSpec::parse(SPEC).unwrap();
        assert_eq!(spec.repository(), "ghcr.io/example/kubesolo");
        assert_eq!(spec.tag(), Some("v1.1.8"));
        assert_eq!(ContainerSpec::parse(&spec.render()).unwrap(), spec);
        let next = spec.with_version("v1.2.0");
        assert_eq!(next.image, "ghcr.io/example/kubesolo:v1.2.0");
        let args = next.run_args("rubix").join(" ");
        for needle in [
            "-p 6443:6443",
            "-p 9090:9090",
            "pki:/var/lib/kubesolo/pki",
            "registries.yaml",
            "KUBESOLO_MTU=1400",
            "--debug",
        ] {
            assert!(args.contains(needle), "{needle} missing in {args}");
        }
    }

    #[test]
    fn container_spec_rejects_unsafe_names() {
        assert!(ContainerSpec::parse("name=--rm\nimage=x:v1.0.0\n").is_err());
        assert!(ContainerSpec::parse("name=a b\nimage=x:v1.0.0\n").is_err());
        assert!(ContainerSpec::parse("name=a\nimage=-x\n").is_err());
        assert!(ContainerSpec::parse("name=a\nimage=x\nbogus=1\n").is_err());
    }

    #[test]
    fn container_upgrade_touches_only_named_container() {
        let d = data_dir();
        let mut r = Fake::default();
        let spec = ContainerSpec::parse(SPEC).unwrap();
        let mut b = ContainerBackend::new("docker", spec, &mut r);
        let out = run_upgrade(&mut b, d.path(), None, "v1.2.0", 3, &mut Vec::new()).unwrap();
        assert!(matches!(out, UpgradeOutcome::Upgraded { .. }));
        let log = r.log.join("\n");
        assert!(
            log.starts_with("docker pull ghcr.io/example/kubesolo:v1.2.0"),
            "{log}"
        );
        assert!(log.contains("docker stop rubix"));
        assert!(log.contains("docker rename rubix rubix-pre-upgrade"));
        assert!(log.contains("docker rm rubix-pre-upgrade"));
        assert!(!log.contains("rm -f"));
        assert!(
            r.log
                .iter()
                .all(|l| !l.contains("prune") && !l.contains(" kill"))
        );
    }

    #[test]
    fn container_run_failure_restores_old_container() {
        let d = data_dir();
        let mut r = Fake {
            fail_on: Some("run -d".into()),
            ..Fake::default()
        };
        let spec = ContainerSpec::parse(SPEC).unwrap();
        let mut b = ContainerBackend::new("docker", spec, &mut r);
        let out = run_upgrade(&mut b, d.path(), None, "v1.2.0", 3, &mut Vec::new()).unwrap();
        let UpgradeOutcome::RolledBack { backup, .. } = out else {
            panic!()
        };
        assert!(backup.join(CONTAINER_SPEC_FILE).is_file());
        let log = r.log.join("\n");
        assert!(
            log.contains("docker rm -f rubix\ndocker rename rubix-pre-upgrade rubix"),
            "{log}"
        );
    }

    #[test]
    fn container_pull_failure_leaves_container_running() {
        let d = data_dir();
        let mut r = Fake {
            fail_on: Some("pull".into()),
            ..Fake::default()
        };
        let spec = ContainerSpec::parse(SPEC).unwrap();
        let mut b = ContainerBackend::new("docker", spec, &mut r);
        assert!(run_upgrade(&mut b, d.path(), None, "v1.2.0", 3, &mut Vec::new()).is_err());
        assert!(r.log.iter().all(|l| !l.contains("stop")));
    }
}
