use rubix_dev::conformance::{
    CERTIFICATION_DISCLAIMER, CONFORMANCE_FOCUS_REGEX, CONFORMANCE_SKIP_REGEX,
    ConformanceInventory, ConformanceResult, ConformanceSummary, Kubeconfig,
};

fn executed_summary() -> ConformanceSummary {
    let results: Vec<_> = ConformanceInventory::selected_tests()
        .into_iter()
        .map(|t| ConformanceResult {
            test_id: t.id,
            name: t.name,
            passed: true,
            duration_ms: 1,
            error: None,
        })
        .collect();
    ConformanceSummary {
        total_selected: results.len(),
        passed: results.len(),
        failed: 0,
        excluded_count: ConformanceInventory::explicit_exclusions().len(),
        focus_filter: CONFORMANCE_FOCUS_REGEX.into(),
        skip_filter: CONFORMANCE_SKIP_REGEX.into(),
        certification_disclaimer: CERTIFICATION_DISCLAIMER.into(),
        exclusions: ConformanceInventory::explicit_exclusions(),
        results,
    }
}

#[test]
fn summary_requires_exact_unique_successful_per_case_results() {
    // Report-consistency fixture only, not execution evidence.
    let summary = executed_summary();
    summary.verify_qualification().unwrap();
    let mut missing = summary.clone();
    missing.results.clear();
    assert!(missing.verify_qualification().is_err());
    let mut duplicate = summary.clone();
    duplicate.results[1] = duplicate.results[0].clone();
    assert!(duplicate.verify_qualification().is_err());
    let mut failed = summary.clone();
    failed.results[0].passed = false;
    failed.results[0].error = Some("workload failed".into());
    assert!(failed.verify_qualification().is_err());
    let mut contradictory = summary.clone();
    contradictory.results[0].error = Some("not executed".into());
    assert!(contradictory.verify_qualification().is_err());
    let mut wrong = summary.clone();
    wrong.results[0].name = "unrelated test".into();
    assert!(wrong.verify_qualification().is_err());
    let mut counts = summary.clone();
    counts.passed -= 1;
    assert!(counts.verify_qualification().is_err());
    let mut exclusions = summary;
    exclusions.exclusions[0].rationale.clear();
    assert!(exclusions.verify_qualification().is_err());
}

#[test]
fn yaml_roundtrip_preserves_escaped_scalars_empty_users_and_preferences() {
    for context in [
        "admin: prod",
        "true",
        "# comment",
        "line\nbreak",
        "quote\"slash\\",
    ] {
        let input = serde_json::json!({ "apiVersion":"v1", "kind":"Config", "current-context":context, "preferences":{"colors":true,"extension":"quoted: value"}, "clusters":[{"name":"cluster", "cluster":{"server":"https://127.0.0.1:6443", "certificate-authority":"/a: b/#ca"}}], "contexts":[{"name":context,"context":{"cluster":"cluster","user":"user"}}], "users":[{"name":"user","user":{"token":"true # token\nnext"}},{"name":"empty","user":{}}] });
        let parsed = Kubeconfig::parse(&input.to_string()).unwrap();
        assert_eq!(Kubeconfig::parse(&parsed.to_yaml()).unwrap(), parsed);
    }
}

#[test]
fn cli_cannot_claim_node_qualification() {
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_rubix-conformance"))
        .arg("run")
        .output()
        .unwrap();
    assert!(!run.status.success());
    assert!(String::from_utf8_lossy(&run.stderr).contains("not implemented"));
}
