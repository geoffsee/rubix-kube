# Offline air-gap delivery reference

Zero Egress is a qualification requirement, not an established deployment result.
Verified offline bundle staging and Docker image import are implemented slices;
egress-denied retained-node workloads remain unqualified. Read the
[upstream inventory](../architecture/upstream-inputs.md),
[asset implementation](../../crates/rubix-assets/README.md) and
[management boundary](../../crates/rubixctl/README.md).

## Sixteen declared archive cells

The release matrix declares four architectures × two libcs × online/offline.
It is not evidence that sixteen production archives are already published.
Canonical release filenames use
`rubix-kube-VERSION-linux-ARCH[-musl][-offline].tar.gz`:

| Cell | Architecture | Libc | Delivery |
| --- | --- | --- | --- |
| Cell 01 | amd64 | glibc | online |
| Cell 02 | amd64 | glibc | offline |
| Cell 03 | amd64 | musl | online |
| Cell 04 | amd64 | musl | offline |
| Cell 05 | arm64 | glibc | online |
| Cell 06 | arm64 | glibc | offline |
| Cell 07 | arm64 | musl | online |
| Cell 08 | arm64 | musl | offline |
| Cell 09 | arm (ARMv7 hard-float) | glibc | online |
| Cell 10 | arm (ARMv7 hard-float) | glibc | offline |
| Cell 11 | arm (ARMv7 hard-float) | musl | online |
| Cell 12 | arm (ARMv7 hard-float) | musl | offline |
| Cell 13 | riscv64 | glibc | online |
| Cell 14 | riscv64 | glibc | offline |
| Cell 15 | riscv64 | musl | online |
| Cell 16 | riscv64 | musl | offline |

The compatibility download selector currently names upstream artifacts
`kubesolo-VERSION-linux-ARCH[-musl][-offline].tar.gz` and defaults to the Portainer
release URL. That is not a promise of Rust candidate publication. Select the
reviewed distribution version and actual digest-backed manifest explicitly;
do not substitute upstream Kubernetes v1.35.7 as the release version.

## Complete payload and integrity

Preserve kube-apiserver, kube-controller-manager, kubelet, kube-proxy, Kine,
managed containerd, supported OCI runtime/shims and CNI programs as well as images.
The release manifest has seven OCI indexes: the node (`image-rubix-kube`) plus
six dependency images. The legacy supervised bundle inventory uses
`image-kubesolo` for the Docker node payload; do not rename inventory IDs casually.
Dependency identities are `docker.io/coredns/coredns`, `docker.io/portainer/pause`,
`docker.io/rancher/local-path-provisioner`, `docker.io/library/busybox`,
`docker.io/portainer/agent` and `docker.io/portainer/d2k`. Use the pinned inventory
versions and hashes rather than resolving mutable `latest`/`lts` tags yourself.
Portainer is unavailable on riscv64; D2K is disabled on ARMv7 and riscv64. Offline
inventories require every supported image, while online optional pulls remain
separate. Copy only a complete prepared candidate, not a native binary relabeled
for another architecture or libc.

A tar listing containing `bundle.manifest` does not verify anything. Obtain the
expected digest through an approved independent source; a checksum shipped beside
an untrusted archive supplies consistency, not publisher authentication. The
installer audits a private archive copy, hashes declared staged files, checks
size/mode/inventory and rejects missing, extra, linked or traversal entries before
publication. Preserve the manifest and original digest receipts for review.

## Host staging

```sh
rubixctl install --run-mode service --offline-install /media/rubix-kube-0.1.0-linux-amd64-offline.tar.gz --path /srv/rubix-staging
```

This example assumes an actual approved matching candidate, not that release 0.1.0
exists. The host must match its Linux architecture and libc. The command stages
verified declared files under `--path`; it does not register a service or import
images into containerd's `k8s.io` namespace. Manual runtime materialization,
configuration, import and retained-node startup require their own verified setup
and live receipts. Listing containerd images alone does not establish working
pods, DNS, storage, no registry fallback or egress-denied node readiness.

## Docker Engine import

```sh
rubixctl install --run-mode container --name airgap-cluster --version 0.1.0 --offline-install /media/rubix-kube-0.1.0-linux-amd64-offline.tar.gz --container-ports 8080:80
```

Omit `--image`: that override takes precedence and retains registry-pull behavior.
The bundle must contain hash-listed `asset-inventory.json`, the complete supervised
offline inventory and its declared gzip `image-kubesolo` payload. Decoder checks
config platform and ordered layer DiffIDs on the same privately staged bytes sent
to Docker. Exactly one archive repo tag is allowed; import must confirm the
expected tag before creating owned resources. Creation uses `--pull=never`, with
no pull fallback after import. This loads the node image into Docker, not addon
images into the node's containerd. Synthetic Engine/image tests do not qualify
live Docker or air-gap workloads.

Docker load/pull commands have ten-minute deadlines; other commands have one-minute
deadlines plus a one-second child cleanup budget. Child termination does not prove
an already accepted remote Engine mutation was cancelled. Inspect partial effects
and retained state after failure.

Private registry mirrors are external runtime configuration and can require network
access. Do not overwrite generated managed containerd configuration or assume a
single arbitrary `hosts.toml` is consumed. Use the selected runtime's pinned
configuration and trust policy, then separately verify egress-denied behavior.
