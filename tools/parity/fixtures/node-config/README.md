# Distribution node configuration fixtures

This E02.02 fixture invokes actual pinned KubeSolo kubelet/containerd constructors
and writers in additive package tests within a copied source tree. It starts no
runtime, kubelet, daemon or cluster and performs no mounts or cgroup initialization.
Official Kubernetes generated defaults remain a separate oracle; this fixture owns
the distribution's explicit overrides and rendering.

Five kubelet cases capture host/default, container/default, static CPU policy with
options/reservations, and reported systemd/cgroupfs drivers. Full source-reviewed
configuration maps are independently specified in verify.py. The actual writer's
YAML, parsed written document and repeated byte identity are retained. Four argument
cases use the actual configureKubeletArgs with an inert Cobra command to capture
IPv4/IPv6 overrides and omission for loopback/invalid IP. Six private-file restart
cases compare exact synthetic checkpoint bytes for unchanged/unrelated settings and absence
when policy, options, reserved CPUs or a malformed previous configuration differ.
The observer distinguishes absent, unchanged and changed states and checks a
corrupt-but-present negative control. Writer failures use an output directory and
a parent that is a regular file.

Containerd captures the actual complete constructor map and written TOML parsed
back into a tree. Independent expectations include runtime_type io.containerd.runc.v2,
absence of runtime_path, crun binary option, sandbox image, CNI and registry paths,
root/state/socket, import directory, supported task platforms and cgroup choice.
Snapshotter selection inspects only private tmpfs plus a missing-parent error path:
overlayfs here is a configuration selection, not a successful overlay mount.
Overlay-on-overlay/fuse/native variants are not qualified. Render bytes additionally
compare to a reviewed, hash-bound frozen fixture; the independent complete semantic
maps ensure generated output is not its sole oracle.

Runtime receives explicit synthetic DNS 192.0.2.53 and search root through Docker,
with network disabled. Before generating host-mode configuration the harness checks
that this is the only nameserver; it never exports resolver file content. A missing
/run/systemd/private is an enforced fixture condition. Thus host-default here means
the controlled container filesystem, not the developer's host or every init system.
Container mode independently resolves to /dev/null. All writable paths are fixed
inside the container's private 64 MiB tmpfs, so capture fields require no path stripping.
No host files, credentials or private keys are mounted or exported.

Run explicitly:

```sh
python3 tools/parity/fixtures/node-config/capture.py --output /tmp/unique-node-config
python3 -O tools/parity/fixtures/node-config/verify.py /tmp/unique-node-config
python3 -m unittest discover -s tools/parity/fixtures/node-config -p 'test_*.py'
```

The immutable source archive, digest-pinned Go 1.26.5 builder, unchanged module pins,
external_deps build, and exact harness/helper/output hashes are recorded. The shared
defaults lifecycle helper is imported read-only. Builds have a 30-minute/8 MiB limit;
each test a 90-second Go deadline plus 110-second/1 MiB client bounds. Runtime is UID65532,
read-only, network-none, no capabilities or privilege escalation, 512 MiB RAM, 2 CPUs,
128 PIDs and no host mounts. Each component runs in two independent containers;
strict tests require exactly one record in each retained log, match both to the
fixture, enforce complete provenance inventories, and mutate semantics/rendering,
record counts and identity. Cleanup independently removes/inventories only owned
containers/images and retains failure receipts; ordinary Docker build cache remains.

Baseline writers are not atomic, and checkpoint comparison ignores the previous YAML parser error, comparing zero or
partially decoded fields instead; the malformed-to-static fixture removes the
checkpoint. Invalidation happens before writing replacement config. These observations do
not establish safe interrupted-write recovery or live workload CPU reassignment.
Fixture ownership is #36; later consumers are #69/#70/#71 (kubelet configuration,
container/cgroup policy and checkpoint transitions) and #57 (managed runtime config).
Runtime startup/image handling #58, cleanup #59, DNS integration, alternate hosts,
actual CRI discovery, node registration, live pod behavior and Rust consumption remain
separate. No full-node launch, conformance or complete #36 qualification is claimed.
