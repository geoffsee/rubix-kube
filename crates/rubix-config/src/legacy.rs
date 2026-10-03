//! Legacy service argument extraction and conversion to versioned YAML.
//!
//! Upstream contract (`internal/cli/migrate_config.go`):
//! - Extracts known `KubeSolo` flags from legacy service definitions.
//! - Foreign flags (such as sysvinit `start-stop-daemon --start --quiet --pidfile`) are safely ignored.
//! - Shell quotes (`"..."`, `'...'`) are stripped from values.
//! - Supports `--no-<flag>` boolean negations (e.g. `--no-local-storage`).
//! - If `--config` is already present, migration is a no-op (already migrated).
//! - Resolves extracted flags with default configuration and renders clean `KubeSolo` YAML.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regex::Regex;

use crate::{
    Config, ConfigError, DEFAULT_CONFIG_PATH, ExplicitFlags, FIELDS, FieldType, HostContext,
    INPUT_BINDINGS, PersistenceError, render_effective_yaml, resolve_layers, write_document,
};

/// Regular expression pattern matching service arguments:
/// `--[a-z0-9][a-z0-9-]*(?:=(?:"[^"]*"|'[^']*'|[^\s"']+))?`
const SERVICE_FLAG_REGEX: &str = r#"--[a-z0-9][a-z0-9-]*(?:=(?:"[^"]*"|'[^']*'|[^\s"']+))?"#;

/// Strips surrounding double or single shell quotes from a flag value if present.
pub fn unquote_shell_value(val: &str) -> String {
    let s = val.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Checks if a service content line or definition already contains `--config`.
pub fn has_config_flag(content: &str) -> bool {
    let re = Regex::new(SERVICE_FLAG_REGEX).expect("valid regex");
    for mat in re.find_iter(content) {
        let flag_str = mat.as_str();
        let name_part = flag_str
            .strip_prefix("--")
            .unwrap_or(flag_str)
            .split_once('=')
            .map_or(flag_str.strip_prefix("--").unwrap_or(flag_str), |(k, _)| k);
        if name_part == "config" {
            return true;
        }
    }
    false
}

/// Information about a known `KubeSolo` flag definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlagKind {
    Bool,
    Other,
}

fn lookup_flag(name: &str) -> Option<(&'static str, FlagKind)> {
    if name == "full" {
        return Some(("full", FlagKind::Bool));
    }
    for binding in INPUT_BINDINGS {
        if let Some(flag) = binding.flag
            && flag == name
        {
            let field = FIELDS.iter().find(|f| f.path == binding.path);
            let kind = match field.map(|f| f.kind) {
                Some(FieldType::Boolean | FieldType::OptionalBool) => FlagKind::Bool,
                _ => FlagKind::Other,
            };
            return Some((flag, kind));
        }
    }
    None
}

/// Extracted legacy flags representation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtractedFlags {
    /// Explicit flag mappings: flag name -> string value
    pub flags: BTreeMap<String, String>,
}

/// Extracts known `KubeSolo` flags from legacy service file content.
///
/// Follows upstream rules:
/// 1. Finds all tokens matching `--[a-z0-9][a-z0-9-]*(?:=(?:"..."|'...'|...))?`.
/// 2. Checks against known `KubeSolo` flags (and `--no-<bool-flag>` variants).
/// 3. Ignores unknown/foreign flags (e.g. init script flags like `--start`, `--pidfile`).
/// 4. Strips shell quotes from values.
/// 5. Deduplicates: first occurrence wins.
pub fn extract_service_flags(content: &str) -> ExtractedFlags {
    let re = Regex::new(SERVICE_FLAG_REGEX).expect("valid regex");
    let mut flags = BTreeMap::new();
    let mut seen = BTreeSet::new();

    for mat in re.find_iter(content) {
        let raw = mat.as_str();
        let stripped = raw.strip_prefix("--").unwrap_or(raw);
        let (key_part, val_part) = stripped
            .split_once('=')
            .map_or((stripped, None), |(k, v)| (k, Some(v)));

        // Check if known or `--no-<known-bool>`
        let (canonical, is_negated) = if let Some(known) = lookup_flag(key_part) {
            (known.0, false)
        } else if let Some(negated_key) = key_part.strip_prefix("no-")
            && let Some((known_name, FlagKind::Bool)) = lookup_flag(negated_key)
        {
            (known_name, true)
        } else {
            // Foreign flag (e.g. --start, --pidfile, --exec, --background)
            continue;
        };

        // First occurrence wins
        if !seen.insert(canonical.to_string()) {
            continue;
        }

        let value = if is_negated {
            "false".to_string()
        } else if let Some(val) = val_part {
            unquote_shell_value(val)
        } else {
            // Boolean flag with no value defaults to "true"
            "true".to_string()
        };

        flags.insert(canonical.to_string(), value);
    }

    ExtractedFlags { flags }
}

