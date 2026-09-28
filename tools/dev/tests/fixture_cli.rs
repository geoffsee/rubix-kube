use rubix_dev::{Result, fixture_oracles as oracle};
use serde_json::{Value, json};
use std::{path::Path, process::Command};

fn verify(family: &str, path: &Path) -> Result<bool> {
    Ok(Command::new(env!("CARGO_BIN_EXE_rubix-fixture"))
        .args(["verify", family])
        .arg(path)
        .output()?
        .status
        .success())
}

fn write(path: &Path, value: &Value) -> Result<()> {
    std::fs::write(path, serde_json::to_vec(value)?)?;
    Ok(())
}

#[test]
fn verifier_cli_rejects_wrong_components_types_missing_families_and_malformed_json() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let apiserver = oracle::credentials::expected("apiserver")?;
    let kubelet = oracle::credentials::expected("kubelet")?;
    for (name, value) in [("apiserver", &apiserver), ("kubelet", &kubelet)] {
        write(&root.join(format!("{name}.json")), value)?;
    }
    assert!(verify("credentials", root)?);
    for (name, wrong) in [("apiserver", &kubelet), ("kubelet", &apiserver)] {
        let path = root.join(format!("{name}.json"));
        write(&path, wrong)?;
        assert!(!verify("credentials", root)?);
        write(&path, &oracle::credentials::expected(name)?)?;
    }
    let mut changed = apiserver;
    changed["checks"]["key_valid"] = 1.into();
    write(&root.join("apiserver.json"), &changed)?;
    assert!(!verify("credentials", root)?);
    let mapping_path = root.join("mapping.json");
    let mut mapping = oracle::mapping::expected();
    write(&mapping_path, &mapping)?;
    assert!(verify("runtime-mapping", &mapping_path)?);
    mapping[0]["embedded"]["RuntimeExternal"] = 1.into();
    write(&mapping_path, &mapping)?;
    assert!(!verify("runtime-mapping", &mapping_path)?);
    let webhook_path = root.join("webhook.json");
    write(&webhook_path, &oracle::webhook::expected()?)?;
    assert!(verify("webhooks", root)?);
    for wrong in [json!({}), json!({"component":"apiserver"})] {
        write(&webhook_path, &wrong)?;
        assert!(!verify("webhooks", root)?);
    }
    for raw in [b"{\"a\":1,\"a\":2}".as_slice(), b"{\"a\":NaN}"] {
        std::fs::write(&webhook_path, raw)?;
        assert!(!verify("webhooks", root)?);
    }
    assert!(!verify("unknown", root)?);
    Ok(())
}
