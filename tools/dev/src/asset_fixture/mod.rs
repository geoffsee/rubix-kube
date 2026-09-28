mod archive;
mod capture;
mod common;
mod elf;
mod native;
#[cfg(test)]
mod tests;
use common::{Result, root};
pub(super) fn main() -> Result<()> {
    let args_os = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args_os.first().is_some_and(|argument| argument == "__exec") {
        rubix_dev::process::child_exec(&args_os)?;
        return Ok(());
    }
    let args = args_os
        .into_iter()
        .map(|argument| argument.into_string().map_err(|_| "argument UTF8".into()))
        .collect::<Result<Vec<_>>>()?;
    let _signals = if args
        .iter()
        .any(|argument| argument == "capture" || argument == "archive-runtime")
    {
        Some(rubix_dev::process::SignalGuard::install(
            common::cancellation(),
        )?)
    } else {
        None
    };

    match args.as_slice() {
        [command] if command == "namespace" => capture::namespace(),
        [command] if command == "archive-runtime" => archive::runtime(),
        [command, actual, pinned] if command == "graph" => archive::graph(actual.as_ref(), pinned.as_ref()),
        [family, command, directory] if command == "verify" => capture::verify(&root()?, family, directory.as_ref()),
        [family, command, directory] if command == "capture" && family != "elf" => capture::docker(&root()?, family, directory.as_ref()),
        [family, command, directory, cache] if family == "elf" && command == "capture" => elf::capture(&root()?, directory.as_ref(), cache.as_ref()),
        _ => Err("usage: rubix-asset-fixture {elf|decode|archive} verify DIRECTORY | {decode|archive} capture DIRECTORY | elf capture DIRECTORY CACHE | archive-runtime | namespace | graph ACTUAL PINNED".into()),
    }
}
