//! Managed containerd runtime integration for Rubix Kube.
//!
//! Provides generation of containerd v3 configuration, runtime and CRI
//! endpoint settings, shim lookup via PATH resolution, snapshotter selection,
//! registry `hosts.toml` configuration, and immutable symlink handling.

pub mod cgroup;
pub mod config;
pub mod registry;
pub mod shim;
pub mod snapshotter;
pub mod symlink;

pub use cgroup::{evaluate_systemd_cgroup, is_cgroup_v2, is_systemd_running, use_systemd_cgroup};
pub use config::{
    CONTAINERD_CONFIG_VERSION, ContainerdConfigFile, ContainerdConfigOptions,
    ContainerdServicePaths, DEFAULT_CONTAINERD_CONFIG_DIR, DEFAULT_RUNTIME_NAME,
    DEFAULT_SANDBOX_IMAGE, DEFAULT_STANDARD_CNI_CONF_DIR, generate_containerd_config,
    render_containerd_config, write_containerd_config_file,
};
pub use registry::{
    HostEndpointConfig, RegistryError, RegistryHostFile, ensure_registry_dir,
    list_configured_registries, read_registry_hosts_toml, write_registry_hosts_toml,
};
pub use shim::{
    DEFAULT_SHIM_BINARY_NAME, RUNTIME_RUNC_V2, ShimError, build_containerd_path, find_shim_in_path,
    validate_runtime_spec,
};
pub use snapshotter::{
    OVERLAYFS_SUPER_MAGIC, Snapshotter, check_fuse_overlayfs_in_path, detect_snapshotter,
    is_overlayfs, select_snapshotter,
};
pub use symlink::{
    DEFAULT_SYSTEM_CONTAINERD_SOCK, SymlinkError, ensure_symbolic_link, ensure_system_socket_link,
};
