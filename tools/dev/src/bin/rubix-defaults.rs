fn main() {
    let raw: Vec<_> = std::env::args_os().skip(1).collect();
    if raw.first().is_some_and(|v| v == "__exec") {
        match rubix_dev::process::child_exec(&raw) {
            Ok(code) => std::process::exit(i32::from(code)),
            Err(error) => {
                eprintln!("child execution: {error}");
                std::process::exit(125);
            },
        }
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.split_first() {
        Some((command, rest)) if command == "verify" => rubix_dev::defaults::verify_cli(rest),
        Some((command, rest)) if command == "capture" => rubix_dev::defaults::capture::cli(rest),
        Some((command, rest)) if command == "verify-evidence" && rest.is_empty() => {
            let directory = rubix_dev::defaults::directory();
            rubix_dev::defaults::capture::verify_evidence(
                &directory,
                &directory.join("evidence"),
                false,
            )
            .map(|()| 0)
        },
        _ => Err("usage: rubix-defaults <capture|verify|verify-evidence> ...".into()),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("defaults: {error}");
            std::process::exit(2);
        },
    }
}
