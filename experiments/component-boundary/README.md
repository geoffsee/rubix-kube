# Kubernetes / Kine component boundary experiment

E01.02 candidate: supervise the official Kubernetes v1.35.7 API server and Kine
v0.16.3 CGO executable as separate processes. This experiment does not start a
kubelet, container runtime, scheduler, addon or workload. It tests the protocol
boundary with a Python test driver; production Rust supervision remains E04 work.

Run from a machine with Docker and Python 3.10 or later:

```sh
python3 experiments/component-boundary/run.py --output /tmp/rubix-boundary-evidence
```

The output directory must not exist. Image preparation downloads checksum-locked
release binaries and installs OpenSSL. Runtime uses an unprivileged, network-isolated
container, all capabilities dropped, no host mounts, no published ports, and a
1 GiB memory limit. The runner removes only its uniquely named container and image.
Docker build caches remain ordinary Docker-managed caches. It never operates an
existing cluster. The component run has a 600-second timeout; image preparation has a separate
900-second timeout and each diagnostic/cleanup operation has a 30-second timeout.
Independent diagnostic collection continues after an individual collection failure.
`runner-result.json` records orchestration and teardown failures; either makes the
runner exit nonzero even if component assertions passed.

`result.json` records input identities, assertion results, process RSS/high-water
RSS, startup duration, and shutdown outcomes. Component logs and Docker image/container
metadata are retained even when assertions fail. Credentials and SQLite state stay
in the disposable container; evidence excludes their secret contents.

The experiment requires:

- TLS certificate verification and client-certificate authenticated create/read/update/delete;
- rejection of unauthenticated requests and an authenticated identity without RBAC access;
- clean stop of both components, an independent SQLite integrity/key check, and recovery
  of the identical object UID and updated value after restarting both components;
- acknowledged-update persistence after an intentionally injected datastore SIGKILL,
  recovery of readiness in the same API process after restarting only Kine, followed
  by deletion persistence through a further restart;
- bounded SIGTERM shutdown, reaping children, and absence of surviving owned process groups.
  Forced cleanup makes the experiment fail, even if CRUD succeeded.

Hashes in `inputs.json` were obtained from the official Kubernetes checksum
endpoints and Kine's release API asset digests. Every download and runtime binary is
checked against these committed values. The Docker base is pinned by manifest digest.
The OpenSSL package repository is not snapshot-locked; retain `image-inspect.json`
for exact experiment-image identity. This is a bounded integration experiment, not
reproducible release artifact assembly.

This experiment currently prepares only Linux arm64 and amd64 artifacts. Other
supported product platforms remain qualification requirements. Measurements describe
two retained components inside this fixture, not whole-distribution budgets, idle
steady-state measurements, performance comparisons, or Kubernetes conformance.

See [the candidate decision](ADR.md). A passing experiment supplies evidence for the
candidate; it does not by itself close E01 or establish K3s-fork behavioral parity.

SQLite DSN parameters explicitly match Kine v0.16.3 `sqlite.DefaultParams`, used
by the KubeSolo reference: WAL, 30-second busy timeout, NORMAL synchronization,
immediate transactions, statement cache 20, shared cache. Pool and notification
settings also match the reference. Power-loss durability is not established by SIGKILL.

The API server advertises the synthetic documentation address `192.0.2.1` because
Kubernetes rejects loopback addresses in its reconciled default Service Endpoints.
The fixture has no workload clients or routes to that address: test clients connect
only to TLS `127.0.0.1:6443`, and the listener remains loopback-bound. Service endpoint
reachability is outside this protocol experiment and remains a networking integration
gate. Readiness requires the complete authenticated `/readyz` response to succeed,
including post-start hooks; a reachable socket alone is not sufficient.

Source SHA-256 identities are recorded by the runner at invocation for all fixture
build inputs and by the container for the executed `spike.py`. Retain these alongside
the eventual Git revision when archiving runs prepared before a commit.

The ADR records an observed failure to stop the API gracefully while Kine remains
unavailable. The current crash scenario restores Kine and verifies API recovery before
normal dependency-ordered shutdown; it does not claim to resolve that shutdown limitation.