/// Replaces extracted flags in the service definition content with `--config=<config_path>`.
pub fn rewrite_service_content(content: &str, config_path: &str) -> String {
    let flag_re = Regex::new(SERVICE_FLAG_REGEX).expect("valid regex");

    let is_kubesolo_flag = |flag_token: &str| -> bool {
        let flag_name = flag_token
            .strip_prefix("--")
            .unwrap_or(flag_token)
            .split_once('=')
            .map_or(
                flag_token.strip_prefix("--").unwrap_or(flag_token),
                |(k, _)| k,
            );
        flag_name == "full"
            || flag_name == "config"
            || lookup_flag(flag_name).is_some()
            || flag_name
                .strip_prefix("no-")
                .is_some_and(|n| matches!(lookup_flag(n), Some((_, FlagKind::Bool))))
    };

    let mut result_lines = Vec::new();
    let mut config_inserted = false;

    for line in content.lines() {
        if line.contains("kubesolo") {
            let mut rewritten = line.to_string();
            let matches: Vec<_> = flag_re.find_iter(line).collect();
            for mat in matches.into_iter().rev() {
                if is_kubesolo_flag(mat.as_str()) {
                    rewritten.replace_range(mat.range(), "");
                }
            }

            let has_continuation = rewritten.trim_end().ends_with('\\');
            let base = if has_continuation {
                rewritten
                    .trim_end()
                    .strip_suffix('\\')
                    .unwrap_or(&rewritten)
                    .trim_end()
            } else {
                rewritten.trim_end()
            };

            let final_line = if !config_inserted {
                config_inserted = true;
                if has_continuation {
                    format!("{base} --config={config_path} \\")
                } else {
                    format!("{base} --config={config_path}")
                }
            } else if has_continuation {
                format!("{base} \\")
            } else {
                base.to_string()
            };
            result_lines.push(final_line);
        } else if config_inserted && flag_re.is_match(line) {
            let mut rewritten = line.to_string();
            let matches: Vec<_> = flag_re.find_iter(line).collect();
            let mut had_kubesolo_flag = false;
            for mat in matches.into_iter().rev() {
                if is_kubesolo_flag(mat.as_str()) {
                    had_kubesolo_flag = true;
                    rewritten.replace_range(mat.range(), "");
                }
            }
            if had_kubesolo_flag {
                let trimmed = rewritten.trim_end_matches(['\\', ' ']).trim();
                if trimmed.is_empty() {
                    continue;
                }
                if line.trim_end().ends_with('\\') {
                    result_lines.push(format!("{trimmed} \\"));
                } else {
                    result_lines.push(trimmed.to_string());
                }
            } else {
                result_lines.push(line.to_string());
            }
        } else {
            result_lines.push(line.to_string());
        }
    }

    let mut res = result_lines.join("\n");
    if content.ends_with('\n') {
        res.push('\n');
    }
    res
}

/// Converts extracted legacy service flags into a validated `Config` model.
///
/// Does not read environment or host context by default unless provided.
pub fn flags_to_config(
    extracted: &ExtractedFlags,
    host: Option<&HostContext>,
) -> Result<Config, ConfigError> {
    let default_host = HostContext {
        cpu_count: 8,
        architecture: std::env::consts::ARCH.into(),
        detected_container_mode: false,
    };
    let host_ctx = host.unwrap_or(&default_host);
    let explicit = ExplicitFlags(extracted.flags.clone());
    let resolved = resolve_layers(
        None,
        &BTreeMap::new(),
        &explicit,
        crate::EnvironmentMode::Omit,
        host_ctx,
    )?;
    Ok(resolved.validated.into_config())
}

/// Converts extracted flags directly into rendered `KubeSolo` YAML string.
pub fn render_config_from_flags(
    extracted: &ExtractedFlags,
    host: Option<&HostContext>,
) -> Result<String, ConfigError> {
    let config = flags_to_config(extracted, host)?;
    render_effective_yaml(&config)
}

