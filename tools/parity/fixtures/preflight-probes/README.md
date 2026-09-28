# Disposable supplemental probe qualification

This fixture runs actual Rust supplemental filesystem and wildcard socket probes
in fresh Linux/arm64 Docker containers. It is not a new Go oracle: the existing
`../preflight-policy` fixture independently captured the pinned Go disk/glob and
mandatory/pprof port behavior. The Rust production source and private-root test
adapter are separately identified by `source-hashes.json`.

The build pins the Rust image by digest, Cargo.lock and the repository toolchain.
Each of two fresh containers runs the pure tests plus two explicitly ignored live
cases, as UID 65532, without capabilities/network access/host mounts, on a read-only
root with a 16 MiB private tmpfs. The network namespace permits real IPv4 and IPv6
wildcard listeners without affecting host services. No sysctl, module load or
package/service command runs. Private module files exist only in an owned temporary
root used by the test's filesystem adapter. Public `collect_supplemental` is also
called read-only against the container namespace; host-specific output is not
normalized into a success oracle.

Socket records prove free ports, one-family-only conflicts, disabled pprof skipping,
closure/rebind and preservation of competing listeners. The explicitly labeled
`injected_ipv6_unsupported_real_ipv4` case injects an initial unsupported-family
result and then really binds/conflicts/closes IPv4. Detection on a kernel without
IPv6 remains unqualified; no host IPv6 setting is changed.

Run only the bounded capture driver for live qualification:

```
python3 tools/parity/fixtures/preflight-probes/capture.py --output /tmp/new-probe-evidence
python3 tools/parity/fixtures/preflight-probes/verify.py
python3 -m unittest discover -s tools/parity/fixtures/preflight-probes -p 'test_*.py'
```

A capture build is bounded to 900 seconds/8 MiB, each run to 60 seconds/1 MiB, and
metadata/cleanup commands to 30 seconds/64 KiB. Every cleanup inspection is attempted
independently and the receipt is published on failure. Only the exact owned image
and containers are removed. Docker build cache is daemon-owned. The verifier checks
raw assertion markers, identical binaries across repeats, source/manifest/lock
binding, current harness hashes, exact evidence inventory and clean teardown.
Logs retain test-runtime formatting; no claim of byte-identical test timings is made.
The tests qualify trusted repository code, not hostile artifact execution.
