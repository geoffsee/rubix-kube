# Independent configuration and command fixtures

This is the node configuration/command slice of
[E02.02](https://github.com/geoffsee/rubix-kube/issues/36), against KubeSolo commit
`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`. It supplies 48 artifact-neutral cases in
[config-command.json](config-command.json). Use the same suite for the Go reference and Rust
candidate; expectations never branch on artifact kind.

```sh
python3 tools/parity/run.py --artifact /path/to/artifact.json \
  --suite tools/parity/fixtures/config-command.json --output /tmp/new-config-parity-run
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s tools/parity/fixtures -p 'test_*.py' -v
```

The runner must support `case.files` and complete `{fixture:config.yaml}` argument tokens.
It stages each input as a read-only, root-owned file in the isolated image, accessible to the
unprivileged artifact. There are no host mounts. `/dev/stdin` is deliberately not used: after
privilege dropping, the reference cannot reopen the root-owned input pipe as a configuration
file. Every successful node case uses `--print-config` or `--version`; invalid-command cases
terminate before bootstrap. No fixture starts a cluster, installs a service or changes a host.

## Oracle and provenance

[provenance.json](provenance.json) maps every case to pinned Go source files and source hashes,
records field coverage and capture commands, and identifies the source-built artifact. Expectations
were derived from the reference's configuration types/defaults, loader, registry, validator,
Kingpin flag declarations and node entry point, then exercised against the actual Linux binary.
The Rust implementation/generator does not produce these expected values.

[defaults.yaml](defaults.yaml) is the complete observed default document, checked field by field
against `Defaults()` and the derived config-socket rule. Exact-document assertions retain stable
YAML order/formatting; selected nondefault cases assert complete relevant sections. Raw stdout and
stderr are preserved in [the Go capture](evidence/go-reference.json), without normalization.
Timestamped log lines use literal diagnostic fragments in assertions, so timestamps are preserved
as evidence but do not become comparison requirements. Synthetic Edge credentials are deliberate
fixture values, not live credentials. `--print-config` includes those values; it is not a redacted
configuration-API read.

Coverage includes all 30 registered configuration fields through default/nondefault or rejection
cases, file/environment/flag precedence, empty/false/zero distinctions, config path selection,
container-mode tri-state, image normalization, SAN splitting, API socket derivation/limits,
CPU option/reservation validation, D2K validation, and warning-versus-error behavior for YAML
metadata, unknown/duplicate keys, malformed/type-invalid input and low MTU. The suite distinguishes
actual CLI syntax: Kingpin uses bare boolean flags and `--no-<flag>` negation; `--flag=false`
is rejected as an unexpected positional value at this baseline.

The D2K enabled and LoadBalancer requirement cases apply to Linux amd64/arm64, where the baseline
supports D2K. Other architectures need separate unsupported-target fixtures; this suite does not
claim that image support exists there. CPU success cases use policy `none`, avoiding host-dependent
CPU-count expectations. Static/container mode rejection and invalid policy/options are covered;
exclusive-CPU operation and host CPU-count boundary cases remain component qualification work.

## Evidence and negative controls

The frozen Go capture passes all 48 cases. The actual Rust placeholder fails comparison despite
exiting successfully for some commands. A separate real reference run with the storage default
intentionally changed in the expectation fails; the recorded negative result is not a parity pass.
[evidence/negative-controls.json](evidence/negative-controls.json) retains both outcomes, artifact
identities, runner hashes and cleanup results. The unit regressions independently alter captured
storage behavior, warning/error behavior and precedence and prove each change is rejected.

The capture commands record source-built `external_deps` Go provenance. This variant suffices for
configuration printing/validation, but does not include embedded runtime images or prove offline
cluster startup. Each runner capture verifies cleanup of its own container, image and evidence
volume. The frozen records are historical observations, not a substitute for running a new
candidate through the suite.

## Remaining E02.02 scope

This slice does not close E02.02 or parent E02. Generated Kubernetes resources/manifests, certificates
and trust persistence, atomic config writing/backup/permissions, config API editing/ETags, management
CLI lifecycle commands, derived runtime paths, and live restart/cleanup state still need independent
fixtures. Network/IP/hostname discovery and architecture-sensitive CPU/D2K rules also need explicit
host variants. These stay with E02 characterization and their E03–E26 domain owners; the VM adapter
makes disposable host scenarios possible but does not by itself supply those assertions.

The exact deliberately altered suite is retained as
[evidence/deliberate-mismatch-suite.json](evidence/deliberate-mismatch-suite.json); run
it with the reference artifact to reproduce the expected failure. Integrity regressions
check suite/capture hashes, raw output digests, the default document, negative results
and verified cleanup records. These checks protect stored evidence, not fresh parity.

The same 48 cases also ran in two fresh Debian arm64 VMs: Go passed all cases;
the real Rust placeholder failed all cases. [VM records](evidence/vm) retain source,
suite and artifact identities, compressed diagnostics and verified guest cleanup.
