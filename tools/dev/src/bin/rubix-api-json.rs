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
        match args.as_slice() {
            [command, path] if command == "verify" => {
                rubix_dev::api_json::verify(&rubix_dev::api_json::load(path.as_ref())?)
            },
            [command, path] if command == "check-capture" => {
                println!("{}", rubix_dev::api_json::check_capture(path.as_ref())?);
                Ok(())
            },
            _ => rubix_dev::component_boundary::cli("api-json", &args),
        }
    })();
    if result.is_err() {
        eprintln!(
            "API capture validation failed; inspect the owned capture and source identity locally"
        );
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}
