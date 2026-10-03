use crate::CheckInputs;
use crate::completion::Shell;
use rubix_platform::Libc;
use std::fmt;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckOptions {
    pub install_prerequisites: bool,
    pub pprof: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompletionOptions {
    pub shell: Option<Shell>,
    pub raw_arg: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct DownloadOptions {
    pub version: String,
    pub path: PathBuf,
    pub arch: Option<String>,
    pub custom_url: Option<String>,
    pub temp_dir: Option<PathBuf>,
    pub proxy: Option<String>,
    pub offline: bool,
    pub libc: Option<Libc>,
}

impl fmt::Debug for DownloadOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DownloadOptions")
            .field("version", &self.version)
            .field("path", &self.path)
            .field("arch", &self.arch)
            .field(
                "custom_url",
                &self.custom_url.as_ref().map(|_| "<redacted>"),
            )
            .field("temp_dir", &self.temp_dir)
            .field("proxy", &self.proxy.as_ref().map(|_| "<redacted>"))
            .field("offline", &self.offline)
            .field("libc", &self.libc)
            .finish()
    }
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            version: crate::artifact::DEFAULT_VERSION.to_string(),
            path: PathBuf::from("."),
            arch: None,
            custom_url: None,
            temp_dir: None,
            proxy: None,
            offline: false,
            libc: None,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, PartialEq, Eq)]
pub struct InstallOptions {
    pub version: String,
    pub path: PathBuf,
    pub apiserver_extra_sans: Option<String>,
    pub node_ip: Option<String>,
    pub mtu: Option<String>,
    pub portainer_edge_id: Option<String>,
    pub portainer_edge_key: Option<String>,
    pub portainer_edge_async: bool,
    pub portainer_edge_image: Option<String>,
    pub local_storage: bool,
    pub debug: bool,
    pub pprof_server: bool,
    pub run_mode: String,
    pub proxy: Option<String>,
    pub offline_install: Option<PathBuf>,
    pub install_prereqs: bool,
    pub d2k: bool,
    pub d2k_namespace: String,
    pub cpu_manager_policy: String,
    pub cpu_manager_policy_options: Option<String>,
    pub reserved_cpus: Option<String>,
    pub system_reserved: Option<String>,
    pub container_image: Option<String>,
    pub container_ports: Option<String>,
    pub name: String,
    pub custom_url: Option<String>,
    pub temp_dir: Option<PathBuf>,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            version: crate::artifact::DEFAULT_VERSION.to_string(),
            path: PathBuf::from(crate::artifact::DEFAULT_DATA_PATH),
            apiserver_extra_sans: None,
            node_ip: None,
            mtu: None,
            portainer_edge_id: None,
            portainer_edge_key: None,
            portainer_edge_async: false,
            portainer_edge_image: None,
            local_storage: false,
            debug: false,
            pprof_server: false,
            run_mode: "service".to_string(),
            proxy: None,
            offline_install: None,
            install_prereqs: false,
            d2k: false,
            d2k_namespace: "d2k".to_string(),
            cpu_manager_policy: "none".to_string(),
            cpu_manager_policy_options: None,
            reserved_cpus: None,
            system_reserved: None,
            container_image: None,
            container_ports: None,
            name: "rubix".to_string(),
            custom_url: None,
            temp_dir: None,
        }
    }
}

impl fmt::Debug for InstallOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstallOptions")
            .field("version", &self.version)
            .field("path", &self.path)
            .field("apiserver_extra_sans", &self.apiserver_extra_sans)
            .field("node_ip", &self.node_ip)
            .field("mtu", &self.mtu)
            .field("portainer_edge_id", &self.portainer_edge_id)
            .field(
                "portainer_edge_key",
                &self.portainer_edge_key.as_ref().map(|_| "<redacted>"),
            )
            .field("portainer_edge_async", &self.portainer_edge_async)
            .field("portainer_edge_image", &self.portainer_edge_image)
            .field("local_storage", &self.local_storage)
            .field("debug", &self.debug)
            .field("pprof_server", &self.pprof_server)
            .field("run_mode", &self.run_mode)
            .field("proxy", &self.proxy.as_ref().map(|_| "<redacted>"))
            .field("offline_install", &self.offline_install)
            .field("install_prereqs", &self.install_prereqs)
            .field("d2k", &self.d2k)
            .field("d2k_namespace", &self.d2k_namespace)
            .field("cpu_manager_policy", &self.cpu_manager_policy)
            .field(
                "cpu_manager_policy_options",
                &self.cpu_manager_policy_options,
            )
            .field("reserved_cpus", &self.reserved_cpus)
            .field("system_reserved", &self.system_reserved)
            .field("container_image", &self.container_image)
            .field("container_ports", &self.container_ports)
            .field("name", &self.name)
            .field(
                "custom_url",
                &self.custom_url.as_ref().map(|_| "<redacted>"),
            )
            .field("temp_dir", &self.temp_dir)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UninstallOptions {
    pub path: PathBuf,
    pub purge: bool,
}

