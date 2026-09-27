# CI and repository rules

The default branch accepts squash-merged pull requests with passing `Format`, `Clippy`,
`Tests (debug)`, `Tests (release)` and `Dependencies` checks from GitHub Actions.
The branch must be current with its base. Linear history, resolved review conversations,
and protection against force-pushes/deletion apply without bypass actors. Human approvals
are optional to support solo development; automated checks remain mandatory.

## Fast feedback

CI runs independent jobs in parallel and cancels superseded runs. Pull requests run on
every change, including workflow-only changes; branch pushes run only on `main` to avoid
duplicate PR runs. Merge-group events are supported if a merge queue is introduced later.
Both test profiles include all targets and doctests. Commands use the committed lockfile
and toolchain rather than a moving Rust channel.

Rust caches contain downloaded dependencies and reusable dependency compilation, separated
by job/profile. Only default-branch pushes save caches; PRs restore them. Cold caches remain
valid builds. The first successful default-branch run warms the caches. Advisory data is
refreshed by cargo-deny rather than treated as a permanent cached result.

External actions are pinned to commit SHAs, credentials are not persisted by checkout, and
job permissions are minimal. Dependabot proposes weekly action-pin and Cargo updates.

## CodeQL activation

The workflow analyzes Rust and GitHub Actions using `security-and-quality` and build mode
`none`. It runs alongside CI without another Rust compilation, plus a weekly scan.

CodeQL is currently staged, not enforced: this private, personally owned repository is not
eligible for GitHub's documented private-repository Code Security offering. Do not interpret
skipped CodeQL jobs as successful scans. Visibility and ownership must not change implicitly.
See [GitHub's eligibility requirements](https://docs.github.com/en/code-security/reference/code-scanning/troubleshoot-analysis-errors/private-repository-enablement).

Once the repository is eligible and Code Security is enabled where required:

1. Use advanced setup with this workflow; avoid duplicate default-setup scans.
2. Set repository Actions variable `ENABLE_CODEQL=true`.
3. Run the workflow on the default branch and a pull request, verifying Rust and Actions results.
4. Change `rulesets/codeql.json` enforcement to `active` and apply that ruleset.

The CodeQL rule requires both analysis jobs and scan results. All security severities and
quality warnings/errors block merging; successful SARIF upload alone is not sufficient.

## Applying rules

The JSON files are reviewable API payloads, not automatically synchronized configuration.
After required check names have been verified on a PR, create the default-branch ruleset:

```sh
gh api --method POST repos/geoffsee/rubix-kube/rulesets \
  --input .github/rulesets/default-branch.json
```

For updates, list repository rulesets and use `PUT` on the existing ruleset ID instead of
creating duplicates. Inspect the active branch rules after applying. Keep repository merge
settings squash-only. Do not activate the CodeQL ruleset until scans are operational.

If the default branch is renamed, update the workflow `push.branches` filters; the rulesets
and cache-write policy follow the repository's default branch automatically.
