# State Transition Report: v1.1.8 -> Rubix v0.1.0

## 1. Supported Version & Starting State
- **Starting Version**: `v1.1.8`
- **Layout**: Legacy flags in service unit, persistent PKI, Kine SQLite datastore
- **Config File Native Support**: false
- **Flag Migration Required**: true

## 2. Configuration Transition
- **Migrated from Service Flags**: true
- **Config File**: `/etc/kubesolo/config.yaml`
- **Service Backup**: `Some("/etc/kubesolo/config.yaml.bak")`
- **File Permissions 0600**: true
- **Node IP**: `10.0.0.10`
- **Disable IPv6**: false
- **Edge ID**: `edge-test-cluster`

## 3. PKI Trust Roots & Client Credentials
- **CA Root Preserved**: true
- **CA Private Key Preserved**: true
- **ServiceAccount Key Preserved**: true
- **Kubeconfig Format Tested**: YAML
- **Client Cert Authenticated by CA**: true

## 4. Native Snapshot Experiment (not production Kine migration)
- **Raw SQLite Rejected by Rubix-Datastore**: true *(proven rather than assumed)*
- **Explicit Export Format**: `RUBXSNP1`
- **Restored Active Keys**: 35
- **Monotonic Revisions Preserved**: true
- **Restored Revision**: 100
- **Keys Identical**: true

## 5. Persistent Volume Storage
- **Volumes**: 2
- **Files Preserved**: 15
- **Total Bytes**: 40960
- **All File Checksums Match**: true
- **All Permissions Match**: true

## 6. Operational Prerequisites
- **Nonportable State Items Classified**: 10
- **Estimated Downtime**: ~10 minutes (unmeasured)
- **Required Backups Verified**: true
- **Fixture Checks Successful**: **PASS**
- **Production Migration Qualification**: **UNQUALIFIED** (no live Kine transition evidence)
