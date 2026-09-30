# rubix-apiserver

`rubix-apiserver` provides the Kubernetes API server integration, options negotiation, storage readiness gating, PKI prerequisites enforcement, and supervised lifecycle adapter for Rubix.

## Overview

- **Official Kubernetes v1.35.7 Baseline**: Implements standard API-server options, endpoint negotiation, and RBAC security models without obsolete edge overrides.
- **etcd Datastore Engine**: Directly backed by `rubix-datastore`'s revision engine, ensuring strong MVCC, resource versioning, and watch event delivery.
- **Supervised Lifecycle**: Integrates with `rubix-supervisor` via `ApiserverAdapter`, ensuring strict ordering with datastore readiness and PKI generation.
- **Prerequisite Validation**: Rejects startup and fails readiness when PKI credentials or etcd storage are invalid or unavailable.
- **Client & Admission Integration**: Supports discovery, authenticated CRUD/list/watch, and security rejection for unauthenticated/unauthorized callers.
