# rubix-apiserver

`rubix-apiserver` provides the Kubernetes API server integration, options negotiation, storage readiness gating, PKI prerequisites enforcement, and supervised lifecycle adapter for Rubix.

## Overview

- **Official Kubernetes v1.35.7 Baseline**: Implements standard API-server options, endpoint negotiation, and RBAC security models without obsolete edge overrides.
- **etcd Datastore Engine**: Directly backed by `rubix-datastore`'s revision engine, ensuring strong MVCC, resource versioning, and watch event delivery.
- **Supervised Lifecycle**: Integrates with `rubix-supervisor` via `ApiserverAdapter`, ensuring strict ordering with datastore readiness and PKI generation.
- **Prerequisite Validation**: Rejects startup and fails readiness when PKI credentials or etcd storage are invalid or unavailable.
- **Client & Admission Integration**: Supports discovery, authenticated CRUD/list/watch, and security rejection for unauthenticated/unauthorized callers.
- **HTTPS gateway for kubectl**: `ApiserverAdapter` binds a loopback mTLS listener (client certificates from the cluster CA, bearer tokens, or anonymous) and maps Kubernetes paths onto `KubernetesApiClient`. It serves legacy and aggregated discovery, JSON and a partial Kubernetes protobuf create path, `GET .../{name}/status`, and `GET .../pods/{name}/log`. Pod logs come from a `PodLogReader` that the in-process kubelet registers with `ApiserverService::set_pod_log_reader`; the reader must not hold the service itself, or shutdown cannot release the datastore lock. `/openapi/v3` and `/openapi/v3/api/v1` serve a minimal document that only tells kubectl the core kinds accept `fieldValidation`, so `kubectl create` works without `--validate=false`; there is no schema, no OpenAPI v2, and no HTTP watch stream.
