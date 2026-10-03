use crate::CheckInputs;
use crate::artifact::{artifact_archive_name, artifact_download_url, resolve_target};
use crate::contract::DownloadOptions;
use rubix_platform::Libc;
use std::io::{self, Write};
use std::path::Path;

/// Downloads into owned staging, then atomically publishes on the destination
/// filesystem. Failed archive downloads leave existing archives untouched.
pub fn stage_download(
    dest: &Path,
    temp_dir: Option<&Path>,
    fetch: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let staging = match temp_dir {
        Some(parent) => {
            std::fs::create_dir_all(parent)?;
            tempfile::TempDir::new_in(parent)?
        },
        None => tempfile::TempDir::new()?,
    };
    let staged = staging.path().join("bundle");
    fetch(&staged)?;
    let parent = dest
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut published = tempfile::NamedTempFile::new_in(parent)?;
    let mut source = std::fs::File::open(staged)?;
    io::copy(&mut source, &mut published)?;
    published.as_file().sync_all()?;
    published.persist(dest).map_err(|error| error.error)?;
    Ok(())
}

/// Publishes an executable only after a complete copy, permission enforcement,
/// and sync. Partial copies cannot truncate a previously published installer.
pub fn stage_installer(
    dest: &Path,
    copy: impl FnOnce(&mut std::fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let parent = dest
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut installer = tempfile::NamedTempFile::new_in(parent)?;
    copy(installer.as_file_mut())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        installer
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }
    installer.as_file().sync_all()?;
    installer.persist(dest).map_err(|error| error.error)?;
    Ok(())
}

/// Curl receives a bounded protocol policy and an explicit end of options.
pub fn curl_download_command(url: &str, dest: &Path, proxy: Option<&str>) -> std::process::Command {
    let mut command = std::process::Command::new("curl");
    command
        .args([
            "-fSL",
            "--proto",
            "=https,http",
            "--proto-redir",
            "=https,http",
            "-o",
        ])
        .arg(dest);
    if let Some(proxy) = proxy {
        command.arg("--proxy").arg(proxy);
    }
    command.arg("--").arg(url);
    command
}

/// Executes the artifact download workflow: resolves target architecture and libc,
/// calculates archive name and URL, fetches the binary bundle, and stages the management CLI.
pub fn execute_download(
    options: &DownloadOptions,
    inputs: &mut dyn CheckInputs,
    _stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<u8> {
    writeln!(
        stderr,
        "\n  rubixctl  download\n\n  > Resolving target architecture"
    )?;

    // Bundles include the running management executable. Check its platform
    // even for an explicit archive target, before creating any bundle outputs.
    let evidence = match inputs.discover() {
        Ok(evidence) => evidence,
        Err(err) => {
            writeln!(
                stderr,
                "  [fail] architecture detection: {err}; obtain supported host observations before retrying"
            )?;
            return Ok(1);
        },
    };
    let target = if let Some(ref arch_str) = options.arch {
        match resolve_target(Some(arch_str), options.libc, None) {
            Ok(target) => target,
            Err(err) => {
                writeln!(
                    stderr,
                    "  [fail] architecture detection: {err}; obtain supported host observations before retrying"
                )?;
                return Ok(1);
            },
        }
    } else {
        match resolve_target(None, options.libc, Some(&evidence)) {
            Ok(target) => target,
            Err(err) => {
                writeln!(
                    stderr,
                    "  [fail] architecture detection: {err}; obtain supported host observations before retrying"
                )?;
                return Ok(1);
            },
        }
    };

    let executable_target = resolve_target(None, None, Some(&evidence));
    let executable_libc = match evidence.executable.environment.as_str() {
        "gnu" => Some(Libc::Glibc),
        "musl" => Some(Libc::Musl),
        _ => None,
    };
    if !matches!(executable_target, Ok(host) if host.architecture == target.architecture)
        || executable_libc != Some(target.libc)
    {
        writeln!(
            stderr,
            "  [fail] bundle installer does not match the selected Linux architecture and libc ABI; run download with a matching Linux rubixctl executable. No bundle files were created."
        )?;
        return Ok(1);
    }

    let archive_name = artifact_archive_name(&options.version, target, options.offline);
    let download_url = artifact_download_url(
        &options.version,
        &archive_name,
        options.custom_url.as_deref(),
    );

    writeln!(
        stderr,
        "  [ok] Target resolved: {archive_name}\n\n  > Downloading Rubix {}",
        options.version
    )?;

    let dest_dir = &options.path;
    let archive_path = dest_dir.join(&archive_name);

    if let Err(err) = inputs.download_file(
        &download_url,
        &archive_path,
        options.proxy.as_deref(),
        options.temp_dir.as_deref(),
    ) {
        writeln!(stderr, "  [fail] download: {err}")?;
        return Ok(1);
    }

    let installer_path = dest_dir.join("rubixctl");
    if let Err(err) = inputs.copy_self(&installer_path) {
        writeln!(stderr, "  [fail] copy installer: {err}")?;
        return Ok(1);
    }

    writeln!(
        stderr,
        "  [ok] Bundle ready in {}\n\n  Next steps:\n  Transfer the files to the target machine, then:\n    sudo ./rubixctl install --offline-install={}\n",
        dest_dir.display(),
        format_args!("./{archive_name}")
    )?;

    Ok(0)
}
