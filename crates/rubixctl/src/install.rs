use crate::CheckInputs;
use crate::artifact::resolve_target;
use crate::contract::InstallOptions;
use rubix_assets::{Architecture, ArtifactNaming};
use rubix_platform::Libc;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// Name of the metadata file every offline bundle carries at its root.
pub const BUNDLE_MANIFEST: &str = "bundle.manifest";

/// One verified bundle entry: a relative path with its expected digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleEntry {
    pub sha256: String,
    pub path: PathBuf,
    pub executable: bool,
}

/// Parsed bundle metadata. Format (one record per line, `#` comments allowed):
///
/// ```text
/// version=v1.1.8
/// os=linux
/// arch=arm64
/// libc=glibc
/// file <sha256> <relative-path>
/// exec <sha256> <relative-path>
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleManifest {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub libc: String,
    pub entries: Vec<BundleEntry>,
}

pub(crate) fn safe_relative(path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    if path.is_empty() || p.is_absolute() {
        return Err(format!("unsafe bundle path '{path}'"));
    }
    for c in p.components() {
        if !matches!(c, Component::Normal(_)) {
            return Err(format!("unsafe bundle path '{path}'"));
        }
    }
    Ok(p.to_path_buf())
}

impl BundleManifest {
    pub fn parse(text: &str) -> Result<Self, String> {
        let (mut version, mut os, mut arch, mut libc) = (None, None, None, None);
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();
        for line in text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            if let Some((k, v)) = line.split_once('=') {
                let slot = match k {
                    "version" => &mut version,
                    "os" => &mut os,
                    "arch" => &mut arch,
                    "libc" => &mut libc,
                    other => return Err(format!("unknown manifest key '{other}'")),
                };
                *slot = Some(v.to_string());
            } else {
                let mut parts = line.splitn(3, ' ');
                let (role, digest, path) = (parts.next(), parts.next(), parts.next());
                let (Some(role), Some(digest), Some(path)) = (role, digest, path) else {
                    return Err(format!("malformed manifest line '{line}'"));
                };
                let executable = match role {
                    "file" => false,
                    "exec" => true,
                    other => return Err(format!("unknown manifest record '{other}'")),
                };
                if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(format!("invalid sha256 for '{path}'"));
                }
                let rel = safe_relative(path)?;
                if !seen.insert(rel.clone()) {
                    return Err(format!("duplicate manifest entry '{path}'"));
                }
                entries.push(BundleEntry {
                    sha256: digest.to_ascii_lowercase(),
                    path: rel,
                    executable,
                });
            }
        }
        let missing = |n: &str| format!("manifest missing '{n}'");
        Ok(Self {
            version: version.ok_or_else(|| missing("version"))?,
            os: os.ok_or_else(|| missing("os"))?,
            arch: arch.ok_or_else(|| missing("arch"))?,
            libc: libc.ok_or_else(|| missing("libc"))?,
            entries,
        })
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "version={}\nos={}\narch={}\nlibc={}\n",
            self.version, self.os, self.arch, self.libc
        );
        for e in &self.entries {
            let role = if e.executable { "exec" } else { "file" };
            let _ = std::fmt::Write::write_fmt(
                &mut out,
                format_args!("{role} {} {}\n", e.sha256, e.path.display()),
            );
        }
        out
    }
}

pub(crate) fn arch_name(a: Architecture) -> &'static str {
    match a {
        Architecture::Amd64 => "amd64",
        Architecture::Arm64 => "arm64",
        Architecture::ArmV7 => "arm",
        Architecture::Riscv64 => "riscv64",
    }
}

pub(crate) fn libc_name(l: Libc) -> &'static str {
    match l {
        Libc::Glibc => "glibc",
        Libc::Musl => "musl",
    }
}

fn elf_machine(a: Architecture) -> u16 {
    match a {
        Architecture::Amd64 => 62,
        Architecture::Arm64 => 183,
        Architecture::ArmV7 => 40,
        Architecture::Riscv64 => 243,
    }
}

