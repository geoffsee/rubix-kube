//! Static workloads, Kubernetes resource identities, and nonportable state classification.
//!
//! Validates:
//! 1. Preservation of Kubernetes core resource identities:
//!    - UIDs (`metadata.uid`)
//!    - Namespaces and names
//!    - Resource versions (`metadata.resourceVersion`)
//!    - Spec specifications (containers, ports, volumes, images)
//! 2. Preservation of static pod manifests (e.g. `/etc/kubernetes/manifests`).
//! 3. Explicit classification of nonportable transient runtime state:
//!    - Sockets: `containerd.sock`, `config.sock`, `kine.sock`
//!    - PIDs: `/var/run/kubesolo.pid`
//!    - Runtime task shims: `containerd-shim-runc-v2`
//!    - Network iptables/nftables masquerade rules: `kubesolo-masq`
//!
//!    These must be recreated by the supervisor and not assumed portable across transitions.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Canonical representation of a Kubernetes resource object for identity preservation testing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadIdentity {
    pub api_version: String,
    pub kind: String,
    pub namespace: String,
    pub name: String,
    pub uid: String,
    pub resource_version: String,
    pub spec_hash: String,
}

/// Category of host/runtime state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatePortabilityCategory {
    /// State that MUST be preserved verbatim across transition (config, PKI, datastore, PVs).
    PortablePersistent,
    /// Ephemeral runtime state that CANNOT be preserved and must be recreated (sockets, PIDs, shims).
    NonportableEphemeral,
}

/// Item in the state portability classification registry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateClassificationItem {
    pub path_or_resource: String,
    pub category: StatePortabilityCategory,
    pub justification: String,
}

/// Returns the authoritative state classification inventory.
#[must_use]
pub fn state_classification_inventory() -> Vec<StateClassificationItem> {
    vec![
        StateClassificationItem {
            path_or_resource: "/etc/kubesolo/config.yaml".into(),
            category: StatePortabilityCategory::PortablePersistent,
            justification: "Desired node configuration; survives upgrade and reset.".into(),
        },
        StateClassificationItem {
            path_or_resource: "P/pki/ca/ca.crt".into(),
            category: StatePortabilityCategory::PortablePersistent,
            justification: "Root CA certificate; maintains cluster cryptographic trust.".into(),
        },
        StateClassificationItem {
            path_or_resource: "P/pki/admin/admin.kubeconfig".into(),
            category: StatePortabilityCategory::PortablePersistent,
            justification: "Admin client credentials; allows ongoing cluster access.".into(),
        },
        StateClassificationItem {
            path_or_resource: "P/kine/db/state.db".into(),
            category: StatePortabilityCategory::PortablePersistent,
            justification: "Durable SQLite database containing all Kubernetes state.".into(),
        },
        StateClassificationItem {
            path_or_resource: "P/local-path-storage".into(),
            category: StatePortabilityCategory::PortablePersistent,
            justification: "Persistent volume directory storing application workloads.".into(),
        },
        StateClassificationItem {
            path_or_resource: "P/containerd/containerd.sock".into(),
            category: StatePortabilityCategory::NonportableEphemeral,
            justification: "UNIX domain socket owned by running containerd process.".into(),
        },
        StateClassificationItem {
            path_or_resource: "P/config.sock".into(),
            category: StatePortabilityCategory::NonportableEphemeral,
            justification: "Config API UNIX domain socket; must be unbound and recreated.".into(),
        },
        StateClassificationItem {
            path_or_resource: "/var/run/kubesolo.pid".into(),
            category: StatePortabilityCategory::NonportableEphemeral,
            justification: "Process identifier file; invalid after service stop.".into(),
        },
        StateClassificationItem {
            path_or_resource: "P/containerd/state".into(),
            category: StatePortabilityCategory::NonportableEphemeral,
            justification: "Ephemeral container task state; rebuilt on supervisor start.".into(),
        },
        StateClassificationItem {
            path_or_resource: "iptables/kubesolo-masq".into(),
            category: StatePortabilityCategory::NonportableEphemeral,
            justification: "Kernel firewall rules; re-established by rubix-network.".into(),
        },
    ]
}

/// Asserts that all workload identities before transition match the identities after transition.
pub fn assert_workload_identities_preserved(
    before: &[WorkloadIdentity],
    after: &[WorkloadIdentity],
) -> Result<(), String> {
    let mut before_map: BTreeMap<(&str, &str, &str), &WorkloadIdentity> = BTreeMap::new();
    for item in before {
        before_map.insert((&item.kind, &item.namespace, &item.name), item);
    }

    if before.len() != after.len() {
        return Err(format!(
            "workload count mismatch: before={}, after={}",
            before.len(),
            after.len()
        ));
    }

    for item in after {
        let key = (
            item.kind.as_str(),
            item.namespace.as_str(),
            item.name.as_str(),
        );
        match before_map.get(&key) {
            Some(prev) => {
                if prev.uid != item.uid {
                    return Err(format!(
                        "UID changed for {key:?}: before={}, after={}",
                        prev.uid, item.uid
                    ));
                }
                if prev.resource_version != item.resource_version {
                    return Err(format!(
                        "resourceVersion changed for {key:?}: before={}, after={}",
                        prev.resource_version, item.resource_version
                    ));
                }
                if prev.spec_hash != item.spec_hash {
                    return Err(format!(
                        "specHash changed for {key:?}: before={}, after={}",
                        prev.spec_hash, item.spec_hash
                    ));
                }
            },
            None => {
                return Err(format!("unexpected new workload after transition: {key:?}"));
            },
        }
    }

    Ok(())
}

/// Asserts static pod manifests in manifests directory are preserved before and after transition.
pub fn assert_static_manifests_preserved(dir_before: &Path, dir_after: &Path) -> io::Result<()> {
    if !dir_before.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir_before)? {
        let entry = entry?;
        let name = entry.file_name();
        let target = dir_after.join(&name);
        if !target.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "static manifest {} missing after transition",
                    name.to_string_lossy()
                ),
            ));
        }

        let bytes_before = fs::read(entry.path())?;
        let bytes_after = fs::read(&target)?;
        if bytes_before != bytes_after {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "static manifest {} content mismatch",
                    name.to_string_lossy()
                ),
            ));
        }
    }

    Ok(())
}
