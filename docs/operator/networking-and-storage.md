# Networking and local storage reference

Read the [compatibility contract](../architecture/compatibility-contract.md) and
[acceptance matrix](../architecture/acceptance-matrix.md). These implementation
identities and configuration examples do not qualify current-source pod networking,
service routing or PV access on a real node.

## Addresses and MTU

The baseline pod CIDR is `10.42.0.0/16`, service CIDR `10.43.0.0/16`, API service
address `10.43.0.1` and CoreDNS service address `10.43.0.10`. Preserve these
compatibility identities unless an explicitly supported configuration changes them.
`network.nodeIP` and `network.mtu` select or override observed host properties.
Zero MTU uses automatic resolution; positive MTU below 1280 is invalid when IPv6
is enabled. Do not assume a generic 9000 upper bound or that every interface/VPN
combination is qualified.

```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
network:
  mtu: 1420
```

## CNI ownership and generated configuration

Required programs include `bridge`, `host-local`, `portmap` and `loopback`; verify
actual executable assets for the selected architecture/libc. The owned filename
is `10-bridge.conflist`. The generator uses CNI version 1.0.0, name `kubesolo-net`,
bridge `cni0`, `isGateway: true`, `ipMasq: false`, host-local IPAM and portmap.
Do not replace these with `rubix-bridge`/`cbr0` identities from unrelated examples.
Use the actual generated document rather than hand-copying a partial conflist.

Managed configuration is staged under `<base>/containerd/cni/conf`, with an owned
standard-directory link; external mode writes the selected configuration directory.
Inspect existing configuration ordering and any collision with the reserved name
before activating Rubix. Preserve third-party CNI files and binaries. Earlier
lexicographic configurations can change which network a runtime actually uses;
a successfully written file is not proof of pod connectivity.

## Pod egress coexistence

The network adapter creates the owned nftables table `kubesolo-masq` or iptables
rule comment `kubesolo: pod masquerade`. Pod-to-external traffic is masqueraded;
pod-to-pod and unrelated host traffic remain outside that rule. Cleanup must
select those exact identities, never blanket `iptables -F` or `nft flush ruleset`.
Inspect actual backend/host capability observations rather than assuming one tool
or kernel feature is installed. Startup dependency plans order host networking
before retained Kubelet startup; plans/tests are not a guarantee of zero packet
loss or a currently qualified restart on every host.

```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
network:
  disableIPv6: true
```

IPv6/sysctl/resolver operations require their documented host ownership and
capability checks. An input setting alone does not prove dual-stack workloads,
DNS, firewall coexistence or successful sysctl mutation.

## LocalPath storage

The selected addon is Rancher LocalPath (`rancher.io/local-path`), with default
StorageClass `local-path` and `WaitForFirstConsumer`. ReclaimPolicy Delete and
Retain are distinct: inspect the actual PV/StorageClass before deleting a PVC.
The default data root is `/var/lib/kubesolo/local-path-storage`; sharedPath is
explicit configuration and can point outside the normal base directory.

```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
storage:
  localPath:
    enabled: true
    sharedPath: "/mnt/fast-nvme/k8s-volumes"
```

Provisioner helpers/path checks are implemented slices; neither passing fixture
checks nor a stored Pod spec proves a real helper executed or a volume was mounted.
Verify actual PVC binding, helper completion, pod reads/writes, retention and
teardown in a disposable node. Quiesce PV writers before file comparisons; preserve
ownership, full modes and symlink targets without following them. Ordinary reset
and uninstall retain PV data. Explicit purge is destructive and must be reviewed
against the selected owned root and mount boundaries.
