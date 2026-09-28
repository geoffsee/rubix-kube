//! Review resolved API-server option snapshots without refreshing their evidence.
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use rubix_dev::{Result, resolved_report};
use serde_json::Value;

fn markdown(report: &Value) -> Result<String> {
    let mut output = format!(
        "# Resolved API-server option review\n\nStatus: **{}**; {} changes; {} removals.\n\n## Source pins\n\n```json\n{}\n```\n",
        report["status"].as_str().ok_or("report status")?,
        report["change_count"],
        report["removal_count"],
        serde_json::to_string_pretty(&report["source_pins"])?
    );
    for category in resolved_report::CATEGORIES {
        let title = category.replace('_', " ");
        let title = format!("{}{}", title[..1].to_uppercase(), &title[1..]);
        write!(output, "\n## {title}\n\n")?;
        let changes = report["changes"][category]
            .as_array()
            .ok_or("report changes")?;
        if changes.is_empty() {
            output.push_str("No changes.\n");
        }
        for change in changes {
            writeln!(output, "    {}\n", serde_json::to_string(change)?)?;
        }
    }
    Ok(format!("{}\n", output.trim_end()))
}

fn run() -> Result<ExitCode> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("report")) {
        return Err(
            "usage: rubix-resolved report --before DIR --after DIR [--format json|markdown]".into(),
        );
    }
    let mut before = None;
    let mut after = None;
    let mut format = String::from("json");
    while let Some(option) = arguments.next() {
        let value = arguments.next().ok_or("missing option value")?;
        match option.to_str() {
            Some("--before") if before.is_none() => before = Some(PathBuf::from(value)),
            Some("--after") if after.is_none() => after = Some(PathBuf::from(value)),
            Some("--format") => format = value.into_string().map_err(|_| "invalid format")?,
            _ => return Err("unknown or duplicate option".into()),
        }
    }
    if format != "json" && format != "markdown" {
        return Err("format must be json or markdown".into());
    }
    let before = resolved_report::load_snapshot(&before.ok_or("--before required")?)?;
    let after = resolved_report::load_snapshot(&after.ok_or("--after required")?)?;
    let report = resolved_report::build_report(&before, &after)?;
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", markdown(&report)?);
    }
    Ok(if report["status"] == "changed" {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("resolved-report: {error}");
            ExitCode::from(2)
        },
    }
}
