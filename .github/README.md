# CI and repository rules

The default branch accepts squash-merged pull requests with passing `Format`, `Clippy`,
`Tests (debug)`, `Tests (release)`, `Dependencies` and `Security` checks from GitHub Actions.
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
job permissions are minimal. Dependabot proposes weekly action-pin, Cargo and security-tool updates.

## Local security tools

`Security` runs Semgrep Community Edition and zizmor alongside the existing CI, without a
Rust build or hosted analysis service. Tool versions and transitive package hashes are pinned
in `security/pyproject.toml` and `security/uv.lock`. Installation downloads packages; analysis
uses only local files. Semgrep metrics/version checks are disabled, no registry rules are
fetched, and zizmor runs explicitly offline. No scanner API token or SARIF upload is required.

```sh
uv sync --project .github/security --locked --python 3.12
uv run --project .github/security --frozen --offline --no-sync python .github/security/check.py
```

The repository-owned Semgrep rules initially detect TLS verification bypasses, common MD5/SHA-1
calls, and directly formatted shell commands. Positive/negative fixtures run before each scan.
All findings, scanner errors and unscanned Rust inputs fail the check; inline `nosemgrep`
suppression is disabled. Refine rules and their fixtures in a reviewed PR when an intentional
use needs different treatment. The workflow writes tool caches only on default-branch pushes.

zizmor audits local workflow and composite-action definitions for permission, injection and
other workflow risks. Offline mode excludes checks requiring GitHub API history, such as
remote action provenance checks. Clippy remains the compiler-aware quality gate; cargo-deny
checks dependency advisories, sources, licenses and bans.

This is deliberately scoped coverage, not CodeQL-equivalent whole-program analysis. Semgrep CE
does not provide general cross-file dataflow analysis, and these five rules do not detect every
Rust vulnerability or every equivalent spelling. Expand the rules with tested cases as runtime,
PKI and network implementations arrive. See [Semgrep CE](https://github.com/semgrep/semgrep)
and [zizmor operating modes](https://docs.zizmor.sh/usage/#operating-modes).

## Applying rules

The JSON files are reviewable API payloads, not automatically synchronized configuration.
After required check names have been verified on a PR, create the default-branch ruleset:

```sh
gh api --method POST repos/geoffsee/rubix-kube/rulesets \
  --input .github/rulesets/default-branch.json
```

For updates, list repository rulesets and use `PUT` on the existing ruleset ID instead of
creating duplicates. Inspect the active branch rules after applying. Keep repository merge
settings squash-only. `Security` is a normal required status check and needs no Code Security license.

If the default branch is renamed, update the workflow `push.branches` filters; the rulesets
and cache-write policy follow the repository's default branch automatically.