impl Default for UninstallOptions {
    fn default() -> Self {
        Self {
            path: PathBuf::from(crate::artifact::DEFAULT_DATA_PATH),
            purge: false,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct UpgradeOptions {
    pub version: String,
    pub path: PathBuf,
    pub offline_install: Option<PathBuf>,
    pub custom_url: Option<String>,
    pub proxy: Option<String>,
}

impl fmt::Debug for UpgradeOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UpgradeOptions")
            .field("version", &self.version)
            .field("path", &self.path)
            .field("offline_install", &self.offline_install)
            .field(
                "custom_url",
                &self.custom_url.as_ref().map(|_| "<redacted>"),
            )
            .field("proxy", &self.proxy.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl Default for UpgradeOptions {
    fn default() -> Self {
        Self {
            version: crate::artifact::DEFAULT_VERSION.to_string(),
            path: PathBuf::from(crate::artifact::DEFAULT_DATA_PATH),
            offline_install: None,
            custom_url: None,
            proxy: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResetOptions {
    pub path: PathBuf,
}

impl Default for ResetOptions {
    fn default() -> Self {
        Self {
            path: PathBuf::from(crate::artifact::DEFAULT_DATA_PATH),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigOptions {
    pub subcommand: Option<String>,
    pub key: Option<String>,
    pub value: Option<String>,
    pub file: Option<PathBuf>,
    pub environment: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KubeconfigOptions {
    pub subcommand: Option<String>,
    pub path: PathBuf,
    pub output: Option<PathBuf>,
}

impl Default for KubeconfigOptions {
    fn default() -> Self {
        Self {
            subcommand: None,
            path: PathBuf::from(crate::artifact::DEFAULT_DATA_PATH),
            output: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct D2kOptions {
    pub subcommand: Option<String>,
    pub path: PathBuf,
    pub output: Option<PathBuf>,
}

impl Default for D2kOptions {
    fn default() -> Self {
        Self {
            subcommand: None,
            path: PathBuf::from(crate::artifact::DEFAULT_DATA_PATH),
            output: None,
        }
    }
}

/// The command handler contract for Gate C05. Sibling tracks implement this trait
/// to wire their respective CLI commands into rubixctl.
pub trait CommandHandler {
    fn execute_check(
        &mut self,
        options: CheckOptions,
        inputs: &mut dyn CheckInputs,
        stderr: &mut dyn Write,
    ) -> io::Result<u8> {
        crate::execute_check(options, inputs, stderr)
    }

    fn execute_completion(
        &mut self,
        options: CompletionOptions,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> io::Result<u8> {
        crate::execute_completion(&options, stdout, stderr)
    }

    fn execute_download(
        &mut self,
        options: DownloadOptions,
        inputs: &mut dyn CheckInputs,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> io::Result<u8> {
        crate::execute_download(&options, inputs, stdout, stderr)
    }

    fn execute_install(
        &mut self,
        _options: InstallOptions,
        _inputs: &mut dyn CheckInputs,
        _stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> io::Result<u8> {
        writeln!(stderr, "error: command 'install' is not yet implemented")?;
        Ok(1)
    }

    fn execute_uninstall(
        &mut self,
        _options: UninstallOptions,
        _inputs: &mut dyn CheckInputs,
        _stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> io::Result<u8> {
        writeln!(stderr, "error: command 'uninstall' is not yet implemented")?;
        Ok(1)
    }

    fn execute_upgrade(
        &mut self,
        _options: UpgradeOptions,
        _inputs: &mut dyn CheckInputs,
        _stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> io::Result<u8> {
        writeln!(stderr, "error: command 'upgrade' is not yet implemented")?;
        Ok(1)
    }

    fn execute_reset(
        &mut self,
        _options: ResetOptions,
        _inputs: &mut dyn CheckInputs,
        _stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> io::Result<u8> {
        writeln!(stderr, "error: command 'reset' is not yet implemented")?;
        Ok(1)
    }

    fn execute_config(
        &mut self,
        options: ConfigOptions,
        inputs: &mut dyn CheckInputs,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> io::Result<u8> {
        crate::execute_config(&options, inputs, stdout, stderr)
    }

    fn execute_kubeconfig(
        &mut self,
        _options: KubeconfigOptions,
        _inputs: &mut dyn CheckInputs,
        _stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> io::Result<u8> {
        writeln!(stderr, "error: command 'kubeconfig' is not yet implemented")?;
        Ok(1)
    }

    fn execute_d2k(
        &mut self,
        _options: D2kOptions,
        _inputs: &mut dyn CheckInputs,
        _stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> io::Result<u8> {
        writeln!(stderr, "error: command 'd2k' is not yet implemented")?;
        Ok(1)
    }
}

/// Default command handler implementation for standalone `rubixctl` execution.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultCommandHandler;

impl CommandHandler for DefaultCommandHandler {}
