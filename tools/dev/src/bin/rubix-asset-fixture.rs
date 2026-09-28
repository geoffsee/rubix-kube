//! Offline fixture qualification and explicit disposable capture tooling.
#[path = "../asset_fixture/mod.rs"]
mod asset_fixture;
fn main() -> std::process::ExitCode {
    if let Err(error) = asset_fixture::main() {
        eprintln!("asset fixture: {error}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
