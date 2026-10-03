//! Kubeconfig management: identity resolution, backup, parsing, and merging.
use crate::CheckInputs;
use crate::contract::KubeconfigOptions;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[cfg(unix)]
use rubix_supervisor::rustix;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Resolved invoking user identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvokingUser {
    pub username: String,
    pub home_dir: PathBuf,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

/// Resolves the invoking user from environment variables and `/etc/passwd`.
///
/// Priority:
/// 1. `SUDO_USER` (if present and != "root"):
///    - `SUDO_UID` / `SUDO_GID` from env if present.
///    - Read `/etc/passwd` to look up home directory and fallback UID/GID.
///    - If not in `/etc/passwd`, home defaults to `/home/<SUDO_USER>`.
/// 2. `DOAS_USER` (if present and != "root"):
///    - Read `/etc/passwd` for UID, GID, and home directory.
///    - If not in `/etc/passwd`, home defaults to `/home/<DOAS_USER>`.
/// 3. Normal user or root fallback:
///    - `HOME` env variable (fallback `/root` if empty/unset).
///    - `USER` env variable (fallback `root` if empty/unset).
///    - Current process effective UID / GID on Unix.
///
/// Under no circumstances is `/proc/self/loginuid` accessed or guessed.
pub fn resolve_invoking_user(environment: &BTreeMap<String, String>) -> InvokingUser {
    let passwd_content = fs::read_to_string("/etc/passwd").unwrap_or_default();
    resolve_invoking_user_with_passwd(environment, &passwd_content)
}

/// Pure helper for resolving invoking user with explicit passwd file content (useful for tests).
#[allow(clippy::similar_names)]
pub fn resolve_invoking_user_with_passwd(
    environment: &BTreeMap<String, String>,
    passwd_content: &str,
) -> InvokingUser {
    if let Some(sudo_user) = environment.get("SUDO_USER")
        && !sudo_user.is_empty()
        && sudo_user != "root"
    {
        let env_uid = environment
            .get("SUDO_UID")
            .and_then(|s| s.parse::<u32>().ok());
        let env_gid = environment
            .get("SUDO_GID")
            .and_then(|s| s.parse::<u32>().ok());

        let passwd_info = lookup_passwd(passwd_content, sudo_user);
        let uid = env_uid.or_else(|| passwd_info.as_ref().map(|p| p.uid));
        let gid = env_gid.or_else(|| passwd_info.as_ref().map(|p| p.gid));
        let home_dir =
            passwd_info.map_or_else(|| PathBuf::from(format!("/home/{sudo_user}")), |p| p.home);

        return InvokingUser {
            username: sudo_user.clone(),
            home_dir,
            uid,
            gid,
        };
    }

    if let Some(doas_user) = environment.get("DOAS_USER")
        && !doas_user.is_empty()
        && doas_user != "root"
    {
        let passwd_info = lookup_passwd(passwd_content, doas_user);
        let uid = passwd_info.as_ref().map(|p| p.uid);
        let gid = passwd_info.as_ref().map(|p| p.gid);
        let home_dir =
            passwd_info.map_or_else(|| PathBuf::from(format!("/home/{doas_user}")), |p| p.home);

        return InvokingUser {
            username: doas_user.clone(),
            home_dir,
            uid,
            gid,
        };
    }

    // Default / fallback to current process user
    let username = environment
        .get("USER")
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "root".to_string());

    let home_dir = environment
        .get("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| {
            if username == "root" {
                PathBuf::from("/root")
            } else {
                PathBuf::from(format!("/home/{username}"))
            }
        });

    #[cfg(unix)]
    let (proc_uid, proc_gid) = current_process_uid_gid();
    #[cfg(unix)]
    let (uid, gid) = (Some(proc_uid), Some(proc_gid));
    #[cfg(not(unix))]
    let (uid, gid) = (None, None);

    InvokingUser {
        username,
        home_dir,
        uid,
        gid,
    }
}

