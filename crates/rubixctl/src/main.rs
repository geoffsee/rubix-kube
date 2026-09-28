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
    let parsed = rubixctl::parse_command(&args, &environment);
    if let Ok(rubixctl::Command::Check(options)) = parsed
        && options.install_prerequisites
    {
        return preparation(options);
    }
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

fn preparation(options: rubixctl::CheckOptions) -> std::process::ExitCode {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        let _ = writeln!(
            io::stderr(),
            "error: prerequisite runtime could not be initialized"
        );
        return std::process::ExitCode::FAILURE;
    };
    let cancellation = rubixctl::preparation::Cancellation::default();
    let mut executor = rubixctl::preparation::AlpinePreparation;
    let mut host = Host;
    let mut stderr = io::stderr().lock();
    let workflow = rubixctl::check_workflow::execute_check_with_preparation(
        options,
        &mut host,
        &mut executor,
        &cancellation,
        &mut stderr,
    );
    match runtime.block_on(rubixctl::preparation::with_signals(&cancellation, workflow)) {
        Ok(Ok(code)) => code.into(),
        Ok(Err(_)) => {
            let _ = writeln!(stderr, "error: management output failed");
            std::process::ExitCode::FAILURE
        },
        Err(_) => {
            let _ = writeln!(
                stderr,
                "error: prerequisite signal listeners could not be installed; no checks or preparation were started"
            );
            std::process::ExitCode::FAILURE
        },
    }
}
