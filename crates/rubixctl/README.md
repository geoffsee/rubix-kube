# Management checks and explicit prerequisites

`rubixctl` is a separate management executable. Its current commands are `check`,
`version` and help. The existing `rubix-kube` startup parser, startup YAML and its
90-case qualification remain separate. This crate does not start Kubernetes or provide a general installer. Explicit
opt-in can install missing Alpine networking packages and enable its cgroups
service; ordinary checks never prepare a host or stop conflicting processes.

The name is an explicit Rubix management CLI choice replacing `kubesoloctl` for
this executable. Help/version text is branded accordingly. KubeSolo environment
keys, on-disk paths, service/runtime/CNI identities and configuration schemas do not
change. No compatibility alias or release packaging is implied; installers and
existing scripts must choose the correct executable when packaging is introduced.
The crate remains private workspace code and adds no registry dependency.

## Interface and effects

`parse_command` accepts argument strings and an explicit environment map.
`execute` injects `CheckInputs` and output writers; `execute_check` consumes parsed
options. Help, version, parse errors and unsupported preparation require no host
observations. Errors use static categories rather than echoing arbitrary arguments,
file contents, environment values, hostnames or raw OS errors. Output failures
propagate to a nonzero process exit.

`check` accepts `--pprof-server` and `--install-prereqs`. Environment defaults are
`KUBESOLO_PPROF_SERVER` and `KUBESOLO_INSTALL_PREREQS`; exact lowercase `true`, `1`
and `yes` enable them, `false`, `0`, `no` and other values default false. Explicit
flags override environment. Boolean `=value` uses Go ParseBool spellings; separate
boolean-looking words remain positional arguments and are ignored, as by Cobra.
Repeated flags use the final value. `--` ends flag parsing. There is no YAML or
startup-flag processing here.

Explicit `--install-prereqs` (or its existing environment opt-in) authorizes only
missing Alpine networking packages and the Alpine cgroups service. The executable
installs invocation-wide signal handling before observations or actions. See
[PREPARATION.md](PREPARATION.md) for fixed commands, reobservation, partial effects
and incomplete-cleanup exit status. The synchronous library `execute` boundary
remains read-only and rejects this opt-in; the executable uses the async workflow.

The host check uses Linux discovery and supplemental filesystem observations.
It evaluates earlier checks before briefly binding wildcard ports; any earlier
blocker/unknown/preparation requirement prevents all port probing. The port phase
checks 2379, 6443, 10443 and optionally 6060, releases its own listeners, and
reevaluates the report. The command explicitly selects managed baseline context; supplemental input
contains only filesystem observations, never execution controls or port evidence. It has no authority to kill another listener. Passing observations do not
reserve ports or establish runtime readiness.

Non-Linux host checking is explicitly unsupported. Unlike baseline macOS
`kubesoloctl check`, this slice does not contact a container engine; that consumer
remains separate. Help and version remain usable without Linux host support.

## Compatibility evidence

The read-only baseline `2ef1c4787989f11f868f81bb84ae2afd4a49a81d` supplies
`internal/cli/{root.go,cmd_check.go,cmd_version.go,helpers.go}`, CLI config and UI.
The actual Cobra 1.10.2/pflag 1.0.10 source oracle under
`tools/parity/fixtures/management-check` captures 56 parsing/output cases twice.
It injects a harmless check effect marker and inert sibling registrations rather
than running an installer or claiming full management-command support. Rust consumes
independently specified expected parser outcomes, not its own rendered output.

Observed precedence includes `--help check` producing root help while
`--help=true check` produces check help; malformed flags still fail when help is
present. Extra check/version arguments are ignored. Unknown help topics retain
Cobra's exit 0 but use a static safe diagnostic. `version` is a management subcommand;
`check --version` is an error. Startup Kingpin behavior is not reused here.

Intentional changes are bounded argv acceptance (256 arguments/64KiB), UTF-8 command
input, rebranded/smaller help, static safe diagnostics, propagated output failures,
explicit unsupported container-engine checks, and a qualified success
message. The baseline's universal "Host is ready" message would overstate these
observations. ANSI styling, byte-identical management UI and unimplemented sibling
commands are not claimed. E05 integration and E23 installation remain open.

```sh
cargo test -p rubixctl --locked
cargo clippy -p rubixctl --all-targets --locked -- -D warnings
```

Real host-check executable qualification runs only in disposable Linux containers;
ordinary tests use injected effects or invoke only help/version.
