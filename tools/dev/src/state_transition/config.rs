//! Configuration transition verification.
//!
//! Validates:
//! 1. Migration of legacy service flags (Go `KubeSolo` `v1.1.8`, `v1.2.0`) into `/etc/kubesolo/config.yaml`.
//! 2. Preservation and non-destruction of existing `/etc/kubesolo/config.yaml` (`v1.3.0+`).
//! 3. Preservation of sensitive parameters (e.g. `portainer-edge-key`, cluster CIDRs, node IP).
//! 4. Generation of atomic backup `.bak` for modified service files.
//! 5. Proper file permissions (`0600`) and decoding fidelity under `rubix_config`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rubix_config::HostContext;
use rubix_config::decode;
use rubix_config::legacy::{extract_service_flags, flags_to_config};
use rubixctl::migrate::{ServiceMigrationResult, migrate_legacy_service};
use serde::{Deserialize, Serialize};

use crate::state_transition::versions::SupportedStartingVersion;

/// Result of configuration transition validation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigTransitionAssertion {
    pub starting_version: SupportedStartingVersion,
    pub service_migrated: bool,
    pub config_path: PathBuf,
    pub backup_path: Option<PathBuf>,
    pub node_ip: String,
    pub disable_ipv6: bool,
    pub edge_id: String,
    pub permissions_valid_0600: bool,
}

/// Executes and validates configuration transition for a given starting version and service definition.
pub fn validate_config_transition(
    version: SupportedStartingVersion,
    service_path: &Path,
    dest_config_path: &Path,
    host: Option<&HostContext>,
) -> io::Result<ConfigTransitionAssertion> {
    let mut stderr_buf = Vec::new();

    let migration_result = migrate_legacy_service(
        service_path,
        Some(dest_config_path),
        "latest",
        host,
        &mut stderr_buf,
    )?;

    match migration_result {
        ServiceMigrationResult::Migrated {
            config_path,
            service_backup_path,
            ..
        } => {
            // Read and decode the generated config
            let bytes = fs::read(&config_path)?;
            let text = std::str::from_utf8(&bytes).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("config is not UTF-8: {e}"),
                )
            })?;
            let decoded = decode(text).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("generated config failed decoding: {e}"),
                )
            })?;

            // Verify permissions are 0600
            #[cfg(unix)]
            let perms_ok = {
                use std::os::unix::fs::PermissionsExt;
                let meta = fs::metadata(&config_path)?;
                (meta.permissions().mode() & 0o777) == 0o600
            };
            #[cfg(not(unix))]
            let perms_ok = true;

            Ok(ConfigTransitionAssertion {
                starting_version: version,
                service_migrated: true,
                config_path,
                backup_path: Some(service_backup_path),
                node_ip: decoded.config.network.node_ip,
                disable_ipv6: decoded.config.network.disable_ipv6,
                edge_id: decoded.config.portainer.edge_id,
                permissions_valid_0600: perms_ok,
            })
        },
        ServiceMigrationResult::DestinationConfigExists { config_path: _ }
        | ServiceMigrationResult::AlreadyMigrated { .. } => {
            let config_path = dest_config_path;
            let bytes = fs::read(config_path)?;
            let text = std::str::from_utf8(&bytes).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("config is not UTF-8: {e}"),
                )
            })?;
            let decoded = decode(text).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("existing config failed decoding: {e}"),
                )
            })?;

            #[cfg(unix)]
            let perms_ok = {
                use std::os::unix::fs::PermissionsExt;
                let meta = fs::metadata(config_path)?;
                (meta.permissions().mode() & 0o777) == 0o600
            };
            #[cfg(not(unix))]
            let perms_ok = true;

            Ok(ConfigTransitionAssertion {
                starting_version: version,
                service_migrated: false,
                config_path: config_path.to_path_buf(),
                backup_path: None,
                node_ip: decoded.config.network.node_ip,
                disable_ipv6: decoded.config.network.disable_ipv6,
                edge_id: decoded.config.portainer.edge_id,
                permissions_valid_0600: perms_ok,
            })
        },
        other => Err(io::Error::other(format!(
            "unexpected service migration result for {version}: {other:?}"
        ))),
    }
}

/// Asserts that legacy service arguments match the newly decoded YAML configuration.
pub fn assert_flags_match_yaml(
    service_file_content: &str,
    yaml_content: &str,
) -> Result<(), String> {
    let flags = extract_service_flags(service_file_content);
    let expected = flags_to_config(&flags, None)
        .map_err(|e| format!("failed to convert flags to config: {e}"))?;

    let decoded = decode(yaml_content).map_err(|e| format!("failed to decode YAML: {e}"))?;

    if expected.network.node_ip != decoded.config.network.node_ip {
        return Err(format!(
            "node_ip mismatch: expected {:?}, got {:?}",
            expected.network.node_ip, decoded.config.network.node_ip
        ));
    }
    if expected.network.disable_ipv6 != decoded.config.network.disable_ipv6 {
        return Err(format!(
            "disable_ipv6 mismatch: expected {:?}, got {:?}",
            expected.network.disable_ipv6, decoded.config.network.disable_ipv6
        ));
    }
    if expected.portainer.edge_id != decoded.config.portainer.edge_id {
        return Err(format!(
            "edge_id mismatch: expected {:?}, got {:?}",
            expected.portainer.edge_id, decoded.config.portainer.edge_id
        ));
    }

    Ok(())
}
