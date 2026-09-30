# rubix-cri

Client bindings, endpoint validation, provider detection, and external CRI runtime attachment for Kubernetes CRI v1.

## Features

- **CRI v1 Protocol Client**: Client bindings for the official Kubernetes v1.35.7 CRI protocol (`runtime.v1`), generated deterministically and verified with `rubix-upstream check-cri`.
- **Endpoint Parsing & Validation**:
  - Validates Unix domain socket endpoints supporting raw absolute paths (e.g. `/run/containerd/containerd.sock`) and canonical `unix://` URIs (e.g. `unix:///var/run/crio/crio.sock`).
  - Supports separate runtime and image service endpoints, automatically defaulting image service to runtime socket if not explicitly specified.
  - Rejects unsupported URI schemes (`tcp://`, `http://`, etc.), relative paths, and empty paths with actionable errors.
- **Provider Detection & Classification**:
  - Classifies external CRI providers into `CriProvider::Containerd` and `CriProvider::Crio`.
  - Rejects unsupported or legacy runtimes (such as `dockershim`, `docker`, `frakti`) with clear error messaging.
- **Bounded Readiness Probing**:
  - Validates connectivity and responsiveness against both `RuntimeService` (`Version` RPC) and `ImageService` (`ListImages` RPC).
  - Enforces bounded timeouts and configurable retry intervals.
- **External Lifecycle Attachment**:
  - Supervised by `rubix-supervisor` via `ExternalRuntimeService` (`COMPONENT_EXTERNAL_CRI`).
  - Acts strictly as a client: does not launch child processes, extract local payloads, mutate host sockets, or purge host runtime state.
  - Cooperatively stops when supervisor requests shutdown.
