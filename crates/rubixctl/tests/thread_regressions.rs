use rubixctl::{DownloadOptions, InstallOptions, UpgradeOptions};
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn url_and_proxy_credentials_are_redacted_in_all_command_debug_output() {
    let url = Some("https://user:url-secret@example.test/archive?token=query-secret".into());
    let proxy = Some("https://user:proxy-secret@example.test".into());
    let outputs = [
        format!(
            "{:?}",
            DownloadOptions {
                custom_url: url.clone(),
                proxy: proxy.clone(),
                ..Default::default()
            }
        ),
        format!(
            "{:?}",
            InstallOptions {
                custom_url: url.clone(),
                proxy: proxy.clone(),
                ..Default::default()
            }
        ),
        format!(
            "{:?}",
            UpgradeOptions {
                custom_url: url,
                proxy,
                ..Default::default()
            }
        ),
    ];
    for output in outputs {
        assert!(output.contains("<redacted>"));
        for secret in ["url-secret", "query-secret", "proxy-secret"] {
            assert!(!output.contains(secret));
        }
    }
}

#[test]
fn failed_download_preserves_bundle_and_cleans_owned_staging() {
    let root = tempfile::TempDir::new().unwrap();
    let staging = root.path().join("staging");
    std::fs::create_dir(&staging).unwrap();
    std::fs::write(staging.join("unrelated"), "retain").unwrap();
    let dest = root.path().join("bundle");
    std::fs::write(&dest, "old").unwrap();
    let error = rubixctl::download::stage_download(&dest, Some(&staging), |path| {
        assert!(path.starts_with(&staging));
        std::fs::write(path, "partial")?;
        Err(std::io::Error::other("transfer failed"))
    });
    assert!(error.is_err());
    assert_eq!(std::fs::read_to_string(&dest).unwrap(), "old");
    assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 1);
    rubixctl::download::stage_download(&dest, Some(&staging), |path| {
        std::fs::write(path, "complete")
    })
    .unwrap();
    assert_eq!(std::fs::read_to_string(&dest).unwrap(), "complete");
    assert_eq!(std::fs::read_dir(staging).unwrap().count(), 1);
}

#[test]
fn custom_url_cannot_inject_curl_options_or_change_protocol_policy() {
    let command =
        rubixctl::download::curl_download_command("-K/etc/private", Path::new("out"), None);
    let args: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_str().unwrap())
        .collect();
    assert_eq!(&args[args.len() - 2..], &["--", "-K/etc/private"]);
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--proto", "=https,http"])
    );
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--proto-redir", "=https,http"])
    );
}

#[test]
fn sudo_recovery_does_not_restore_privileged_execution_controls() {
    let mut env = BTreeMap::from([("SUDO_USER".into(), "alice".into())]);
    rubixctl::recover_sudo_env_from_bytes(&mut env, b"KUBESOLO_INSTALL_PREREQS=true\0KUBESOLO_CUSTOM_URL=file:///private\0KUBESOLO_PATH=/root\0KUBESOLO_PORTAINER_EDGE_KEY=key\0KUBESOLO_PORTAINER_EDGE_ID=id\0");
    assert_eq!(env.get("KUBESOLO_PORTAINER_EDGE_KEY").unwrap(), "key");
    assert_eq!(env.get("KUBESOLO_PORTAINER_EDGE_ID").unwrap(), "id");
    assert_eq!(env.len(), 3);
}

#[cfg(unix)]
#[test]
fn unrelated_non_utf8_environment_does_not_panic() {
    use std::os::unix::ffi::OsStringExt;
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rubixctl"))
        .arg("version")
        .env_clear()
        .env("UNRELATED", std::ffi::OsString::from_vec(vec![0xff]))
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rubixctl"))
        .arg("version")
        .env_clear()
        .env("KUBESOLO_PATH", std::ffi::OsString::from_vec(vec![0xff]))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be UTF-8"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
}
