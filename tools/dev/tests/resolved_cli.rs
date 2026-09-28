use rubix_dev::{Result, json::parse, sha256};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

fn write(directory: &Path, record: &Value, receipt: &Value) -> Result<()> {
    std::fs::create_dir_all(directory)?;
    let raw = serde_json::to_vec(record)?;
    let mut receipt = receipt.clone();
    receipt["output_sha256"] = json!({"run0.json":sha256(&raw),"run1.json":sha256(&raw)});
    std::fs::write(directory.join("resolved.json"), raw)?;
    std::fs::write(
        directory.join("receipt.json"),
        serde_json::to_vec(&receipt)?,
    )?;
    Ok(())
}

#[test]
fn report_exit_codes_markdown_and_read_only_behavior() -> Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../resolved-defaults");
    let record = parse(&std::fs::read(source.join("expected.json"))?)?;
    let receipt = parse(&std::fs::read(source.join("evidence/receipt.json"))?)?;
    let temporary = tempfile::tempdir()?;
    let before = temporary.path().join("before");
    let after = temporary.path().join("after");
    write(&before, &record, &receipt)?;
    let run = |after: &Path, format: &str| -> Result<std::process::Output> {
        Ok(Command::new(env!("CARGO_BIN_EXE_rubix-resolved"))
            .arg("report")
            .arg("--before")
            .arg(&before)
            .arg("--after")
            .arg(after)
            .args(["--format", format])
            .output()?)
    };
    assert_eq!(run(&before, "json")?.status.code(), Some(0));
    let mut changed = record;
    changed["cases"]["default"]["service_ip"] = json!("10.0.0.2");
    write(&after, &changed, &receipt)?;
    let raw = std::fs::read(after.join("resolved.json"))?;
    let first = run(&after, "markdown")?;
    assert_eq!(first.status.code(), Some(1));
    assert_eq!(first.stdout, run(&after, "markdown")?.stdout);
    assert!(String::from_utf8(first.stdout)?.contains("Resolved apiserver options"));
    assert_eq!(std::fs::read(after.join("resolved.json"))?, raw);
    std::fs::write(after.join("resolved.json"), b"{}")?;
    assert_eq!(run(&after, "json")?.status.code(), Some(2));
    Ok(())
}
