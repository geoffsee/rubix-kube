fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "__exec") {
        return match rubix_dev::process::child_exec(&args) {
            Ok(code) => std::process::ExitCode::from(code),
            Err(_) => std::process::ExitCode::from(125),
        };
    }
    let result = (|| -> rubix_dev::Result<()> {
        let args = args
            .into_iter()
            .map(|arg| arg.into_string().map_err(|_| "argument UTF8".into()))
            .collect::<rubix_dev::Result<Vec<_>>>()?;
        rubix_dev::component_boundary::cli("component-boundary", &args)
    })();
    if let Err(error) = result {
        eprintln!("component boundary: {error}");
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}
