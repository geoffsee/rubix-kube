#[path = "../supervisor_fixture/mod.rs"]
mod supervisor_fixture;
fn main() -> std::process::ExitCode {
    match supervisor_fixture::main() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("supervisor fixture: {error}");
            std::process::ExitCode::FAILURE
        },
    }
}
