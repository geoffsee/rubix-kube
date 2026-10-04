# Fresh installation and container lifecycle reference

A usable retained-executable node requires a complete verified candidate and
current-source disposable Linux evidence. This runbook does not qualify universal
host installation or application readiness. Consult the
[compatibility contract](../architecture/compatibility-contract.md) and
[management implementation notes](../../crates/rubixctl/README.md).

## Preflight

```sh
rubixctl check
rubixctl check --install-prereqs
```

The read-only check selects managed baseline context and observes Linux host
capabilities and filesystem facts. Only after earlier checks permit probing does
it briefly bind 2379, 6443 and 10443, plus 6060 when `--pprof-server` is enabled.
It does not test every metrics/D2K port, reserve ports, establish CRI connectivity
or prove node readiness. Non-Linux host checking is unsupported.

Explicit prerequisite preparation permits missing Alpine `iproute2`/`iptables`
packages and the cgroups OpenRC service. Other preparation targets are rejected;
this is not a general host package installer. Ordinary checks never stop a
conflicting process or modify the host.

## Universal host installation: implemented staging boundary

Online host `install` is not implemented. Service-mode `--offline-install` checks
host architecture/libc, copies the archive privately, rejects unsafe entries,
verifies `bundle.manifest`, then publishes its declared files into `--path`.
It returns after staging: it does not generate configuration, register/initiate
an init service or place a complete running node at `/usr/local/bin`.

```sh
rubixctl install --run-mode service --offline-install /media/candidate.tar.gz --path /srv/rubix-staging
```

`candidate.tar.gz` must actually have a canonical matching archive filename and
valid bundle metadata; this placeholder is not a downloadable release. Choose
the distribution version and target as described in [air-gap delivery](air-gap-deployment.md).
The installer accepts service/container modes; `--init` is not a CLI flag.

Service generation/lifecycle adapters exist for systemd, OpenRC, SysVinit,
Upstart, runit and s6. Their presence does not make host installation automatic
or qualify all six live supervisors. Before manually activating a service,
inspect the generated definition, exact binary/configuration paths, supervisor
scan/link ownership and retained executable assets. Use the selected backend's
own documented controls and preserve unrelated services.

## Minimal manual host preparation

Do not extract an unaudited archive directly into `/usr/local/bin`. Obtain a
reviewed, digest-verified candidate and use private verified staging. Preserve
retained executables, runtime shims, CNI programs, images and their inventory;
copying only the management/node binaries is insufficient.

The configuration schema and default paths remain KubeSolo-compatible. An
example partial configuration is:

```yaml
apiVersion: kubesolo.io/v1alpha1
kind: Config
path: /var/lib/kubesolo
logging:
  debug: false
network:
  nodeIP: ""
  mtu: 0
  disableIPv6: false
  loadBalancer:
    enabled: true
runtime:
  endpoint: ""
storage:
  localPath:
    enabled: true
metrics:
  enabled: false
  bindAddress: "127.0.0.1:9105"
```

Store sensitive configuration at `/etc/kubesolo/config.yaml` with mode 0600.
Use the checked-in [default example](../../crates/rubix-config/examples/default-config.yaml)
and validate effective startup resolution on the actual host. `--print-config`
can disclose credentials; protect its output.

```sh
rubix-kube --config /etc/kubesolo/config.yaml --print-config
```

Foreground/Daemon process and service plans require a separate verified manual
launch setup; `install --run-mode foreground` and `daemon` are rejected by the
current executable. Supervision and signal-handling tests do not establish live
host startup qualification. Do not create an ad-hoc PID file and assume it grants
ownership of every process or a complete 30-second node shutdown guarantee.

## Named container installation

A Linux Docker Engine is required on Linux, macOS or WSL2. The adapter queries
Engine architecture; a container creation result is not Kubernetes readiness.
The implemented entry point is:

```sh
rubixctl install --run-mode container --name dev-01 --image registry.example/rubix:reviewed --container-ports 8080:80,8443:443
```

Explicit `--image` may pull from a registry and overrides an offline bundle. For
verified offline import, omit that override and use [the offline interface](air-gap-deployment.md).
Named identities retain compatibility: `kubesolo-dev-01`,
`kubesolo-dev-01-net`, `kubesolo-dev-01-data`; the default `rubix` name maps to
`kubesolo`. Port mappings default to loopback. The API and enabled D2K endpoints
use Engine-observed published ports; reserved workload mappings cannot replace
them. Inspect the actual returned endpoints rather than assuming an address.

Status/start/stop/restart/remove primitives exist in the container lifecycle
library, but there is no public `rubixctl container` command. Use explicitly
selected Docker Engine controls for operational inspection. Reset/uninstall
select the instance via the persisted `container.spec` at `--path`, not a
`--name`/`--run-mode` option. Do not direct cleanup at a path lacking the intended
spec: it may select host-service cleanup. Named-volume and ambiguous configuration
bindings can require explicit operator cleanup and fail before lifecycle effects.

## Client access

```sh
rubixctl kubeconfig fetch --output /tmp/rubix-admin.kubeconfig
rubixctl kubeconfig merge
```

`merge` is its own subcommand, not a `fetch --merge` flag. Fetch/parse supports
YAML and JSON kubeconfig, but syntactic parsing is not authenticated live API
access. Test the selected context with a real client only after the actual node
is running, and retain private-key permissions and separate instance identities.
