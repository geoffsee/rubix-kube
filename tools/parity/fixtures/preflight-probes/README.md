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

The verifier checks the committed `evidence/` directory and `provenance.json`.
To refresh it, run the following from the repository root. The shell stops if
capture or installation fails; verification then checks the newly installed run.
Review the resulting evidence diff before committing it.

```sh
set -e
python3 tools/parity/fixtures/preflight-probes/capture.py --output /tmp/new-probe-evidence
python3 - <<'INSTALL'
import hashlib
import json
from pathlib import Path
import shutil

capture = Path('/tmp/new-probe-evidence')
fixture = Path('tools/parity/fixtures/preflight-probes')
names = {'build.log', 'run0.log', 'run1.log', 'source-hashes.json', 'receipt.json'}
if {path.name for path in capture.iterdir()} != names:
    raise SystemExit('unexpected capture file inventory')
receipt = json.loads((capture / 'receipt.json').read_text())
for key in ['errors', 'cleanup_errors', 'remaining_containers', 'remaining_images']:
    if receipt.get(key) != []:
        raise SystemExit('capture did not finish cleanly: ' + key)
(fixture / 'evidence').mkdir(exist_ok=True)
for name in names:
    shutil.copyfile(capture / name, fixture / 'evidence' / name)
hashes = {name: hashlib.sha256((fixture / 'evidence' / name).read_bytes()).hexdigest()
          for name in sorted(names)}
(fixture / 'provenance.json').write_text(json.dumps({'files': hashes}, indent=2) + '\n')
INSTALL
python3 tools/parity/fixtures/preflight-probes/verify.py
python3 -m unittest discover -s tools/parity/fixtures/preflight-probes -p 'test_*.py'
```

To verify existing committed evidence without capturing or replacing files, run
only the final verifier and test commands.

A capture build is bounded to 900 seconds/8 MiB, each run to 60 seconds/1 MiB, and
metadata/cleanup commands to 30 seconds/64 KiB. Every cleanup inspection is attempted
independently and the receipt is published on failure. Only the exact owned image
and containers are removed. Docker build cache is daemon-owned. The verifier checks
raw assertion markers, identical binaries across repeats, source/manifest/lock
binding, current harness hashes, exact evidence inventory and clean teardown.
Logs retain test-runtime formatting; no claim of byte-identical test timings is made.
The tests qualify trusted repository code, not hostile artifact execution.