struct PasswdEntry {
    uid: u32,
    gid: u32,
    home: PathBuf,
}

fn lookup_passwd(content: &str, target_user: &str) -> Option<PasswdEntry> {
    for line in content.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() >= 6 && fields[0] == target_user {
            let uid = fields[2].parse::<u32>().ok()?;
            let gid = fields[3].parse::<u32>().ok()?;
            let home = PathBuf::from(fields[5]);
            return Some(PasswdEntry { uid, gid, home });
        }
    }
    None
}

/// Helper to get current process effective UID and GID.
#[cfg(unix)]
fn current_process_uid_gid() -> (u32, u32) {
    let uid = rustix::process::geteuid().as_raw();
    let gid = rustix::process::getegid().as_raw();
    (uid, gid)
}

/// Resolves standard target destination path for `rubixctl kubeconfig`.
/// If `output` is specified, it is used (expanding leading `~` to user's home).
/// Otherwise, defaults to `<user_home>/.kube/config`.
pub fn resolve_destination(user: &InvokingUser, output: Option<&Path>) -> PathBuf {
    if let Some(out) = output {
        expand_tilde(out, &user.home_dir)
    } else {
        user.home_dir.join(".kube").join("config")
    }
}

fn expand_tilde(path: &Path, home: &Path) -> PathBuf {
    if let Ok(stripped) = path.strip_prefix("~") {
        home.join(stripped)
    } else {
        path.to_path_buf()
    }
}

/// Formats UTC `SystemTime` as YYYYMMDDHHMMSS for backup suffix.
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
pub fn format_backup_timestamp(now: SystemTime) -> String {
    let dur = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();

    // Pure Rust civil time computation
    let days = (secs / 86_400) as i64;
    let rem_secs = (secs % 86_400) as u32;

    let hour = rem_secs / 3600;
    let min = (rem_secs % 3600) / 60;
    let sec = rem_secs % 60;

    // Euclidean affine transform from day count to civil date (Howard Hinnant algorithm)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = i64::from(yoe) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}{m:02}{d:02}{hour:02}{min:02}{sec:02}")
}

/// Creates a pre-merge backup if destination exists.
/// Returns `Some(backup_path)` if created, or None if destination did not exist.
pub fn create_premerge_backup(dest: &Path, now: SystemTime) -> io::Result<Option<PathBuf>> {
    if !dest.exists() {
        return Ok(None);
    }
    let ts = format_backup_timestamp(now);
    let backup_name = format!(
        "{}.backup-{}",
        dest.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("config"),
        ts
    );
    let backup_path = dest
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(backup_name);

    fs::copy(dest, &backup_path)?;
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&backup_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&backup_path, perms)?;
    }
    Ok(Some(backup_path))
}

/// Atomic write with 0600 mode, ensures parent dir exists with 0700 mode,
/// and applies chown to invoking user if running with root privileges.
pub fn atomic_write_secure(path: &Path, data: &[u8], user: &InvokingUser) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    if !parent.exists() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(parent)?.permissions();
            perms.set_mode(0o700);
            fs::set_permissions(parent, perms)?;
        }
    }

    let mut temp = tempfile::Builder::new().tempfile_in(parent)?;

    #[cfg(unix)]
    {
        let mut perms = temp.as_file().metadata()?.permissions();
        perms.set_mode(0o600);
        temp.as_file_mut().set_permissions(perms)?;
    }

    temp.write_all(data)?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;

    // Adjust ownership if running as root
    chown_invoking_user(path, user)?;

    Ok(())
}

