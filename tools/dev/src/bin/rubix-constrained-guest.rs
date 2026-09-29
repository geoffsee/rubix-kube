#[path = "../node_fixture/constrained_guest.rs"]
mod constrained_guest;
#[path = "../parity/process.rs"]
pub mod process;

fn main() -> std::process::ExitCode {
    match constrained_guest::main() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("constrained guest: {error}");
            std::process::ExitCode::FAILURE
        },
    }
}
