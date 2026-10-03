use crate::CheckInputs;
use crate::artifact::{artifact_archive_name, artifact_download_url, resolve_target};
use crate::contract::DownloadOptions;
use std::io::{self, Write};

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
    if !matches!(executable_target, Ok(host) if host.architecture == target.architecture) {
        writeln!(
            stderr,
            "  [fail] bundle installer does not match the selected Linux architecture; run download with a matching Linux rubixctl executable. No bundle files were created."
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

    if let Err(err) = inputs.download_file(&download_url, &archive_path, options.proxy.as_deref()) {
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
        archive_path.display()
    )?;

    Ok(0)
}
