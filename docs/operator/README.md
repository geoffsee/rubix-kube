# Rubix operator references and runbooks

These documents describe implemented management interfaces and required operator
checks. They do not establish a production deployment or migration qualification.
C13/C14 and the C16/C17 operator handoff gates remain pending current-source live
Linux installation, workload, recovery and handoff evidence. Passing documentation
or synthetic fixture tests is not that evidence.

## Runbook map

| Reference | Scope |
| --- | --- |
| [Fresh installation](fresh-installs.md) | Verified host bundle staging, manual host prerequisites, actual container install interface and client access. |
| [Air-gap delivery](air-gap-deployment.md) | Sixteen declared archive cells, complete asset inventories and separate host/Docker import boundaries. |
| [External runtimes](external-container-runtime.md) | Local CRI endpoints, cgroup-driver resolution and host ownership. |
| [Networking and storage](networking-and-storage.md) | Owned CNI/egress identities and LocalPath behavior. |
| [Addons and egress](addons-and-egress-runbook.md) | Offline addon image acquisition, denied egress isolation, LocalPath PVC lifecycle, and D2K client-certificate mTLS authentication. |
| [Metrics and CPU management](metrics-and-cpu-management.md) | Metrics listener scope, series and CPU manager configuration. |
| [Migration and recovery](migration-and-recovery.md) | Unqualified production cutover, actual receipt recovery, backup integrity and remaining evidence. |
| [Performance budget qualification](performance-budget-qualification-runbook.md) | Matched arm64/amd64 live performance budgets, 24-hour soak, 8 retained processes, and candidate receipt generation. |

Read the [compatibility contract](../architecture/compatibility-contract.md),
[acceptance matrix](../architecture/acceptance-matrix.md),
[component boundary ADR](../../experiments/component-boundary/ADR.md) and
[upstream inventory](../architecture/upstream-inputs.md) first. The selected
boundary retains kube-apiserver, kube-controller-manager, kubelet, kube-proxy,
Kine and managed containerd. Packaging also includes runtime shims, CNI programs
and required images; this is not a single self-contained Kubernetes binary.

## Ownership and retention

Only explicitly owned resources may be changed. A file merely residing under
`/var/lib/kubesolo` does not make unrelated neighboring state disposable. External
runtime daemons, sockets, images and unrelated containers remain host-owned.
The API-server/Kine transport uses dedicated loopback mTLS identities; preserve
that boundary separately from cluster client credentials.

`reset` removes the selected cluster datastore and disposable runtime/kubelet
state, retaining PKI, configuration, PV data and offline archives. Ordinary
`uninstall` retains installation data and removes selected configuration unless
`--keep-config` is supplied. **`uninstall --purge` destroys selected owned state**;
there is no `reset --purge` command. Active upgrade receipts block ordinary
cleanup. Inspect the selected path, mounts, receipts and backups before acting.

## Actual management interfaces

These commands are examples of syntax, not evidence that a node is ready:

```sh
rubixctl check
rubixctl check --install-prereqs
rubixctl install --run-mode service --offline-install /media/candidate.tar.gz --path /srv/rubix-staging
rubixctl install --run-mode container --name dev-01 --image registry.example/rubix:reviewed --container-ports 8080:80
rubixctl kubeconfig fetch --output /tmp/rubix-admin.kubeconfig
rubixctl kubeconfig merge
rubixctl d2k fetch --output /tmp/rubix-docker-credentials
rubixctl config path
rubixctl config get network.loadBalancer.enabled
rubixctl config set network.loadBalancer.enabled false
rubixctl upgrade --version v1.3.3 --offline-install /media/candidate.tar.gz
rubixctl upgrade --recover --path /var/lib/kubesolo
rubixctl reset --path /var/lib/kubesolo
rubixctl uninstall --path /var/lib/kubesolo --keep-config
```

Choose the reviewed **distribution version**, artifact and exact installation
path before any effect; Kubernetes v1.35.7 is an upstream input version, not a
Rubix release version. `upgrade --recover` is supplied by the reviewed recovery
change; see its [limitations](migration-and-recovery.md). The parser has no
`container`, `migrate` or `config validate` command. Lifecycle library primitives
are not automatically separate public CLI subcommands.
