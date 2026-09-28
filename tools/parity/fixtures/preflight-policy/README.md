# Actual Go preflight policy oracle

The pinned Go file is executed unchanged; source SHA256 is
`dd8d85a0593792886b627518b770645e13a9184e5f43a2dd55c071394973cb3e`.
The archive is KubeSolo `2ef1c4787989f11f868f81bb84ae2afd4a49a81d`, checked against
SHA256 `9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec`.
An additive same-package Go test calls actual hostname/controller/filesystem/port
checks; no baseline source function is replaced or generated from Rust code.
A minimal extraction go.mod selects the baseline's exact zerolog, colorable, isatty
and x/sys versions using the archive's unchanged go.sum. The pinned Go1.26.5 Docker
image is the same fixture toolchain used by the preceding platform oracle; it is
not the separately selected production API-generation Go toolchain.

Forty-four observations are captured twice and compared to hand-reviewed
`expected.tsv`. The Rust consumer reconstructs inputs independently. Coverage
includes root UID, raw hostname bounds, Docker landmarks, xt_comment loaded/match/
file/glob, Alpine tools, v1/v2 controllers and preparation-required Alpine service
state, ten actual port-bind scenarios and RunSuite fail-fast order. The latter uses
two explicitly synthetic check closures; it makes no claim of running real failing
services. All other policy calls are unchanged actual reference functions.

Runs use a private container network and owned tmpfs roots. Filesystem cases enter
separate chroot children with only SYS_CHROOT retained; no host mounts, package
installation, modprobe, mount, service calls, runtime control or service files are
involved. Calls use installPrereqs=false. Port tests open/close only this container's
2379/6443/10443/6060 sockets. Root check is actual UID0 in the container; the real/
effective UID divergence remains a source-derived Rust table, not a runtime setuid
qualification. Hostname helper inputs are synthetic; the host hostname is unchanged.

The outer driver has finite build/run deadlines and byte caps, two repeat inventories,
and guarded cleanup/receipt publication even on Docker errors. Each child additionally
has a five-second deadline. Source/harness/helper/output hashes, image identity and
compiled binary hash are retained. The verifier checks exact inventories and real
completion records. Tests mutate expectations, raw claims, source hashes, receipts,
parser edge cases and cleanup failure. No Rust behavior is used as the Go oracle.

```sh
python3 tools/parity/fixtures/preflight-policy/capture.py --output /tmp/new-preflight
python3 -O tools/parity/fixtures/preflight-policy/verify.py
python3 -m unittest discover -s tools/parity/fixtures/preflight-policy -p 'test_*.py'
```

Capture creates a new directory and never rewrites accepted expectations. Publishing
new evidence/provenance is an explicit review step. These files qualify bounded
policy observations, not privileged preparation, installer completion, external CRI,
nftables-only operation or a working node. See `crates/rubix-platform/PREFLIGHT.md`.
