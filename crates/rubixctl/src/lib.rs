//! Read-only management command boundary, independent from node startup parsing.
mod parse;
pub use parse::{CheckOptions, Command, HelpTopic, ParseError, parse_command};
use rubix_platform::preflight::{
    CheckId, CheckStatus, Finding, PortAvailability, PreflightInputs, RuntimeOwnership,
    evaluate_preflight,
};
use rubix_platform::preflight_probe::SupplementalFacts;
use rubix_platform::{HostEvidence, Observation, PlatformError, ProbeFailure};
use std::collections::BTreeMap;
use std::io::{self, Write};

/// Injected host effects. Implementations must never prepare a host or stop listeners.
pub trait CheckInputs {
    fn discover(&mut self) -> Result<HostEvidence, PlatformError>;
    fn supplemental(&mut self) -> Result<SupplementalFacts, PlatformError>;
    fn ports(&mut self, pprof: bool) -> Result<[Observation<PortAvailability>; 4], PlatformError>;
}
fn help(topic: HelpTopic) -> &'static str {
    match topic {
        HelpTopic::Root => {
            "Rubix Kube management commands\n\nUsage:\n  rubixctl [command]\n\nCommands:\n  check    Run host pre-flight checks without installing\n  version  Print version information\n\nUse rubixctl check --help for check options.\n"
        },
        HelpTopic::Check => {
            "Run Linux host pre-flight checks without installing.\n\nUsage:\n  rubixctl check [flags]\n\nFlags:\n  -h, --help             Help for check\n      --install-prereqs  Request prerequisite preparation (currently unsupported)\n      --pprof-server     Include pprof port 6060 in port checks\n"
        },
        HelpTopic::Version => "Print rubixctl version information\n\nUsage:\n  rubixctl version\n",
    }
}
/// Parses and executes a bounded read-only command. Diagnostics do not echo arguments,
/// environment values, hostname, file contents or raw OS errors. Output failures propagate.
pub fn execute(
    args: &[String],
    environment: &BTreeMap<String, String>,
    version: &str,
    inputs: &mut dyn CheckInputs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<u8> {
    match parse_command(args, environment) {
        Ok(Command::Help(topic)) => {
            stdout.write_all(help(topic).as_bytes())?;
            Ok(0)
        },
        Ok(Command::UnknownHelp) => {
            writeln!(stderr, "Unknown help topic")?;
            stderr.write_all(help(HelpTopic::Root).as_bytes())?;
            Ok(0)
        },
        Ok(Command::Version) => {
            writeln!(stdout, "rubixctl {version}")?;
            Ok(0)
        },
        Ok(Command::Check(options)) => execute_check(options, inputs, stderr),
        Err(error) => {
            writeln!(stderr, "error: {}", error.message())?;
            Ok(1)
        },
    }
}
fn platform_failure(stderr: &mut dyn Write, stage: &str, error: PlatformError) -> io::Result<u8> {
    writeln!(
        stderr,
        "  [fail] {stage}: {error}; obtain supported host observations before retrying"
    )?;
    Ok(1)
}
fn check_name(id: CheckId) -> &'static str {
    match id {
        CheckId::Root => "root privileges",
        CheckId::Hostname => "hostname RFC 1123 compliance",
        CheckId::DockerConflict => "Docker conflict",
        CheckId::XtablesComment => "iptables xt_comment module",
        CheckId::AlpineNetworking => "nftables and iptables",
        CheckId::Cgroups => "cgroups controllers",
        CheckId::Ports => "required ports available",
    }
}
/// Runs supplied checks with no preparation and no port probe after an earlier failure.
pub fn execute_check(
    options: CheckOptions,
    inputs: &mut dyn CheckInputs,
    stderr: &mut dyn Write,
) -> io::Result<u8> {
    if options.install_prerequisites {
        writeln!(
            stderr,
            "error: prerequisite preparation is not implemented; rerun with --install-prereqs=false for read-only checks"
        )?;
        return Ok(1);
    }
    writeln!(stderr, "\n  rubixctl  check\n\n  > Detecting system")?;
    let evidence = match inputs.discover() {
        Ok(value) => value,
        Err(error) => return platform_failure(stderr, "system detection", error),
    };
    if evidence.executable.os != "linux" {
        return platform_failure(stderr, "system detection", PlatformError::UnsupportedHost);
    }
    if !["x86_64", "aarch64", "arm", "riscv64"].contains(&evidence.executable.architecture.as_str())
    {
        return platform_failure(stderr, "system detection", PlatformError::UnsupportedTarget);
    }
    writeln!(stderr, "  [ok] System detected\n\n  > Pre-flight checks")?;
    let supplement = match inputs.supplemental() {
        Ok(value) => value,
        Err(error) => return platform_failure(stderr, "supplemental observations", error),
    };
    let mut policy = PreflightInputs {
        install_prerequisites: false,
        pprof: options.pprof,
        runtime: RuntimeOwnership::Managed,
        xt_comment_on_disk: supplement.xt_comment_on_disk,
        alpine_rc_service: supplement.alpine_rc_service,
        ports: [Observation::Unknown(ProbeFailure::Malformed); 4],
    };
    let initial = evaluate_preflight(&evidence, &policy);
    for finding in &initial.findings[..6] {
        if !render_finding(finding, stderr)? {
            return Ok(1);
        }
    }
    policy.ports = match inputs.ports(options.pprof) {
        Ok(value) => value,
        Err(error) => return platform_failure(stderr, "port observations", error),
    };
    let report = evaluate_preflight(&evidence, &policy);
    if !render_finding(&report.findings[6], stderr)? {
        return Ok(1);
    }
    writeln!(
        stderr,
        "  [ok] All 7 checks passed\n\n  Observed host prerequisites passed; ports are not reserved and runtime readiness is not established.\n"
    )?;
    Ok(0)
}

fn render_finding(finding: &Finding, stderr: &mut dyn Write) -> io::Result<bool> {
    if matches!(
        finding.status,
        CheckStatus::Pass | CheckStatus::NotApplicable
    ) {
        writeln!(stderr, "     {}", check_name(finding.check))?;
        Ok(true)
    } else {
        writeln!(
            stderr,
            "  [fail] pre-flight checks: {}: {:?}; remediation={:?}; missing={:?}; uncertainty={:?}",
            check_name(finding.check),
            finding.reason,
            finding.remediation,
            finding.missing,
            finding.uncertainty
        )?;
        Ok(false)
    }
}
