//! Rust qualification tools for startup and owned Alpine guests.
#[path = "../parity/mod.rs"]
pub mod parity;
#[path = "../platform_fixture/mod.rs"]
pub mod platform_fixture;
fn main() -> std::process::ExitCode {
    match platform_fixture::main(&std::env::args_os().skip(1).collect::<Vec<_>>()) {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("rubix-platform-fixture: {error}");
            std::process::ExitCode::FAILURE
        },
    }
}
