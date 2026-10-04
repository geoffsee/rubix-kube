# External container runtime reference

This describes configuration and adapter boundaries; it does not qualify a live
external-runtime deployment. Read the [compatibility contract](../architecture/compatibility-contract.md),
[acceptance matrix](../architecture/acceptance-matrix.md) and
[managed runtime implementation](../../crates/rubix-containerd/README.md).

## Managed and external ownership

Managed mode selects bundled containerd and explicit runtime/shim/CNI assets.
External mode connects to an already configured local CRI runtime, such as host
containerd or CRI-O. Host daemons, sockets, images, registry trust/configuration and
unrelated containers remain host-owned. Rubix cleanup is scoped to owned cluster
state; it must not stop or reset the host runtime. Ordinary reset retains PKI and
PV data. Uninstall's configuration/purge policies are documented in [the index](README.md).
Do not delete `/etc/containerd`, `/etc/crio`, host images or unrelated CNI files.

The catalog includes observations/fixtures for historical runtime versions; that
is not blanket qualification of every newer containerd/CRI-O release. Use the
exact runtime and upstream inputs verified for the target, with independent CRI,
pod, storage and restart receipts. Docker Engine by itself is not a CRI endpoint;
an independently configured CRI adapter would be required.

## Endpoint configuration

```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
runtime:
  endpoint: "unix:///run/containerd/containerd.sock"
```

The legacy node flag is `--container-runtime-endpoint` and its environment input
is `KUBESOLO_CONTAINER_RUNTIME_ENDPOINT`. A local absolute path is normalized to
`unix://`; relative paths and unsupported network schemes are rejected during
config validation. Root-only paths pass config validation and runtime conversion
stores `/` as the socket path. The separate CRI endpoint parser rejects `/`, but
config conversion does not invoke it. Choose an actual socket path rather than
treating config validation as proof of a usable endpoint.
Empty endpoint selects managed mode. A CRI-O example is
`unix:///run/crio/crio.sock`; verify the actual installed socket and permissions
rather than assuming either path exists.

Precedence remains defaults < config file < environment < explicit flags.
Successful decoding does not establish CRI socket connectivity. `rubixctl check`
uses its managed baseline context and is not a live external-CRI verifier.

## Cgroup-driver resolution

Adapter resolution consults CRI `RuntimeConfig` when available. A reported systemd
or cgroupfs driver wins; unsupported/empty runtime configuration follows the
container-mode, explicit setting and host-capability fallback policy described
in the compatibility contract. Container-mode fallback is cgroupfs; ordinary
host fallback selects systemd only with cgroup v2 and active systemd, otherwise
cgroupfs. Network/transport failures must remain errors rather than fabricated
successful RuntimeConfig observations.

Kubelet and the selected CRI runtime must agree. Do not change a shared host
runtime's driver or restart it merely to satisfy a generic example: that can
interrupt unrelated workloads. Inspect the runtime's actual pinned configuration,
its reported driver, rendered Kubelet configuration and startup diagnostics, then
rehearse any host change in a disposable environment. There is no generic
`--cgroup-driver` node flag or YAML `kubernetes.kubelet.cgroupDriver` setting in
the distribution schema. Rendered upstream Kubelet configuration is a separate
artifact, not a new arbitrary user-config field.

## Verification boundary

An effective `rubix-kube --print-config` is distribution input resolution, not the
negotiated runtime result or live pod test. Protect its potentially secret output.
Before using external mode, collect actual CRI info, compatible driver settings,
node readiness and workload creation/teardown evidence at exact versions. Preserve
foreign runtime resources during all rollback/cleanup tests. No receipt in this
runbook establishes current-source external containerd or CRI-O qualification.
