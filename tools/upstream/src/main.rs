//! Internal code generator. `rubix-upstream` verifies all inputs before invoking it.

use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let containerd = args.first().is_some_and(|value| value == "--containerd");
    let values = if containerd { &args[1..] } else { &args[..] };
    if values.len() != 3 {
        return Err(
            "usage: rubix-upstream-codegen [--containerd] <input> <verified-protoc> <output-dir>"
                .into(),
        );
    }
    let input = PathBuf::from(&values[0]);
    let compiler = PathBuf::from(&values[1]);
    let output = PathBuf::from(&values[2]);
    let (protos, include) =
        if containerd {
            let mut protos: Vec<_> = [
                "containers",
                "content",
                "diff",
                "events",
                "images",
                "leases",
                "snapshots",
                "tasks",
                "transfer",
                "version",
            ]
            .iter()
            .map(|service| {
                input.join(format!(
                    "github.com/containerd/containerd/api/services/{service}/v1/{service}.proto"
                ))
            })
            .collect();
            protos.push(input.join(
                "github.com/containerd/containerd/api/services/namespaces/v1/namespace.proto",
            ));
            (protos, input)
        } else {
            let include = input
                .parent()
                .ok_or("protocol input has no parent directory")?
                .to_path_buf();
            (vec![input], include)
        };
    std::fs::create_dir_all(&output)?;
    let mut config = prost_build::Config::new();
    config.protoc_executable(compiler);
    config.btree_map(["."]);
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(false)
        .emit_rerun_if_changed(false)
        .out_dir(output)
        .compile_with_config(config, &protos, &[include])?;
    Ok(())
}
