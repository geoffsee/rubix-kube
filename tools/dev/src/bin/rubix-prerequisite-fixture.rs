//! Disposable prerequisite preparation qualification.
#[path = "../parity/mod.rs"]
#[allow(
    dead_code,
    reason = "Shared adapter module includes unrelated qualification commands"
)]
pub mod parity;
#[path = "../platform_fixture/mod.rs"]
#[allow(
    dead_code,
    reason = "Shared verification module includes unrelated qualification commands"
)]
pub mod platform_fixture;
#[path = "../prerequisite_fixture/mod.rs"]
mod prerequisite_fixture;
fn main() -> std::process::ExitCode {
    match prerequisite_fixture::main(&std::env::args_os().skip(1).collect::<Vec<_>>()) {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("rubix-prerequisite-fixture: {error}");
            std::process::ExitCode::FAILURE
        },
    }
}