/// Checks the ELF identity of an executable against the target architecture.
pub(crate) fn check_elf(path: &Path, arch: Architecture) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() < 20 || &bytes[..4] != b"\x7fELF" {
        return Err(format!("{} is not an ELF executable", path.display()));
    }
    let raw = [bytes[18], bytes[19]];
    let machine = if bytes[5] == 2 {
        u16::from_be_bytes(raw)
    } else {
        u16::from_le_bytes(raw)
    };
    if machine != elf_machine(arch) {
        return Err(format!(
            "{} is built for machine {machine}, expected {}",
            path.display(),
            elf_machine(arch)
        ));
    }
    Ok(())
}

pub(crate) fn sha256_hex(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = io::Read::read(&mut file, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let mut hex = String::new();
    for b in hasher.finalize() {
        let _ = std::fmt::Write::write_fmt(&mut hex, format_args!("{b:02x}"));
    }
    Ok(hex)
}

/// Lists archive members and rejects traversal, absolute paths and links.
pub(crate) fn audit_archive(archive: &Path) -> Result<(), String> {
    let out = Command::new("tar")
        .arg("-tzvf")
        .arg(archive)
        .output()
        .map_err(|e| format!("tar command failed: {e}"))?;
    if !out.status.success() {
        return Err("corrupt or unreadable archive".to_string());
    }
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let kind = line.chars().next().unwrap_or('?');
        if !matches!(kind, '-' | 'd') {
            return Err(format!("unsupported archive member type in '{line}'"));
        }
    }
    let names = Command::new("tar")
        .arg("-tzf")
        .arg(archive)
        .output()
        .map_err(|e| format!("tar command failed: {e}"))?;
    for name in String::from_utf8_lossy(&names.stdout).lines() {
        let name = name.trim_start_matches("./").trim_end_matches('/');
        if name.is_empty() || name == "." {
            continue;
        }
        safe_relative(name)?;
    }
    Ok(())
}

/// Verifies a staged bundle: manifest metadata vs. target, digests, ELF identity,
/// and the absence of unlisted files. Performs no mutation outside `staged`.
pub fn verify_staged_bundle(
    staged: &Path,
    arch: Architecture,
    libc: Libc,
) -> Result<BundleManifest, String> {
    let text = fs::read_to_string(staged.join(BUNDLE_MANIFEST))
        .map_err(|e| format!("missing or unreadable {BUNDLE_MANIFEST}: {e}"))?;
    let manifest = BundleManifest::parse(&text)?;
    if manifest.os != "linux"
        || manifest.arch != arch_name(arch)
        || manifest.libc != libc_name(libc)
    {
        return Err(format!(
            "bundle metadata {}/{}/{} does not match target linux/{}/{}",
            manifest.os,
            manifest.arch,
            manifest.libc,
            arch_name(arch),
            libc_name(libc)
        ));
    }
    let listed: BTreeSet<&PathBuf> = manifest.entries.iter().map(|e| &e.path).collect();
    for entry in &manifest.entries {
        let full = staged.join(&entry.path);
        let actual = sha256_hex(&full)
            .map_err(|e| format!("missing bundle file {}: {e}", entry.path.display()))?;
        if actual != entry.sha256 {
            return Err(format!("digest mismatch for {}", entry.path.display()));
        }
        if entry.executable {
            check_elf(&full, arch)?;
        }
    }
    let mut stack = vec![staged.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for item in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let path = item.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path
                    .strip_prefix(staged)
                    .map_err(|e| e.to_string())?
                    .to_path_buf();
                if rel != Path::new(BUNDLE_MANIFEST) && !listed.contains(&rel) {
                    return Err(format!("unlisted file in bundle: {}", rel.display()));
                }
            }
        }
    }
    Ok(manifest)
}

