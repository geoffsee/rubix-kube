use rubixctl::{Shell, generate_completion};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn powershell_completes_commands_and_shell_arguments() {
    let executable = std::env::var_os("RUBIX_REVIEW_PWSH").unwrap_or_else(|| "pwsh".into());
    let mut script = Vec::new();
    generate_completion(Shell::PowerShell, &mut script).unwrap();
    let binary_dir = std::path::Path::new(env!("CARGO_BIN_EXE_rubixctl"))
        .parent()
        .unwrap();
    let search_path = std::env::join_paths(std::iter::once(binary_dir.to_owned()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .unwrap();
    for (input, expected) in [
        ("rubixctl co", vec!["completion", "config"]),
        ("rubixctl completion", vec!["completion"]),
        (
            "rubixctl completion ",
            vec!["bash", "fish", "powershell", "zsh"],
        ),
        ("rubixctl completion p", vec!["powershell"]),
        ("rubixctl completion zs", vec!["zsh"]),
    ] {
        let mut child = match Command::new(&executable)
            .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "-"])
            .env("PATH", &search_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("PowerShell is unavailable; native completion smoke test skipped");
                return;
            },
            Err(error) => panic!("cannot run PowerShell: {error}"),
        };
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(&script).unwrap();
        writeln!(stdin).unwrap();
        writeln!(
            stdin,
            "[System.Management.Automation.CommandCompletion]::CompleteInput('{input}', {}, $null).CompletionMatches | ForEach-Object {{ $_.CompletionText }}",
            input.len(),
        )
        .unwrap();
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let mut matches = stdout.lines().collect::<Vec<_>>();
        matches.sort_unstable();
        assert_eq!(matches, expected, "completion for {input:?}");
    }
}
