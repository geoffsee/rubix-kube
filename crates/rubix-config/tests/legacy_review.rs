use rubix_config::legacy::{extract_service_flags, flags_to_config, rewrite_service_content};

#[test]
fn extraction_preserves_separate_and_explicit_empty_values() {
    let flags = extract_service_flags(
        "--node-ip 10.0.0.1 --mtu 1400 --path '/var/lib/custom path' --portainer-edge-key= --debug",
    );
    assert_eq!(flags.flags["node-ip"], "10.0.0.1");
    assert_eq!(flags.flags["mtu"], "1400");
    assert_eq!(flags.flags["path"], "/var/lib/custom path");
    assert_eq!(flags.flags["portainer-edge-key"], "");
    assert_eq!(flags.flags["debug"], "true");
    assert!(flags.missing_values.is_empty());
    assert!(flags_to_config(&extract_service_flags("--node-ip --debug"), None).is_err());
}

#[test]
fn rewrite_targets_arguments_and_removes_complete_quoted_tokens() {
    let layouts = [
        "# kubesolo documentation\nDescription=kubesolo\nExecStart=/usr/local/bin/kubesolo '--debug' '--node-ip=10.0.0.1'\n",
        "name=\"kubesolo\"\ncommand=\"/usr/local/bin/kubesolo\"\ncommand_args=\"--debug --node-ip=10.0.0.1\"\n",
        "DAEMON=\"/usr/local/bin/kubesolo\"\nDAEMON_ARGS=\"--debug --node-ip 10.0.0.1\"\n",
        "start-stop-daemon --start --quiet --pidfile /var/run/kubesolo.pid \\\n --exec /usr/local/bin/kubesolo -- \\\n --debug --node-ip=10.0.0.1\n",
        "exec /usr/local/bin/kubesolo '--debug' --node-ip=10.0.0.1\n",
    ];
    for original in layouts {
        let rewritten = rewrite_service_content(original, "/etc/kubesolo/config.yaml");
        assert_eq!(rewritten.matches("--config=").count(), 1);
        assert!(!rewritten.contains("--debug"));
        assert!(!rewritten.contains("--node-ip"));
        assert!(!rewritten.contains("''"));
        if original.contains("command_args=") {
            assert!(rewritten.contains(
                "name=\"kubesolo\"\ncommand=\"/usr/local/bin/kubesolo\"\ncommand_args=\"--config="
            ));
        }
        if original.starts_with("start-stop-daemon") {
            assert!(rewritten.find("-- ").unwrap() < rewritten.find("--config=").unwrap());
        }
        if original.starts_with('#') {
            assert!(
                rewritten.starts_with("# kubesolo documentation\nDescription=kubesolo\nExecStart=")
            );
        }
    }
}