fn publish(staged: &Path, target: &Path, manifest: &BundleManifest) -> io::Result<()> {
    fs::create_dir_all(target)?;
    let rels = manifest
        .entries
        .iter()
        .map(|e| e.path.clone())
        .chain(std::iter::once(PathBuf::from(BUNDLE_MANIFEST)));
    for rel in rels {
        let dest = target.join(&rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(staged.join(&rel), dest)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // linear fail-fast workflow with per-step diagnostics
pub fn execute_install(
    options: &InstallOptions,
    inputs: &mut dyn CheckInputs,
    _stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<u8> {
    writeln!(stderr, "\n  rubixctl  install\n\n  > Resolving host target")?;

    let evidence = match inputs.discover() {
        Ok(evidence) => evidence,
        Err(err) => {
            writeln!(
                stderr,
                "  [fail] system detection: {err}; obtain supported host observations before retrying"
            )?;
            return Ok(1);
        },
    };

    let host_target = match resolve_target(None, None, Some(&evidence)) {
        Ok(t) => t,
        Err(err) => {
            writeln!(stderr, "  [fail] system detection: {err}")?;
            return Ok(1);
        },
    };

    let host_libc = match evidence.executable.environment.as_str() {
        "gnu" => Some(Libc::Glibc),
        "musl" => Some(Libc::Musl),
        _ => None,
    };

    let Some(offline_path) = &options.offline_install else {
        writeln!(
            stderr,
            "error: command 'install' is not yet implemented (online install; use --offline-install)"
        )?;
        return Ok(1);
    };

    let filename = offline_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let parsed = match ArtifactNaming::parse_node_archive(&filename) {
        Ok(p) => p,
        Err(err) => {
            writeln!(stderr, "  [fail] bundle validation: {err}")?;
            return Ok(1);
        },
    };

    if parsed.variant.architecture != host_target.architecture
        || Some(parsed.variant.libc) != host_libc
    {
        writeln!(
            stderr,
            "  [fail] bundle validation: mismatched architecture or libc"
        )?;
        return Ok(1);
    }
    writeln!(stderr, "  [ok] Bundle matches host architecture")?;

    // Everything below up to `publish` touches only a private staging directory.
    if let Err(err) = audit_archive(offline_path) {
        writeln!(stderr, "  [fail] bundle validation: {err}")?;
        return Ok(1);
    }
    let target_dir = &options.path;
    let parent = target_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staging = match fs::create_dir_all(parent).and_then(|()| tempfile::TempDir::new_in(parent))
    {
        Ok(s) => s,
        Err(err) => {
            writeln!(stderr, "  [fail] extraction: {err}")?;
            return Ok(1);
        },
    };
    match Command::new("tar")
        .arg("-xzf")
        .arg(offline_path)
        .arg("-C")
        .arg(staging.path())
        .status()
    {
        Ok(status) if status.success() => {},
        Ok(status) => {
            writeln!(
                stderr,
                "  [fail] extraction: tar exited with status {status}"
            )?;
            return Ok(1);
        },
        Err(err) => {
            writeln!(stderr, "  [fail] extraction: tar command failed: {err}")?;
            return Ok(1);
        },
    }
    let manifest = match verify_staged_bundle(
        staging.path(),
        parsed.variant.architecture,
        parsed.variant.libc,
    ) {
        Ok(m) => m,
        Err(err) => {
            writeln!(
                stderr,
                "  [fail] bundle validation: {err}; no installation changes were made"
            )?;
            return Ok(1);
        },
    };
    if let Err(err) = publish(staging.path(), target_dir, &manifest) {
        writeln!(stderr, "  [fail] extraction: {err}")?;
        return Ok(1);
    }
    writeln!(
        stderr,
        "  [ok] Verified and extracted offline bundle {} ({} files)",
        manifest.version,
        manifest.entries.len()
    )?;
    Ok(0)
}