/// Recursively chown the `.kube` directory or file if running as root under sudo/doas.
#[allow(clippy::collapsible_if, clippy::excessive_nesting)]
pub fn chown_invoking_user(path: &Path, user: &InvokingUser) -> io::Result<()> {
    #[cfg(unix)]
    {
        let (current_uid, _) = current_process_uid_gid();
        if current_uid == 0
            && let (Some(target_uid), Some(target_gid)) = (user.uid, user.gid)
            && target_uid != 0
        {
            let uid = Some(rustix::fs::Uid::from_raw(target_uid));
            let gid = Some(rustix::fs::Gid::from_raw(target_gid));

            // If path is inside or is .kube, recursively chown ~/.kube
            let kube_dir = user.home_dir.join(".kube");
            if path.starts_with(&kube_dir) && kube_dir.exists() {
                chown_dir_recursive(&kube_dir, uid, gid)?;
            } else if path.exists() {
                let _ = rustix::fs::chown(path, uid, gid);
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, user);
    }
    Ok(())
}

#[cfg(unix)]
fn chown_dir_recursive(
    dir: &Path,
    uid: Option<rustix::fs::Uid>,
    gid: Option<rustix::fs::Gid>,
) -> io::Result<()> {
    let _ = rustix::fs::chown(dir, uid, gid);
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                chown_dir_recursive(&path, uid, gid)?;
            } else {
                let _ = rustix::fs::chown(&path, uid, gid);
            }
        }
    }
    Ok(())
}

/// Locates admin kubeconfig file inside the given data directory.
/// Checks:
/// 1. `<data_path>/pki/admin/admin.kubeconfig`
/// 2. `<data_path>/pki/admin.kubeconfig`
/// 3. `<data_path>/admin.kubeconfig`
pub fn locate_admin_kubeconfig(data_path: &Path) -> io::Result<PathBuf> {
    let p1 = data_path.join("pki").join("admin").join("admin.kubeconfig");
    if p1.is_file() {
        return Ok(p1);
    }
    let p2 = data_path.join("pki").join("admin.kubeconfig");
    if p2.is_file() {
        return Ok(p2);
    }
    let p3 = data_path.join("admin.kubeconfig");
    if p3.is_file() {
        return Ok(p3);
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "admin kubeconfig not found in {}. Ensure the cluster has been initialized",
            data_path.display()
        ),
    ))
}

/// Parse a kubeconfig YAML or JSON string into a structured `serde_json::Value`.
pub fn parse_kubeconfig_content(content: &str) -> io::Result<Value> {
    // Attempt JSON first
    if let Ok(v) = serde_json::from_str::<Value>(content)
        && v.is_object()
    {
        return Ok(v);
    }

    // Parse YAML using line-based structural parser for standard kubeconfigs
    parse_yaml_kubeconfig(content)
}

/// Pure Safe Rust YAML parser tailored for Kubeconfig documents.
/// Handles standard Kubeconfig schema:
/// apiVersion, kind, current-context, preferences, clusters, contexts, users.
fn parse_yaml_kubeconfig(content: &str) -> io::Result<Value> {
    let mut map = serde_json::Map::new();
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.peek() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            lines.next();
            continue;
        }

        let leading_spaces = trimmed.len() - trimmed.trim_start().len();
        if leading_spaces != 0 {
            lines.next();
            continue;
        }

        if let Some((key, rest)) = trimmed.split_once(':') {
            let key = key.trim();
            let rest = rest.trim();
            lines.next();

            if key == "clusters" || key == "contexts" || key == "users" {
                let items = parse_yaml_list(&mut lines)?;
                map.insert(key.to_string(), Value::Array(items));
            } else if rest.is_empty() {
                // Mapping or empty
                if lines.peek().is_some_and(|l| l.starts_with("  ")) {
                    let sub_obj = parse_yaml_object(&mut lines, 2)?;
                    map.insert(key.to_string(), Value::Object(sub_obj));
                } else {
                    map.insert(key.to_string(), Value::Null);
                }
            } else if rest == "{}" {
                map.insert(key.to_string(), Value::Object(serde_json::Map::new()));
            } else if rest == "[]" {
                map.insert(key.to_string(), Value::Array(Vec::new()));
            } else {
                map.insert(key.to_string(), parse_scalar_val(rest));
            }
        } else {
            lines.next();
        }
    }

    Ok(Value::Object(map))
}

