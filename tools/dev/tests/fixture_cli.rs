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
fn resource_and_config_commands_reject_missing_records_and_files() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    for component in ["coredns", "localpath", "portainer", "d2k"] {
        write(
            &root.join(format!("{component}.json")),
            &oracle::resources::expected(component)?,
        )?;
    }
    assert!(verify("resources", root)?);
    write(&root.join("d2k.json"), &json!([]))?;
    assert!(!verify("resources", root)?);
    for component in ["config", "configapi"] {
        write(
            &root.join(format!("{component}.json")),
            &oracle::config_api::expected(component)?,
        )?;
    }
    assert!(verify("config-api", root)?);
    std::fs::remove_file(root.join("configapi.json"))?;
    assert!(!verify("config-api", root)?);
    let path = root.join("links.json");
    write(&path, &oracle::config_links::expected()?)?;
    assert!(verify("config-write-links", &path)?);
    write(&path, &json!([]))?;
    assert!(!verify("config-write-links", &path)?);
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
    for component in ["kubelet", "containerd"] {
        write(
            &root.join(format!("{component}.json")),
            &oracle::node_config::expected(component)?,
        )?;
    }
    assert!(verify("node-config", root)?);
    let path = root.join("containerd.json");
    let mut changed = oracle::node_config::expected("containerd")?;
    changed["toml"] = format!("{}\n# altered\n", changed["toml"].as_str().ok_or("toml")?).into();
    write(&path, &changed)?;
    assert!(!verify("node-config", root)?);
    write(&path, &oracle::node_config::expected("kubelet")?)?;
    assert!(!verify("node-config", root)?);
    let path = root.join("pki.json");
    write(&path, &oracle::pki::expected())?;
    assert!(verify("pki", &path)?);
    let mut changed = oracle::pki::expected();
    changed[0]["fresh"]["ca"]["key_mode"] = json!("0644");
    for value in [json!([]), changed] {
        write(&path, &value)?;
        assert!(!verify("pki", &path)?);
    }
    Ok(())
}
