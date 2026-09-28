#[path = "../node_fixture/mod.rs"]
mod node_fixture;
#[path = "../parity/mod.rs"]
pub mod parity;
#[path = "../platform_fixture/mod.rs"]
pub mod platform_fixture;
fn main() -> std::process::ExitCode {
    match node_fixture::main() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("node fixture: {error}");
            std::process::ExitCode::FAILURE
        },
    }
}