#[allow(clippy::excessive_nesting)]
fn next_line_indent(
    lines: &mut std::iter::Peekable<std::str::Lines<'_>>,
    min_indent: usize,
) -> Option<usize> {
    let line = lines.peek()?;
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    let indent = trimmed.len() - trimmed.trim_start().len();
    if indent > min_indent {
        Some(indent)
    } else {
        None
    }
}

fn parse_yaml_object(
    lines: &mut std::iter::Peekable<std::str::Lines<'_>>,
    min_indent: usize,
) -> io::Result<serde_json::Map<String, Value>> {
    let mut map = serde_json::Map::new();

    while let Some(line) = lines.peek() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            lines.next();
            continue;
        }

        let indent = trimmed.len() - trimmed.trim_start().len();
        if indent < min_indent {
            break;
        }

        let trimmed_content = trimmed.trim_start();
        if trimmed_content.starts_with('-') {
            break;
        }

        let Some((k, v)) = trimmed_content.split_once(':') else {
            lines.next();
            continue;
        };

        let k = k.trim();
        let v = v.trim();
        lines.next();

        if v.is_empty() {
            if let Some(child_indent) = next_line_indent(lines, indent) {
                if lines
                    .peek()
                    .is_some_and(|l| l.trim_start().starts_with('-'))
                {
                    let list = parse_yaml_list(lines)?;
                    map.insert(k.to_string(), Value::Array(list));
                } else {
                    let sub_map = parse_yaml_object(lines, child_indent)?;
                    map.insert(k.to_string(), Value::Object(sub_map));
                }
            } else {
                map.insert(k.to_string(), Value::Null);
            }
        } else if v == "{}" {
            map.insert(k.to_string(), Value::Object(serde_json::Map::new()));
        } else if v == "[]" {
            map.insert(k.to_string(), Value::Array(Vec::new()));
        } else {
            map.insert(k.to_string(), parse_scalar_val(v));
        }
    }

    Ok(map)
}

fn parse_yaml_list(lines: &mut std::iter::Peekable<std::str::Lines<'_>>) -> io::Result<Vec<Value>> {
    let mut list = Vec::new();

    while let Some(line) = lines.peek() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            lines.next();
            continue;
        }
        let indent = trimmed.len() - trimmed.trim_start().len();

        let trimmed_content = trimmed.trim_start();
        if !trimmed_content.starts_with('-') {
            break;
        }

        let after_dash = trimmed_content[1..].trim();
        lines.next();

        let mut item_map = serde_json::Map::new();
        if !after_dash.is_empty()
            && let Some((k, v)) = after_dash.split_once(':')
        {
            let k = k.trim();
            let v = v.trim();
            if v.is_empty() {
                // If the next line is indented deeper than sibling fields (indent + 2),
                // it belongs to this child mapping.
                if let Some(child_indent) = next_line_indent(lines, indent + 2) {
                    let sub_map = parse_yaml_object(lines, child_indent)?;
                    item_map.insert(k.to_string(), Value::Object(sub_map));
                } else {
                    item_map.insert(k.to_string(), Value::Null);
                }
            } else {
                item_map.insert(k.to_string(), parse_scalar_val(v));
            }
        }

        let sub_fields = parse_yaml_object(lines, indent + 1)?;
        for (k, v) in sub_fields {
            item_map.insert(k, v);
        }

        list.push(Value::Object(item_map));
    }

    Ok(list)
}

fn parse_scalar_val(s: &str) -> Value {
    let unquoted =
        if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
            &s[1..s.len() - 1]
        } else {
            s
        };

    if unquoted == "true" {
        Value::Bool(true)
    } else if unquoted == "false" {
        Value::Bool(false)
    } else if let Ok(n) = unquoted.parse::<i64>() {
        Value::Number(n.into())
    } else {
        Value::String(unquoted.to_string())
    }
}

