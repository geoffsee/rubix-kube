use rubix_platform::preflight::PortAvailability;
use rubix_platform::preflight_probe::{SupplementalFacts, collect_supplemental, probe_ports};
use rubix_platform::{
    DiscoveryRequest, HostEvidence, Observation, PlatformError, ProbeLimits, discover,
};
use std::collections::BTreeMap;
use std::io::{self, Write};
struct Host;
impl rubixctl::CheckInputs for Host {
    fn discover(&mut self) -> Result<HostEvidence, PlatformError> {
        discover(&DiscoveryRequest::default())
    }
    fn supplemental(&mut self) -> Result<SupplementalFacts, PlatformError> {
        collect_supplemental(ProbeLimits::default())
    }
    fn ports(&mut self, pprof: bool) -> Result<[Observation<PortAvailability>; 4], PlatformError> {
        probe_ports(pprof)
    }
}
fn main() -> std::process::ExitCode {
    let Ok(args): Result<Vec<String>, _> = std::env::args_os()
        .skip(1)
        .map(std::ffi::OsString::into_string)
        .collect()
    else {
        let _ = writeln!(io::stderr(), "error: command arguments must be UTF-8");
        return std::process::ExitCode::FAILURE;
    };
    let environment: BTreeMap<String, String> =
        ["KUBESOLO_INSTALL_PREREQS", "KUBESOLO_PPROF_SERVER"]
            .into_iter()
            .filter_map(|key| std::env::var(key).ok().map(|value| (key.into(), value)))
            .collect();
    if let Ok(code) = rubixctl::execute(
        &args,
        &environment,
        env!("CARGO_PKG_VERSION"),
        &mut Host,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ) {
        code.into()
    } else {
        let _ = writeln!(io::stderr(), "error: management output failed");
        std::process::ExitCode::FAILURE
    }
}
