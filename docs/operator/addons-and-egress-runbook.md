# Addons, Denied Egress, and D2K Client-Certificate Authentication Runbook

This runbook documents the operational verification procedures for Roadmap Criterion 4
(contributing to parent Epic #350 / Roadmap Issue #355). It details live Linux verification
of offline addon image acquisition, denied egress isolation, LocalPath PVC persistence and
reclaim policies, CoreDNS internal resolution, Portainer bootstrap preservation, and
Docker-to-Kubernetes (D2K) mutual TLS (mTLS) client-certificate authentication.

Consult the authoritative contracts before applying this runbook:
- [Compatibility contract](../architecture/compatibility-contract.md)
- [Component boundary ADR](../../experiments/component-boundary/ADR.md)
- [Upstream inputs and image inventory](../architecture/upstream-inputs.md)
- [Networking and storage reference](networking-and-storage.md)
- [Fresh installation reference](fresh-installs.md)
- [Criterion 4 qualification receipt](../release/receipts/criterion-04-addons-and-egress.json)

---

## 1. Denied Egress Firewall Isolation

To prove that Rubix and its managed addons function strictly offline without relying on
internet connectivity or external registries, the qualification host enforces egress firewall
rules that drop all external outbound non-root and container/pod network traffic while preserving
loopback and cluster-internal communication.

### 1.1 Firewall Configuration

Apply iptables or nftables egress drops:

```sh
# Drop outbound non-root traffic destined for external IP networks
iptables -A OUTPUT -m owner ! --uid-owner 0 -d 0.0.0.0/0 -j DROP

# Ensure loopback traffic is accepted
iptables -I OUTPUT -o lo -j ACCEPT

# Verify outbound internet access fails
curl --connect-timeout 3 http://1.1.1.1 || echo "Egress blocked as expected"
```

In nftables environments, install the drop rule in the host filter output chain.

---

## 2. Offline Image Acquisition

All container images required for Rubix operation—including CoreDNS, Local Path Provisioner,
helper/test utilities, Portainer, and D2K—must be acquired exclusively from the offline
distribution archive (`kubesolo-0.1.0-linux-<arch>-offline.tar.gz`). Zero network image pulls
may occur during startup or test execution.

### 2.1 Authoritative Pinned Image Digests

Per [upstream inputs inventory](../architecture/upstream-inputs.md), the exact pinned image
digests are:

| Component | Repository & Tag | Canonical Digest (SHA-256) |
| --- | --- | --- |
| CoreDNS | `rancher/mirrored-coredns-coredns:1.12.0` | `sha256:4b4ff4458f310f135b51a542bbf89849132ae5674c107771ec8e39063d8cc681` |
| Local Path Provisioner | `rancher/local-path-provisioner:v0.0.30` | `sha256:e0925206ca6eec5038c35d9472e92cff8258b668f18545e851a7be4170313ae6` |
| Helper / Test Image | `rancher/mirrored-library-busybox:1.37.0` | `sha256:498a000f370d8c37927118ed80afe8a86594149023036a7124611005648257d1` |
| Portainer CE | `portainer/portainer-ce:2.27.1` | `sha256:9c0716ce5952f146be48c3f7adab860432130310705a61362e6e14d101d24a91` |
| D2K | `portainer/d2k:2025-02-04` | `sha256:56ff56bcfa0488fbece2a373ff25bdf2559e358b5e68ec347c699fa8406ca504` |

### 2.2 Offline Payload Import & Verification

1. Inspect the offline archive manifest to confirm all required images and tags are present:

   ```sh
   tar -tzf /opt/rubix-offline/kubesolo-0.1.0-linux-arm64-offline.tar.gz
   ```

2. Import the offline archive directly into containerd under the `k8s.io` namespace:

   ```sh
   ctr -n k8s.io images import /opt/rubix-offline/kubesolo-0.1.0-linux-arm64-offline.tar.gz
   ```

3. Confirm that all required images are present with matching digests:

   ```sh
   ctr -n k8s.io images list
   rubix-kube --config /etc/kubesolo/config.yaml --run-mode service --check-images
   ```

4. Confirm containerd records 0 network fetch operations:

   ```sh
   journalctl -u containerd --since "5 minutes ago" | grep -i "pulling image" || echo "Zero pulls confirmed"
   ```

---

## 3. Storage Provisioner & PVC Lifecycle

Rubix packages the Rancher Local Path Provisioner to provide dynamic host-backed storage for
single-node workloads.

### 3.1 Provisioner Deployment

Apply the provisioner manifest staged under `/var/lib/kubesolo/manifests/local-path-storage.yaml`:

```sh
kubectl apply -f /var/lib/kubesolo/manifests/local-path-storage.yaml
kubectl wait --for=condition=Ready pod -l app=local-path-provisioner -n kube-system --timeout=60s
```

### 3.2 PVC Binding and Data Persistence Across Pod Replacement

1. Create a `PersistentVolumeClaim` requesting storage from the `local-path` storage class.
2. Schedule a writer pod (`pvc-writer-pod`) using the offline `rancher/mirrored-library-busybox:1.37.0`
   image that writes a unique verification token to the mounted volume:

   ```sh
   kubectl create -f /opt/rubix/test/pvc-writer-pod.yaml
   kubectl wait --for=condition=Ready pod/pvc-writer-pod --timeout=60s
   ```

3. Delete `pvc-writer-pod`:

   ```sh
   kubectl delete pod pvc-writer-pod
   ```

4. Schedule a reader pod (`pvc-reader-pod`) referencing the same PVC:

   ```sh
   kubectl create -f /opt/rubix/test/pvc-reader-pod.yaml
   kubectl wait --for=condition=Ready pod/pvc-reader-pod --timeout=60s
   ```

5. Verify that the reader pod successfully reads the exact token written by the writer pod:

   ```sh
   kubectl exec pod/pvc-reader-pod -- cat /data/token.txt
   ```

### 3.3 Reclaim Policy: Retain vs. Delete

The Local Path Provisioner supports both `Delete` and `Retain` reclaim policies:

- **`ReclaimPolicy: Retain`**:
  When a PVC bound to a retained PV is deleted, the backing directory on the host filesystem
  (under `/opt/local-path-provisioner/<pvc-namespace>_<pvc-name>_<pv-name>`) remains intact.
  Operators can inspect and recover data after workload decommission.
- **`ReclaimPolicy: Delete`**:
  When a PVC using the default `Delete` reclaim policy is deleted, the provisioner's cleanup
  helper pod mounts the host path and purges the directory contents, ensuring zero residual
  disk leak.

Verify cleanup behavior by inspecting host storage paths:

```sh
ls -la /opt/local-path-provisioner/
```

---

## 4. CoreDNS Internal Resolution Under Denied Egress

CoreDNS provides cluster-internal name resolution without requiring access to upstream internet
DNS servers.

### 4.1 Resolution Check

Deploy a DNS probe pod using the offline `busybox` image:

```sh
kubectl exec pod/dns-test-pod -- nslookup kubernetes.default.svc.cluster.local
```

### 4.2 Expected Outcome

- `kubernetes.default.svc.cluster.local` resolves immediately to `10.43.0.1`.
- Upstream public queries (e.g. `google.com`) fail or time out gracefully due to denied egress,
  without causing CoreDNS service crashes or lookup hanging.

---

## 5. Portainer Bootstrap Preservation

When Portainer CE is deployed as an addon, Rubix reconciles its required Kubernetes resources
(Namespace `portainer`, ServiceAccount, ClusterRole, ClusterRoleBinding, Deployment, Services,
and persistent volume mounts).

### 5.1 Bootstrap Idempotence & State Preservation

1. Trigger initial Portainer bootstrap:

   ```sh
   kubectl apply -f /var/lib/kubesolo/manifests/portainer.yaml
   kubectl wait --for=condition=Ready pod -l app.kubernetes.io/name=portainer -n portainer --timeout=90s
   ```

2. Record the resource UIDs and creation timestamps of all Portainer-owned objects:

   ```sh
   kubectl get deployment -n portainer portainer -o jsonpath='{.metadata.uid}'
   kubectl get pvc -n portainer portainer-data -o jsonpath='{.metadata.uid}'
   ```

3. Trigger a second bootstrap execution (e.g., node restart, reconciler re-evaluation, or
   re-applying the bootstrap manifest).
4. Verify that:
   - Existing deployment, service accounts, and volume claims are preserved without modification.
   - Resource UIDs and persistent volume data remain identical.
   - No duplicate objects or reconcile thrashing occurs.

---

## 6. D2K Mutual TLS (mTLS) Client-Certificate Authentication

The Docker-to-Kubernetes (D2K) daemon provides Portainer with an authenticated adapter to
interact with the Kubernetes cluster and managed containerd runtime. Security policy requires
strict mTLS with dedicated Rubix PKI certificates.

### 6.1 Certificate Authority & Trust Boundary

D2K utilizes a dedicated CA independent of the cluster API server CA:
- CA Certificate: `/var/lib/kubesolo/pki/d2k-ca.crt`
- Server Certificate: `/var/lib/kubesolo/pki/d2k-server.crt`
- Server Private Key: `/var/lib/kubesolo/pki/d2k-server.key`
- Valid Client Certificate: `/var/lib/kubesolo/pki/d2k-client.crt`
- Valid Client Private Key: `/var/lib/kubesolo/pki/d2k-client.key`

### 6.2 Positive Verification: Authenticated Request

Execute an HTTPS request presenting the authorized client certificate:

```sh
curl --cacert /var/lib/kubesolo/pki/d2k-ca.crt \
     --cert /var/lib/kubesolo/pki/d2k-client.crt \
     --key /var/lib/kubesolo/pki/d2k-client.key \
     https://127.0.0.1:9443/version
```

**Expected Result**: TLS handshake succeeds; endpoint returns `HTTP 200 OK` with daemon version metadata.

### 6.3 Negative Verification: Missing Client Certificate

Execute an HTTPS request omitting client certificates:

```sh
curl --cacert /var/lib/kubesolo/pki/d2k-ca.crt \
     https://127.0.0.1:9443/version
```

**Expected Result**: Request is rejected during TLS negotiation with a client certificate required
alert (`bad certificate` or `certificate required`), or returns `HTTP 401 Unauthorized`. The
connection is aborted without processing the request body.

### 6.4 Negative Verification: Foreign/Untrusted Certificate

Generate a temporary self-signed client certificate using an untrusted foreign CA:

```sh
openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
  -keyout /tmp/foreign.key -out /tmp/foreign.crt -subj "/CN=unauthorized-client"

curl --cacert /var/lib/kubesolo/pki/d2k-ca.crt \
     --cert /tmp/foreign.crt \
     --key /tmp/foreign.key \
     https://127.0.0.1:9443/version
```

**Expected Result**: TLS handshake fails with `SSL alert number 48 (unknown CA / bad certificate)`
or returns `HTTP 403 Forbidden`. The request is dropped immediately.

### 6.5 Endpoint Readiness Gated on Live Container Health Probe

D2K service endpoint readiness is strictly gated on the live container health probe (`/healthz` or
configured HTTP readiness probe). The endpoint must not be marked `Ready` merely because the pod
controller reconciled or the pod scheduled; it transitions to `Ready` only after the live container
process begins responding affirmatively to health checks over the authenticated transport.

Verify endpoint readiness transition:

```sh
kubectl get endpoints -n portainer d2k -o jsonpath='{.subsets[0].addresses[*].ip}'
```

---

## 7. Qualification Receipt Validation

The qualification results for Criterion 4 are captured in a tamper-evident, candidate-bound receipt:
[`docs/release/receipts/criterion-04-addons-and-egress.json`](../release/receipts/criterion-04-addons-and-egress.json).

To validate the receipt and evaluate Criterion 4 against candidate inventory:

```sh
cargo test -p rubix-dev --test suite -- test_generate_and_validate_criterion_4_receipt
cargo run -p rubix-dev --bin rubix-qualification
```

The receipt validator verifies:
1. Candidate source revision and artifact digests match `cell-inventory.json` and `SHA256SUMS`.
2. All 15 qualification assertions passed.
3. Cleanup status is `complete` with 0 leaked containers and 0 leaked images.
4. Canonical SHA-256 payload integrity hash matches the receipt.