/// Merges incoming kubeconfig into destination kubeconfig.
/// Preserves all existing unrelated clusters, contexts, users, preferences, and extra keys.
/// Updates or adds clusters/contexts/users with matching name.
/// Sets `current-context` to incoming kubeconfig's `current-context`.
pub fn merge_kubeconfigs(target: &mut Value, incoming: &Value) {
    let Value::Object(target_obj) = target else {
        return;
    };

    let Value::Object(incoming_obj) = incoming else {
        return;
    };

    if !target_obj.contains_key("apiVersion") {
        if let Some(v) = incoming_obj.get("apiVersion") {
            target_obj.insert("apiVersion".to_string(), v.clone());
        } else {
            target_obj.insert("apiVersion".to_string(), Value::String("v1".to_string()));
        }
    }
    if !target_obj.contains_key("kind") {
        if let Some(v) = incoming_obj.get("kind") {
            target_obj.insert("kind".to_string(), v.clone());
        } else {
            target_obj.insert("kind".to_string(), Value::String("Config".to_string()));
        }
    }
    if !target_obj.contains_key("preferences") {
        target_obj.insert(
            "preferences".to_string(),
            Value::Object(serde_json::Map::new()),
        );
    }

    // Merge named list entries: clusters, contexts, users
    for list_key in ["clusters", "contexts", "users"] {
        merge_named_list(target_obj, incoming_obj, list_key);
    }

    // Set current-context
    if let Some(ctx) = incoming_obj.get("current-context") {
        target_obj.insert("current-context".to_string(), ctx.clone());
    }
}

fn merge_named_list(
    target_obj: &mut serde_json::Map<String, Value>,
    incoming_obj: &serde_json::Map<String, Value>,
    list_key: &str,
) {
    let incoming_items = incoming_obj
        .get(list_key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let existing_items = target_obj
        .entry(list_key.to_string())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut();

    let Some(existing) = existing_items else {
        return;
    };

    for incoming_entry in incoming_items {
        let incoming_name = incoming_entry.get("name").and_then(Value::as_str);

        if let Some(name) = incoming_name {
            if let Some(exist) = existing
                .iter_mut()
                .find(|e| e.get("name").and_then(Value::as_str) == Some(name))
            {
                *exist = incoming_entry;
            } else {
                existing.push(incoming_entry);
            }
        } else {
            existing.push(incoming_entry);
        }
    }
}

/// Serializes kubeconfig `serde_json::Value` to standardized YAML.
pub fn serialize_kubeconfig(val: &Value) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let Some(obj) = val.as_object() else {
        return out;
    };

    if let Some(api) = obj.get("apiVersion").and_then(Value::as_str) {
        let _ = writeln!(out, "apiVersion: {api}");
    } else {
        out.push_str("apiVersion: v1\n");
    }

    // clusters
    out.push_str("clusters:\n");
    if let Some(clusters) = obj.get("clusters").and_then(Value::as_array) {
        for c in clusters {
            serialize_named_entry(&mut out, c, "cluster");
        }
    }

    // contexts
    out.push_str("contexts:\n");
    if let Some(contexts) = obj.get("contexts").and_then(Value::as_array) {
        for ctx in contexts {
            serialize_named_entry(&mut out, ctx, "context");
        }
    }

    // current-context
    if let Some(curr) = obj.get("current-context").and_then(Value::as_str) {
        let _ = writeln!(out, "current-context: {curr}");
    }

    if let Some(kind) = obj.get("kind").and_then(Value::as_str) {
        let _ = writeln!(out, "kind: {kind}");
    } else {
        out.push_str("kind: Config\n");
    }

    // preferences
    out.push_str("preferences: {}\n");

    // users
    out.push_str("users:\n");
    if let Some(users) = obj.get("users").and_then(Value::as_array) {
        for u in users {
            serialize_named_entry(&mut out, u, "user");
        }
    }

    out
}

