# CI and repository rules

The default branch accepts squash or rebase merges with passing `Format`, `Clippy`,
`Tests (debug)`, `Dependencies` and `Security` checks from GitHub Actions.
The branch must be current with its base. Linear history, resolved review conversations,
and protection against force-pushes/deletion apply without bypass actors. Human approvals
are optional to support solo development; automated checks remain mandatory.

## Fast feedback

CI runs independent jobs in parallel and cancels superseded runs. Pull requests run on
every change, including workflow-only changes; branch pushes run only on `main` to avoid
duplicate PR runs. PR base changes also run validation, including PRs targeting another stack
branch. All PR edits, including title/body edits, run validation with the same required check
names. During rollout, metadata-only skipped suites left native stack merges reporting missing
checks despite earlier successful runs. Full validation avoids that ambiguity; caches and
cancellation limit repeated work. Merge-group events are supported if a merge queue is introduced.
Pull requests run debug tests for all targets and doctests. Commands use the committed lockfile
and toolchain rather than a moving Rust channel.

Release tests, doctests, the `rubix-kube` and `rubixctl` `--profile dist` build, and upstream
input verification run from [landing.yml](workflows/landing.yml) on default-branch pushes and
merge groups. They are not pull-request checks. Qodana scans run from
[code_quality.yml](workflows/code_quality.yml) on pushes to `main` only. GitHub applies one required-check list to a
pull request and to its merge group, so requiring `Tests (release)` would put that job back
on the pull-request critical path. A merge queue therefore does not wait for the landing
workflow. The landing run still fails when those jobs fail.

Rust caches contain downloaded dependencies and reusable dependency compilation, separated
by job/profile. Only default-branch pushes save caches; PRs restore them. Cold caches remain
valid builds. The first successful default-branch run warms the caches. Advisory data is
refreshed by cargo-deny rather than treated as a permanent cached result.

External actions are pinned to commit SHAs, credentials are not persisted by checkout, and
job permissions are minimal. Dependabot proposes weekly action-pin, Cargo and security-tool updates.

## Stacked development

Use [gh-stack](https://github.com/github/gh-stack) for short chains of dependent PRs rooted on
`main`. Start with two to four layers, each buildable and tested. Keep independent work in separate
stacks/worktrees; issue order alone does not establish a dependency. For example, startup,
shutdown, and diagnostics form the E04 chain, while E03's precedence and persistence children
both depend on decoding and can proceed independently.

Give each stack one owner for branch operations. The coordinator owns issue transitions and final
merges, and coordinates shared manifests, lockfiles, and generated inputs. Plan layer ownership
before edits. Fix a lower-layer concern on that branch and rebase its descendants.

```sh
gh stack init --base main supervision/startup
# Implement, test, stage the relevant files, and commit.
gh stack add supervision/shutdown
# Implement, test, stage the relevant files, and commit.
gh stack submit --auto --remote origin
gh stack view --json
# After committing a change on its owning lower branch:
gh stack rebase --upstack --remote origin
gh stack push --remote origin
# After landing a ready prefix:
gh stack sync --remote origin
```

Use explicit branch names, `view --json`, and `submit --auto` for noninteractive operation.
New PRs are drafts; update their generated descriptions with issue links, behavior, and evidence,
then mark only ready layers for review. Consult the installed gh-stack skill and command help for
merge scope and recovery. Verify stack state after synchronization; an aborted sync can exit zero.

Before landing a prefix, verify its exact stack/PR membership and all five required check results for each
layer's current head and base. Native GitHub stacks enforce the trunk's protections on every layer,
as described in the [stack rules](https://docs.github.com/en/pull-requests/reference/stacked-pull-requests).
Retargeting and rebasing require fresh validation; tests on an earlier base are insufficient.
Manage native PR bases through stack operations; GitHub rejects manual `gh pr edit --base` changes.
Use `gh stack merge <verified-target-number> --yes --squash` for agent-managed stacks. Explicit
squash is the default convention; repository policy also permits rebase merging. Never bypass rules.
Reconcile the remaining stack after landing and close issues only when their complete acceptance
criteria and dependencies have reached `main` with evidence.

Keep dependency caches restricted to default-branch writes. Batch coherent edits before pushing
to limit repeated validation across stack descendants. A cache hit does not replace current checks.

## Local security tools

`Security` runs the Rust `rubix-security` maintenance binary with Semgrep Community
Edition and zizmor. Semgrep 1.178.0 uses its pinned multi-platform OCI image digest;
the container scans a read-only source mount with networking disabled. Zizmor 1.30.1
uses a checksum-pinned release executable and runs explicitly offline. The runner
disables Semgrep metrics/version checks, uses repository-owned rules and checks
that every selected Rust source was scanned. No hosted scanner API is required.

```sh
cargo run --locked -p rubix-dev --bin rubix-security
```

The repository-owned Semgrep rules initially detect TLS verification bypasses, common MD5/SHA-1
calls, and directly formatted shell commands. Positive/negative fixtures run before each scan.
All findings, scanner errors and unscanned Rust inputs fail the check; inline `nosemgrep`
suppression is disabled. Refine rules and their fixtures in a reviewed PR when an intentional
use needs different treatment. The workflow writes tool caches only on default-branch pushes.

Install the selected zizmor release on `PATH` and provide Docker for the local command.
The workflow installs the pinned Linux executable after verifying its archive SHA-256.
Scanner implementations remain external dependencies; repository-owned orchestration is Rust.

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
settings consistent with the allowed squash/rebase methods and disable merge commits.
`Security` is a normal required status check and needs no Code Security license.

If the default branch is renamed, update the workflow `push.branches` filters; the rulesets
and cache-write policy follow the repository's default branch automatically.

## Upstream and fixture gates

The landing workflow prepares verified upstream inputs, builds the maintenance
generator, compares committed CRI/containerd clients, checks published Kubernetes binding
provenance, and compares independent official schema/protocol inventories. These operations
run on default-branch pushes and merge groups; ordinary Rust builds still consume committed
clients. Rust tooling regressions run with workspace tests on pull requests. The prepared-parser
integration test runs explicitly after input preparation against the verified compiler. Clippy CI
also enforces the repository policy rejecting Python sources, packaging and interpreter invocations.

Prepared inputs have a manifest/OS/architecture cache key. Every restored byte is reverified;
only successful default-branch push jobs save caches. Cold caches download the same pinned
inputs. Cargo dependencies are fetched explicitly before the offline metadata provenance
check. No cached test result replaces validation of the current revision.

Real privileged VM and baseline capture commands remain explicit developer integrations.
Their frozen observations and deliberate mutation regressions run in CI; those checks do not
claim fresh node parity. Official executable defaults, feature gates and full lifecycle
qualification retain their separate acceptance requirements.
