//! Rust-only parity execution and owned disposable-VM tooling.
#[path = "../parity/mod.rs"]
pub mod parity;

fn main() -> std::process::ExitCode {
    match parity::main(&std::env::args_os().skip(1).collect::<Vec<_>>()) {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("rubix-parity: {error}");
            std::process::ExitCode::FAILURE
        },
    }
}