fn serialize_named_entry(out: &mut String, val: &Value, subkey: &str) {
    use std::fmt::Write as _;
    let Some(obj) = val.as_object() else { return };
    let name = obj.get("name").and_then(Value::as_str).unwrap_or("");
    let sub = obj.get(subkey).and_then(Value::as_object);

    let _ = writeln!(out, "- {subkey}:");
    if let Some(sub_map) = sub {
        for (k, v) in sub_map {
            if let Some(s) = v.as_str() {
                let _ = writeln!(out, "    {k}: {s}");
            } else if let Some(b) = v.as_bool() {
                let _ = writeln!(out, "    {k}: {b}");
            } else if let Some(n) = v.as_i64() {
                let _ = writeln!(out, "    {k}: {n}");
            }
        }
    }
    let _ = writeln!(out, "  name: {name}");
}

/// Executes `rubixctl kubeconfig` commands: view, fetch, merge, or root.
pub fn execute_kubeconfig(
    options: &KubeconfigOptions,
    inputs: &mut dyn CheckInputs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<u8> {
    execute_kubeconfig_with_env(options, inputs, &BTreeMap::new(), stdout, stderr)
}

/// Executes `rubixctl kubeconfig` with explicit environment for identity resolution.
pub fn execute_kubeconfig_with_env(
    options: &KubeconfigOptions,
    _inputs: &mut dyn CheckInputs,
    environment: &BTreeMap<String, String>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<u8> {
    // If environment is empty, inspect actual process environment

    let env_map = if environment.is_empty() {
        std::env::vars().collect::<BTreeMap<_, _>>()
    } else {
        environment.clone()
    };

    let user = resolve_invoking_user(&env_map);
    let admin_kubeconfig_path = match locate_admin_kubeconfig(&options.path) {
        Ok(path) => path,
        Err(err) => {
            writeln!(stderr, "error: {err}")?;
            return Ok(1);
        },
    };
    let admin_content = match fs::read_to_string(&admin_kubeconfig_path) {
        Ok(c) => c,
        Err(err) => {
            writeln!(stderr, "error: failed to read admin kubeconfig: {err}")?;
            return Ok(1);
        },
    };

    match options.subcommand.as_deref() {
        Some("view") => {
            stdout.write_all(admin_content.as_bytes())?;
            if !admin_content.ends_with('\n') {
                stdout.write_all(b"\n")?;
            }
            Ok(0)
        },
        Some("fetch" | "merge") => {
            let dest_path = resolve_destination(&user, options.output.as_deref());

            // Pre-merge backup if destination exists
            if let Some(backup_path) = create_premerge_backup(&dest_path, SystemTime::now())? {
                writeln!(
                    stderr,
                    "Created backup of existing kubeconfig at {}",
                    backup_path.display()
                )?;
            }

            let mut current_cfg = if dest_path.is_file() {
                let existing_content = fs::read_to_string(&dest_path)?;
                parse_kubeconfig_content(&existing_content)?
            } else {
                Value::Object(serde_json::Map::new())
            };

            let admin_val = parse_kubeconfig_content(&admin_content)?;
            merge_kubeconfigs(&mut current_cfg, &admin_val);

            let merged_yaml = serialize_kubeconfig(&current_cfg);
            atomic_write_secure(&dest_path, merged_yaml.as_bytes(), &user)?;

            writeln!(
                stdout,
                "Kubeconfig successfully written to {}",
                dest_path.display()
            )?;
            Ok(0)
        },
        None => {
            // Root kubeconfig command
            if let Some(ref out) = options.output {
                let dest_path = expand_tilde(out, &user.home_dir);
                atomic_write_secure(&dest_path, admin_content.as_bytes(), &user)?;
                writeln!(stdout, "Kubeconfig written to {}", dest_path.display())?;
            } else {
                stdout.write_all(admin_content.as_bytes())?;
                if !admin_content.ends_with('\n') {
                    stdout.write_all(b"\n")?;
                }
            }
            Ok(0)
        },
        Some(unknown) => {
            writeln!(
                stderr,
                "error: unknown subcommand '{unknown}' for kubeconfig"
            )?;
            Ok(1)
        },
    }
}