/// Outcome of a service configuration migration attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrationOutcome {
    /// Already migrated because `--config` is present in the service definition.
    AlreadyMigratedService,
    /// Destination config file already exists; skipped to prevent overwrite.
    DestinationConfigExists,
    /// No legacy flags found in service definition; nothing to migrate.
    NoLegacyFlags,
    /// Successfully migrated flags into configuration file.
    Migrated {
        destination: String,
        backup_created: bool,
    },
}

/// Migrates legacy service arguments into the specified config file (default `/etc/kubesolo/config.yaml`).
///
/// Follows safety guarantees:
/// 1. If service definition already specifies `--config`, skips (returns `AlreadyMigratedService`).
/// 2. If destination configuration file already exists, skips to avoid overwriting existing valid config
///    (returns `DestinationConfigExists`).
/// 3. If no legacy flags are present in service definition, returns `NoLegacyFlags`.
/// 4. Converts extracted flags to YAML and writes atomically using `write_document` (mode `0600`).
pub fn migrate_service_flags(
    service_content: &str,
    destination_path: Option<&Path>,
    host: Option<&HostContext>,
) -> Result<MigrationOutcome, PersistenceError> {
    let dest = destination_path.unwrap_or_else(|| Path::new(DEFAULT_CONFIG_PATH));

    // 1. Check if already migrated
    if has_config_flag(service_content) {
        return Ok(MigrationOutcome::AlreadyMigratedService);
    }

    // 2. Check if destination config file already exists
    if dest.exists() {
        return Ok(MigrationOutcome::DestinationConfigExists);
    }

    // 3. Extract legacy flags
    let extracted = extract_service_flags(service_content);
    if extracted.flags.is_empty() {
        return Ok(MigrationOutcome::NoLegacyFlags);
    }

    // 4. Convert flags to Config model
    let config = flags_to_config(&extracted, host).map_err(|source| PersistenceError {
        stage: crate::PersistenceStage::Render,
        path: dest.to_owned(),
        committed: false,
        source: crate::PersistenceSource::Render(source),
        cleanup_failures: Vec::new(),
    })?;

    // 5. Atomic write with 0600 permissions
    let outcome = write_document(dest, &config)?;

    Ok(MigrationOutcome::Migrated {
        destination: dest.display().to_string(),
        backup_created: outcome.backup_created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unquote_shell_value() {
        assert_eq!(unquote_shell_value("\"hello\""), "hello");
        assert_eq!(unquote_shell_value("'world'"), "world");
        assert_eq!(unquote_shell_value("\"\""), "");
        assert_eq!(unquote_shell_value("''"), "");
        assert_eq!(unquote_shell_value("plain"), "plain");
        assert_eq!(unquote_shell_value("  \"spaced\"  "), "spaced");
        assert_eq!(
            unquote_shell_value("\"nested 'quotes'\""),
            "nested 'quotes'"
        );
    }

    #[test]
    fn test_has_config_flag() {
        assert!(has_config_flag(
            "kubesolo --config=/etc/kubesolo/config.yaml"
        ));
        assert!(has_config_flag(
            "ExecStart=/usr/local/bin/kubesolo --config=\"/custom/path.yaml\""
        ));
        assert!(!has_config_flag(
            "ExecStart=/usr/local/bin/kubesolo --node-ip=1.2.3.4"
        ));
        assert!(!has_config_flag(
            "start-stop-daemon --start --exec /usr/local/bin/kubesolo"
        ));
    }

    #[test]
    fn test_extract_service_flags_various_formats() {
        let content = r#"
[Service]
ExecStart=/usr/local/bin/kubesolo \
  --path=/data/kubesolo \
  --debug \
  --node-ip="192.168.1.50" \
  --mtu=1400 \
  --no-local-storage \
  --portainer-edge-id='edge-123' \
  --portainer-edge-key="secret-key-abc"
"#;
        let extracted = extract_service_flags(content);
        assert_eq!(
            extracted.flags.get("path").map(String::as_str),
            Some("/data/kubesolo")
        );
        assert_eq!(
            extracted.flags.get("debug").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            extracted.flags.get("node-ip").map(String::as_str),
            Some("192.168.1.50")
        );
        assert_eq!(extracted.flags.get("mtu").map(String::as_str), Some("1400"));
        assert_eq!(
            extracted.flags.get("local-storage").map(String::as_str),
            Some("false")
        );
        assert_eq!(
            extracted.flags.get("portainer-edge-id").map(String::as_str),
            Some("edge-123")
        );
        assert_eq!(
            extracted
                .flags
                .get("portainer-edge-key")
                .map(String::as_str),
            Some("secret-key-abc")
        );
    }

    #[test]
    fn test_extract_ignores_foreign_init_flags() {
        // SysVinit start-stop-daemon line
        let content = r"
start-stop-daemon --start --quiet --pidfile /var/run/kubesolo.pid \
  --make-pidfile --background --exec /usr/local/bin/kubesolo -- \
  --node-ip=10.0.0.1 --debug
";
        let extracted = extract_service_flags(content);
        // Only KubeSolo flags captured
        assert_eq!(extracted.flags.len(), 2);
        assert_eq!(
            extracted.flags.get("node-ip").map(String::as_str),
            Some("10.0.0.1")
        );
        assert_eq!(
            extracted.flags.get("debug").map(String::as_str),
            Some("true")
        );
        assert!(!extracted.flags.contains_key("start"));
        assert!(!extracted.flags.contains_key("pidfile"));
        assert!(!extracted.flags.contains_key("exec"));
        assert!(!extracted.flags.contains_key("background"));
    }

    #[test]
    fn test_extract_first_occurrence_wins() {
        let content = "--node-ip=1.1.1.1 --mtu=1400 --node-ip=2.2.2.2";
        let extracted = extract_service_flags(content);
        assert_eq!(
            extracted.flags.get("node-ip").map(String::as_str),
            Some("1.1.1.1")
        );
        assert_eq!(extracted.flags.get("mtu").map(String::as_str), Some("1400"));
    }

    #[test]
    fn test_render_config_from_flags() {
        let content = "--node-ip=10.10.10.10 --debug --path=/var/lib/mykubesolo";
        let extracted = extract_service_flags(content);
        let yaml = render_config_from_flags(&extracted, None).expect("valid rendering");
        assert!(yaml.contains("apiVersion: kubesolo.io/v1alpha1"));
        assert!(yaml.contains("kind: Config"));
        assert!(yaml.contains("path: /var/lib/mykubesolo"));
        assert!(yaml.contains("debug: true"));
        assert!(yaml.contains("nodeIP: 10.10.10.10"));
    }

    #[test]
    fn test_migration_destination_exists_skips() {
        let dir =
            std::env::temp_dir().join(format!("rubix-test-mig-exists-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let config_file = dir.join("config.yaml");
        std::fs::write(&config_file, "existing config").unwrap();

        let service = "ExecStart=/usr/local/bin/kubesolo --node-ip=1.2.3.4";
        let outcome = migrate_service_flags(service, Some(&config_file), None).unwrap();
        assert_eq!(outcome, MigrationOutcome::DestinationConfigExists);

        // Original content unchanged
        assert_eq!(
            std::fs::read_to_string(&config_file).unwrap(),
            "existing config"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_migration_already_migrated_service_skips() {
        let dir =
            std::env::temp_dir().join(format!("rubix-test-mig-already-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let config_file = dir.join("config.yaml");

        let service = "ExecStart=/usr/local/bin/kubesolo --config=/etc/kubesolo/config.yaml";
        let outcome = migrate_service_flags(service, Some(&config_file), None).unwrap();
        assert_eq!(outcome, MigrationOutcome::AlreadyMigratedService);
        assert!(!config_file.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_migration_writes_atomic_0600() {
        let dir =
            std::env::temp_dir().join(format!("rubix-test-mig-atomic-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let config_file = dir.join("config.yaml");

        let service = "ExecStart=/usr/local/bin/kubesolo --node-ip=10.0.0.5 --debug";
        let outcome = migrate_service_flags(service, Some(&config_file), None).unwrap();
        assert_eq!(
            outcome,
            MigrationOutcome::Migrated {
                destination: config_file.display().to_string(),
                backup_created: false
            }
        );

        assert!(config_file.exists());
        let content = std::fs::read_to_string(&config_file).unwrap();
        assert!(content.contains("nodeIP: 10.0.0.5"));
        assert!(content.contains("debug: true"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&config_file).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
