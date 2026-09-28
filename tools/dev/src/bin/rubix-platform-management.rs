fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "__exec") {
        return match rubix_dev::process::child_exec(&args) {
            Ok(code) => code.into(),
            Err(_) => 125.into(),
        };
    }
    if args.first().is_some_and(|a| a == "__chroot") {
        let _ = rubix_dev::platform_management::linux::chroot_exec(&args[1..]);
        return 125.into();
    }
    let result = (|| -> rubix_dev::Result<()> {
        let args = args
            .into_iter()
            .map(|arg| arg.into_string().map_err(|_| "argument encoding".into()))
            .collect::<rubix_dev::Result<Vec<_>>>()?;
        if args == ["management-runtime"] {
            return rubix_dev::platform_management::linux::runtime();
        }
        let root = rubix_dev::repository_root(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))?;
        match args.as_slice() {
            [command, family] if command == "verify" => rubix_dev::platform_management::verify_historical(&root, family),
            [command, family, flag, path] if matches!(command.as_str(), "capture-go" | "qualify-linux") && flag == "--output" => rubix_dev::platform_management::capture::run(&root, family, command == "qualify-linux", path.as_ref()),
            [command, family, path] if matches!(command.as_str(), "verify-go" | "verify-linux") => rubix_dev::platform_management::capture::verify(&root, family, command == "verify-linux", path.as_ref()),
            _ => Err("usage: verify FAMILY | capture-go/qualify-linux FAMILY --output DIR | verify-go/verify-linux FAMILY DIR".into()),
        }
    })();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("qualification failed: {error}");
            std::process::ExitCode::FAILURE
        },
    }
}
