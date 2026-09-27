//! Internal code generator. `upstream.py` verifies all inputs before invoking it.

use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "usage: rubix-upstream-codegen <api.proto> <verified-protoc> <output-dir>".into(),
        );
    }
    let proto = PathBuf::from(&args[0]);
    let protoc = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let include = proto
        .parent()
        .ok_or("protocol input has no parent directory")?;
    std::fs::create_dir_all(&output)?;
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc);
    config.btree_map(["."]);
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(false)
        .emit_rerun_if_changed(false)
        .out_dir(output)
        .compile_with_config(config, &[proto.as_path()], &[include])?;
    Ok(())
}
