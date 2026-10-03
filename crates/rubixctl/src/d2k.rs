use crate::CheckInputs;
use crate::contract::D2kOptions;
use crate::kubeconfig::chown_invoking_user;
use crate::kubeconfig::resolve_invoking_user;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

const D2K_PORT: u16 = 2376;

struct D2kFile<'a> {
    container_path: &'a str,
    local_name: &'a str,
}

const D2K_FILES: &[D2kFile<'_>] = &[
    D2kFile {
        container_path: "/var/lib/kubesolo/pki/ca/ca.crt",
        local_name: "ca.pem",
    },
    D2kFile {
        container_path: "/var/lib/kubesolo/pki/d2k/client.crt",
        local_name: "cert.pem",
    },
    D2kFile {
        container_path: "/var/lib/kubesolo/pki/d2k/client.key",
        local_name: "key.pem",
    },
];

fn find_executable(name: &str) -> Option<PathBuf> {
    if let Ok(paths) = std::env::var("PATH") {
        for path in std::env::split_paths(&paths) {
            let exe = path.join(name);
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

#[allow(
    clippy::too_many_lines,
    clippy::excessive_nesting,
    clippy::uninlined_format_args,
    clippy::collapsible_if,
    clippy::manual_let_else,
    clippy::single_match_else
)]
pub fn execute_d2k(
    options: &D2kOptions,
    _inputs: &mut dyn CheckInputs,
    engine: &mut dyn crate::endpoints::EnginePortInspector,
    environment: &BTreeMap<String, String>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<u8> {
    let subcommand = options.subcommand.as_deref().unwrap_or("help");
    if subcommand != "fetch" && subcommand != "install" {
        writeln!(stderr, "error: unknown d2k subcommand '{subcommand}'")?;
        return Ok(1);
    }

    let name = options.name.as_deref().unwrap_or("rubix");
    let user = resolve_invoking_user(environment);

    let cert_dir = user.home_dir.join(".docker").join("d2k").join(name);
    fs::create_dir_all(&cert_dir)?;
    chown_invoking_user(&cert_dir, &user)?;
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&cert_dir)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&cert_dir, perms)?;
    }

    let is_host_install = options
        .path
        .join("pki")
        .join("d2k")
        .join("client.crt")
        .exists();

    for f in D2K_FILES {
        let dest = cert_dir.join(f.local_name);
        if is_host_install {
            let host_path = if f.container_path.starts_with("/var/lib/kubesolo/") {
                options
                    .path
                    .join(f.container_path.trim_start_matches("/var/lib/kubesolo/"))
            } else {
                PathBuf::from(f.container_path)
            };
            if let Ok(data) = fs::read(&host_path) {
                fs::write(&dest, data)?;
            } else {
                writeln!(stderr, "error: failed to read {}", host_path.display())?;
                return Ok(1);
            }
        } else {
            let mut cp = Command::new("docker");
            cp.args([
                "cp",
                &format!("{}:{}", name, f.container_path),
                dest.to_str().unwrap(),
            ]);
            if !cp.status()?.success() {
                writeln!(stderr, "error: docker cp failed for {}", f.container_path)?;
                return Ok(1);
            }
        }

        chown_invoking_user(&dest, &user)?;
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&dest)?.permissions();
            perms.set_mode(if f.local_name == "ca.pem" {
                0o644
            } else {
                0o600
            });
            fs::set_permissions(&dest, perms)?;
        }
    }

    // Port discovery
    let mut port = D2K_PORT;
    if !is_host_install {
        if let Ok(raw) = engine.inspect_port(name, D2K_PORT) {
            if let Ok(endpoint) = crate::endpoints::resolve_published_endpoint(&raw) {
                port = endpoint.port;
            }
        } else {
            // fallback using docker port command directly if engine is UnavailableEngine
            if let Ok(out) = Command::new("docker")
                .args(["port", name, "2376/tcp"])
                .output()
            {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    if let Ok(endpoint) = crate::endpoints::resolve_published_endpoint(&s) {
                        port = endpoint.port;
                    }
                }
            }
        }
    }

    let endpoint = format!(
        "host=tcp://127.0.0.1:{},ca={},cert={},key={}",
        port,
        cert_dir.join("ca.pem").display(),
        cert_dir.join("cert.pem").display(),
        cert_dir.join("key.pem").display(),
    );

    let docker_path = match find_executable("docker") {
        Some(p) => p,
        None => {
            writeln!(stdout, "cert files written to {}", cert_dir.display())?;
            writeln!(
                stdout,
                "docker command line tool not found — install docker, then run: rubixctl d2k fetch"
            )?;
            return Ok(0);
        },
    };

    let desc = format!("Rubix cluster ({name})");
    let create_out = Command::new(&docker_path)
        .args([
            "context",
            "create",
            name,
            "--description",
            &desc,
            "--docker",
            &endpoint,
        ])
        .output()?;

    if create_out.status.success() {
        writeln!(stdout, "Docker context {:?} created", name)?;
    } else {
        let stderr_out = String::from_utf8_lossy(&create_out.stderr);
        if stderr_out.contains("already exists") {
            let update_out = Command::new(&docker_path)
                .args([
                    "context",
                    "update",
                    name,
                    "--description",
                    &desc,
                    "--docker",
                    &endpoint,
                ])
                .output()?;
            if !update_out.status.success() {
                writeln!(
                    stderr,
                    "error: failed to update Docker context {:?}: {}",
                    name,
                    String::from_utf8_lossy(&update_out.stderr)
                )?;
                return Ok(1);
            }
            writeln!(stdout, "Docker context {:?} updated", name)?;
        } else {
            writeln!(
                stderr,
                "error: failed to create Docker context {:?}: {}",
                name, stderr_out
            )?;
            return Ok(1);
        }
    }

    writeln!(stdout, "D2K context {:?} configured", name)?;
    Ok(0)
}
