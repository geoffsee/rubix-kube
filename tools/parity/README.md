# Isolated distribution parity driver

This E02.01 foundation runs the same black-box expectations against checksum-pinned
Go and Rust Linux artifacts. Artifact identity changes; the suite and assertions do
not branch on implementation language. This is a startup-command driver, not a full
cluster harness. E02.02 owns authoritative golden fixtures, and later work supplies
privileged runtime/lifecycle adapters.

## Prepare the pinned reference

```sh
cargo run --locked -p rubix-dev --bin rubix-parity -- prepare-upstream --output /tmp/kubesolo-reference
```

The new output directory receives the Linux binary, artifact descriptor, Go build
metadata, build logs and preparation result. The builder verifies the immutable
KubeSolo source archive for commit `2ef1c4787989f11f868f81bb84ae2afd4a49a81d`, uses
Go 1.26.5 from a digest-pinned container image, retains `go.mod`/`go.sum` including
K3s replacements, and builds with `CGO_ENABLED=1`, `GOTOOLCHAIN=local`,
`GOFLAGS=-mod=readonly`, `-tags external_deps`, `-trimpath` and two compile workers.
It does not execute the built distribution. The `external_deps` build intentionally
omits embedded payloads and must not be presented as an offline/managed-runtime release.

The upstream source's `cmd/kubesolo/main.go` returns for `--version` before service
construction and for `--print-config` before bootstrap. Startup cases execute as an
unprivileged artifact user (UID 65532) with no capabilities, network, host mounts or exposed ports.
The trusted driver runs as container root with only SETUID, SETGID and KILL capabilities
so it can separate artifact identity, protect evidence, and clean descendant processes.
The artifact cannot write the root-owned evidence directory or alter driver reports.
The preparation build has a 30-minute limit. Each Docker metadata/cleanup operation
has a separate bounded timeout. Ordinary Docker build caches remain Docker-managed.

## Run a shared suite

```sh
cargo run --locked -p rubix-dev --bin rubix-parity -- run \
  --artifact /tmp/kubesolo-reference/artifact.json \
  --suite tools/parity/startup.json \
  --output /tmp/parity-reference
```

Every output directory must be new. Each stream is capped at 256 KiB with a kernel
file-size limit; reaching the cap fails the case. Suites allow at most 64 cases, limiting
artifact stdout/stderr storage to 32 MiB per run. Report import is capped at 64 MiB.
The container root filesystem is read-only; artifact scratch writes use a 64 MiB
`/tmp` tmpfs. Reports use a uniquely named Docker-managed volume with root-only
permissions, preserving bounded driver evidence after the container stops. No host
bind mount is exposed to the container. The runner verifies removal of that owned
volume after evidence export. Replace only `--artifact` to exercise the Rust
implementation. An incomplete Rust executable is expected to fail behavior assertions;
it is not treated as compatible simply because it exits zero.

The artifact descriptor is schema version 1:

```json
{
  "schema_version": 1,
  "kind": "rust",
  "binary": "rubix-kube",
  "sha256": "<64 lowercase hexadecimal characters>",
  "version": "<recorded artifact version>",
  "source": {"repository": "<repository URL>", "revision": "<exact 40-character Git commit>"},
  "build": {"variant": "<exact variant and relevant build metadata>"}
}
```

`binary` resolves relative to the descriptor. The runner validates its digest before
copying it into a uniquely named fixture image; the container independently validates
it before execution. A descriptor is a provenance claim, so retain build logs and
source records supporting its revision. Dirty-source builds must additionally record
source file hashes in `build`; a Git revision alone must not imply clean provenance.
The copied binary must support the Docker engine's Linux architecture and the
bookworm fixture's runtime libraries. Wrong architecture/library errors are failures.

Suites use `schema_version: 1`, an `id`, and a nonempty `cases` array. Each case has:

- unique `id`, argument array `argv`, optional string environment map `env` and `stdin`;
- `timeout_seconds` between 1 and 120 (default 30);
- `privilege: "none"` (default) or `"privileged"`;
- `expect.exit_code`, plus optional `stdout_contains`, `stderr_contains`,
  `combined_contains` string arrays or corresponding `_equals` strings.

Unknown assertions are rejected. Case commands use direct executable invocation,
never shell evaluation. Each case records raw stdout/stderr files and their hashes,
exit status, elapsed time, failures and owned-process-group cleanup. Expectations are
literal; broad version/config markers in `startup.json` are initial smoke coverage,
not the full configuration oracle required by E02.02.

Privileged cases are **not executed** and produce explicit `gap` records. This
adapter never accepts `--privileged` or mounts the Docker socket. Whole-cluster,
networking and lifecycle scenarios remain unsupported until an explicitly isolated
privileged adapter exists. This restriction does not remove their acceptance criteria.

Exit codes: `0` means all requested supported cases passed; `1` means a test,
preparation, diagnostic or teardown failure; `2` means requested cases include gaps.
The result always lists unsupported capabilities even for a passing startup-only run.

## Failure recovery and evidence

Use separate new output directories:

```sh
cargo run --locked -p rubix-dev --bin rubix-parity -- run --artifact /tmp/kubesolo-reference/artifact.json \
  --suite tools/parity/startup.json --output /tmp/parity-setup-failure --inject-failure setup
cargo run --locked -p rubix-dev --bin rubix-parity -- run --artifact /tmp/kubesolo-reference/artifact.json \
  --suite tools/parity/startup.json --output /tmp/parity-test-failure --inject-failure test
```

These are expected failures, not parity passes. Setup injection occurs inside the
created isolated container before case execution; test injection occurs after command
diagnostics have been collected. Evidence copying, container inspection, image
inspection and cleanup are independent attempts. Container evidence is exported into a
bounded quarantine archive, validated completely for allowed regular report filenames,
and published with exclusive no-follow writes. Symlinks, special entries, path traversal,
duplicates and oversized reports fail import. The trusted runner result uses exclusive
no-follow creation too. Driver reports are protected from the artifact UID. Processes
that create a new session are detected by the artifact UID inside this owned container,
killed and reported as failures before another case starts. The runner removes and verifies only
its UUID-owned container/image/evidence volume, including after ambiguous create failures, and leaves
all preexisting Docker resources untouched. A failed removal or unavailable Docker
daemon makes the result fail instead of claiming cleanup success.

`result.json` describes executed behavior; `runner-result.json` records source/suite
hashes, orchestration errors, owned names and final exit status. Preparation errors
still produce `runner-result.json` even when the container never runs. Docker logs,
metadata and raw command outputs remain in the output directory. Archive the entire
directory with the tested repository revision when attaching issue/PR evidence.

```sh
cargo test --locked -p rubix-dev --bin rubix-parity --test parity_process
```

The unit checks exercise assertion failures, language-independent expectations and
rejection of unknown assertions, moving source references and unbounded scenarios.
Real artifact execution and injected-failure runs are separate integration evidence.

### File inputs

A case may provide `files`, a map of canonical relative POSIX paths to UTF-8 contents.
Use a complete argv token such as `{fixture:config.yaml}` after `--config`; the adapter
resolves it to that case's private fixture directory. Inline or environment interpolation
is not performed. Inputs are copied into the disposable image, root-owned and read-only;
the container's root filesystem is read-only. No host path is mounted. Paths cannot escape
or overlap, and limits are 32 files/256 KiB per case and 1 MiB per suite. Contents and
filenames are covered by the recorded suite hash. Fixtures should contain synthetic data.
