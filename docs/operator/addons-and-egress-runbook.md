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
- [Acceptance matrix](../architecture/acceptance-matrix.md)

---

## 1. Denied Egress Firewall Isolation

To prove that Rubix and its managed addons function strictly offline without relying on
internet connectivity or external registries, the qualification host enforces comprehensive
egress firewall rules. These rules drop all outbound host traffic (including root processes,
containerd, and kubelet) and forwarded container/pod traffic destined for external IP networks,
while strictly preserving loopback and cluster-internal CIDRs (`10.42.0.0/16` for pods and
`10.43.0.0/16` for services).

### 1.1 Firewall Rules Installation

Apply iptables egress and forwarding drop rules using a dedicated chain with explicitly controlled ordering at position 1:

```sh
# Create custom filter chain with controlled ordering
iptables -N RUBIX_EGRESS_BLOCK 2>/dev/null || iptables -F RUBIX_EGRESS_BLOCK

# 1. Allow established and related connections
iptables -A RUBIX_EGRESS_BLOCK -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT

# 2. Allow loopback traffic on host
iptables -A RUBIX_EGRESS_BLOCK -o lo -j ACCEPT

# 3. Allow cluster-internal communication (Pod CIDR 10.42.0.0/16 and Service CIDR 10.43.0.0/16)
iptables -A RUBIX_EGRESS_BLOCK -d 10.42.0.0/16 -j ACCEPT
iptables -A RUBIX_EGRESS_BLOCK -d 10.43.0.0/16 -j ACCEPT
iptables -A RUBIX_EGRESS_BLOCK -s 10.42.0.0/16 -d 10.42.0.0/16 -j ACCEPT
iptables -A RUBIX_EGRESS_BLOCK -s 10.42.0.0/16 -d 10.43.0.0/16 -j ACCEPT
iptables -A RUBIX_EGRESS_BLOCK -d 10.42.0.0/16 -j ACCEPT

# 4. Drop all other outbound and forwarded traffic
iptables -A RUBIX_EGRESS_BLOCK -j DROP

# Insert RUBIX_EGRESS_BLOCK at rule position 1 in OUTPUT and FORWARD to ensure no earlier rules bypass it
iptables -D OUTPUT -j RUBIX_EGRESS_BLOCK 2>/dev/null || true
iptables -I OUTPUT 1 -j RUBIX_EGRESS_BLOCK

iptables -D FORWARD -j RUBIX_EGRESS_BLOCK 2>/dev/null || true
iptables -I FORWARD 1 -j RUBIX_EGRESS_BLOCK

# Verify RUBIX_EGRESS_BLOCK jumps are active at rule position 1
iptables -C OUTPUT -j RUBIX_EGRESS_BLOCK 2>/dev/null && \
  iptables -C FORWARD -j RUBIX_EGRESS_BLOCK 2>/dev/null || {
    echo "ERROR: RUBIX_EGRESS_BLOCK jump rules failed to install" >&2
    exit 1
  }

# 5. Enforce IPv6 fail-closed policy
if command -v ip6tables >/dev/null 2>&1; then
  ip6tables -P OUTPUT DROP && \
    ip6tables -P FORWARD DROP && \
    ip6tables -S OUTPUT | grep -Fqx -- '-P OUTPUT DROP' && \
    ip6tables -S FORWARD | grep -Fqx -- '-P FORWARD DROP' || {
      echo "ERROR: IPv6 firewall policies were not applied" >&2
      exit 1
    }
elif [ "$(cat /proc/sys/net/ipv6/conf/all/disable_ipv6 2>/dev/null)" = "1" ]; then
  echo "IPv6 is disabled"
else
  echo "ERROR: IPv6 is neither disabled nor firewalled" >&2
  exit 1
fi
```

### 1.2 Firewall Verification Probes

Verify both host-level and pod-level isolation:

1. **Host-Level External Probe (Root UID 0)**:
   Verify root outbound WAN traffic is dropped using a transport-independent TCP probe:
   ```sh
   # Direct TCP probe to external WAN IP (1.1.1.1:80).
   # Connection must fail due to packet drop / timeout.
   if nc -z -w 2 1.1.1.1 80 2>/dev/null; then
     echo "ERROR: Host external TCP connection succeeded; egress drop failed" >&2
     exit 1
   else
     echo "Host root TCP egress denied as expected"
   fi
   ```

2. **Host-Level Cluster Loopback Probe**:
   Verify local cluster communication succeeds:
   ```sh
   curl --connect-timeout 2 --silent -k https://127.0.0.1:6443/version
   ```

3. **Pod-Level External Probe**:
   Verify pods cannot egress to public IPs or external DNS, asserting probe readiness and execution separately:
   ```sh
   # Deploy probe pod and wait for readiness
   cat <<EOF | kubectl apply -f -
   apiVersion: v1
   kind: Pod
   metadata:
     name: egress-probe
     namespace: default
   spec:
     restartPolicy: Never
     containers:
       - name: probe
         image: rancher/mirrored-library-busybox:1.37.0@sha256:498a000f370d8c37927118ed80afe8a86594149023036a7124611005648257d1
         command: ["sh", "-c", "sleep 3600"]
   EOF
   kubectl wait --for=condition=Ready pod/egress-probe --timeout=60s

   # Verify internal connectivity succeeds
   kubectl exec pod/egress-probe -- nc -z -w 2 10.43.0.1 443 || {
     echo "ERROR: Cluster internal probe failed; check pod network" >&2
     exit 1
   }

   # Verify external connectivity fails
   if kubectl exec pod/egress-probe -- nc -z -w 2 1.1.1.1 53; then
     echo "ERROR: Pod was able to reach external IP; egress drop failed" >&2
     exit 1
   else
     echo "Pod egress denied as expected"
   fi
   kubectl delete pod egress-probe
   ```

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

1. Record qualification start timestamp prior to starting node services to establish the audit interval (including explicit UTC timezone suffix):

   ```sh
   QUALIFICATION_START_TIME=$(date -u +"%Y-%m-%d %H:%M:%S UTC")
   echo "Qualification start recorded: $QUALIFICATION_START_TIME"
   ```

2. Parameterize archive path by target architecture, inspect manifest, and import into containerd:

   ```sh
   ARCH=$(uname -m | sed -e 's/x86_64/amd64/' -e 's/aarch64/arm64/')
   ARCHIVE_PATH="/opt/rubix-offline/kubesolo-0.1.0-linux-${ARCH}-offline.tar.gz"

   tar -tzf "$ARCHIVE_PATH"
   ctr -n k8s.io images import "$ARCHIVE_PATH"
   ```

4. Confirm that all required images are present with matching digests:

   ```sh
   ctr -n k8s.io images list
   rubix-kube --config /etc/kubesolo/config.yaml --run-mode service --check-images
   ```

5. Rigorously verify zero remote network image pulls across the entire qualification interval:

   Do not rely on negative grep matches over unverified log streams. Establish zero network pulls by:
   - Confirming containerd logging is active and accessible:
     ```sh
     journalctl -u containerd -n 5 >/dev/null || { echo "ERROR: containerd journal unavailable"; exit 1; }
     ```
   - Verifying all required image tags exist in containerd without remote pull records:
     ```sh
     ctr -n k8s.io images list -q > /tmp/imported-images.txt
     for img in \
       "rancher/mirrored-coredns-coredns:1.12.0" \
       "rancher/local-path-provisioner:v0.0.30" \
       "rancher/mirrored-library-busybox:1.37.0" \
       "portainer/portainer-ce:2.27.1" \
       "portainer/d2k:2025-02-04"; do
       grep -F "$img" /tmp/imported-images.txt || { echo "ERROR: Missing image $img"; exit 1; }
     done
     ```
   - Verifying containerd logs contain zero remote fetch operations since qualification started:
     ```sh
     PULLS=$(journalctl -u containerd --since "$QUALIFICATION_START_TIME" | grep -iE 'pulling image|downloading layer|fetching' || true)
     if [ -n "$PULLS" ]; then
       echo "ERROR: Remote pull events detected since qualification start ($QUALIFICATION_START_TIME):"
       echo "$PULLS"
       exit 1
     fi
     echo "Zero remote pulls confirmed across containerd journal since $QUALIFICATION_START_TIME"
     ```
   - Inspecting firewall drop counters to confirm no outbound traffic escaped:
     ```sh
     iptables -L OUTPUT -v -n | grep DROP
     iptables -L FORWARD -v -n | grep DROP
     ```

---

## 3. Storage Provisioner & PVC Lifecycle

Rubix packages the Rancher Local Path Provisioner to provide dynamic host-backed storage for
single-node workloads.

### 3.1 Provisioner Deployment

Apply the provisioner manifest staged under `/var/lib/kubesolo/manifests/local-path-storage.yaml`:

```sh
kubectl apply -f /var/lib/kubesolo/manifests/local-path-storage.yaml
kubectl wait --for=condition=Ready pod -l app=local-path-provisioner -n local-path-storage --timeout=60s
```

### 3.2 PVC Binding and Data Persistence Across Pod Replacement

1. Create a `PersistentVolumeClaim` requesting storage from the `local-path` storage class:

   ```sh
   cat <<EOF | kubectl apply -f -
   apiVersion: v1
   kind: PersistentVolumeClaim
   metadata:
     name: local-path-pvc-test
     namespace: default
   spec:
     accessModes:
       - ReadWriteOnce
     storageClassName: local-path
     resources:
       requests:
         storage: 128Mi
   EOF
   ```

2. Schedule a writer pod (`pvc-writer-pod`) using the offline pinned BusyBox image that writes a unique verification token to the mounted volume:

   ```sh
   cat <<EOF | kubectl apply -f -
   apiVersion: v1
   kind: Pod
   metadata:
     name: pvc-writer-pod
     namespace: default
   spec:
     restartPolicy: Never
     containers:
       - name: writer
         image: rancher/mirrored-library-busybox:1.37.0@sha256:498a000f370d8c37927118ed80afe8a86594149023036a7124611005648257d1
         command: ["sh", "-c", "echo 'rubix-storage-verification-token-42' > /data/token.txt && sync"]
         volumeMounts:
           - name: storage
             mountPath: /data
     volumes:
       - name: storage
         persistentVolumeClaim:
           claimName: local-path-pvc-test
   EOF
   kubectl wait --for=jsonpath='{.status.phase}'=Succeeded pod/pvc-writer-pod --timeout=60s
   EXIT_CODE=$(kubectl get pod pvc-writer-pod -o jsonpath='{.status.containerStatuses[0].state.terminated.exitCode}')
   test "$EXIT_CODE" = "0" || { echo "ERROR: Writer pod failed with exit code $EXIT_CODE"; exit 1; }
   ```

3. Delete `pvc-writer-pod`:

   ```sh
   kubectl delete pod pvc-writer-pod
   ```

4. Schedule a reader pod (`pvc-reader-pod`) referencing the same PVC:

   ```sh
   cat <<EOF | kubectl apply -f -
   apiVersion: v1
   kind: Pod
   metadata:
     name: pvc-reader-pod
     namespace: default
   spec:
     restartPolicy: Never
     containers:
       - name: reader
         image: rancher/mirrored-library-busybox:1.37.0@sha256:498a000f370d8c37927118ed80afe8a86594149023036a7124611005648257d1
         command: ["sh", "-c", "sleep 3600"]
         volumeMounts:
           - name: storage
             mountPath: /data
     volumes:
       - name: storage
         persistentVolumeClaim:
           claimName: local-path-pvc-test
   EOF
   kubectl wait --for=condition=Ready pod/pvc-reader-pod --timeout=60s
   ```

5. Verify that the reader pod successfully reads the exact token written by the writer pod:

   ```sh
   TOKEN=$(kubectl exec pod/pvc-reader-pod -- cat /data/token.txt)
   test "$TOKEN" = "rubix-storage-verification-token-42" && echo "Data persistence verified"
   kubectl delete pod pvc-reader-pod
   ```

### 3.3 Reclaim Policy Verification: Retain vs. Delete

The Local Path Provisioner supports both `Delete` and `Retain` reclaim policies. Host-backed directories follow the naming pattern `/opt/local-path-provisioner/<pv-name>_<pvc-namespace>_<pvc-name>`.

#### 3.3.1 Delete Reclaim Policy Verification (Default)

1. Identify the backing host path for `local-path-pvc-test`:

   ```sh
   PV_NAME=$(kubectl get pvc local-path-pvc-test -o jsonpath='{.spec.volumeName}')
   HOST_PATH="/opt/local-path-provisioner/${PV_NAME}_default_local-path-pvc-test"
   test -d "$HOST_PATH" && echo "Backing host directory exists: $HOST_PATH"
   ```

2. Delete the PVC:

   ```sh
   kubectl delete pvc local-path-pvc-test
   ```

3. Verify provisioner cleanup helper pod purges the directory contents, polling with a timeout to accommodate asynchronous helper pod execution:

   ```sh
   CLEANED=0
   for i in $(seq 1 30); do
     if [ ! -d "$HOST_PATH" ]; then
       CLEANED=1
       break
     fi
     sleep 2
   done

   if [ "$CLEANED" -eq 1 ]; then
     echo "Backing host path purged on Delete reclaim policy"
   else
     echo "ERROR: Host directory $HOST_PATH was not purged within timeout" >&2
     exit 1
   fi
   ```

#### 3.3.2 Retain Reclaim Policy Verification

1. Define a retained storage class and create a PVC:

   ```sh
   cat <<EOF | kubectl apply -f -
   apiVersion: storage.k8s.io/v1
   kind: StorageClass
   metadata:
     name: local-path-retain
   provisioner: rancher.io/local-path
   reclaimPolicy: Retain
   volumeBindingMode: Immediate
   EOF

   cat <<EOF | kubectl apply -f -
   apiVersion: v1
   kind: PersistentVolumeClaim
   metadata:
     name: local-path-retain-pvc
     namespace: default
   spec:
     accessModes:
       - ReadWriteOnce
     storageClassName: local-path-retain
     resources:
       requests:
         storage: 128Mi
   EOF
   ```

2. Write data to volume via a temporary pod and wait for Succeeded completion:

   ```sh
   cat <<EOF | kubectl apply -f -
   apiVersion: v1
   kind: Pod
   metadata:
     name: retain-writer
     namespace: default
   spec:
     restartPolicy: Never
     containers:
       - name: writer
         image: rancher/mirrored-library-busybox:1.37.0@sha256:498a000f370d8c37927118ed80afe8a86594149023036a7124611005648257d1
         command: ["sh", "-c", "echo 'retained-data' > /data/retain.txt && sync"]
         volumeMounts:
           - name: storage
             mountPath: /data
     volumes:
       - name: storage
         persistentVolumeClaim:
           claimName: local-path-retain-pvc
   EOF
   kubectl wait --for=jsonpath='{.status.phase}'=Succeeded pod/retain-writer --timeout=60s
   EXIT_CODE=$(kubectl get pod retain-writer -o jsonpath='{.status.containerStatuses[0].state.terminated.exitCode}')
   test "$EXIT_CODE" = "0" || { echo "ERROR: Retain writer pod failed with exit code $EXIT_CODE"; exit 1; }
   kubectl delete pod retain-writer
   ```

3. Record backing host path:

   ```sh
   RETAIN_PV=$(kubectl get pvc local-path-retain-pvc -o jsonpath='{.spec.volumeName}')
   RETAIN_HOST_PATH="/opt/local-path-provisioner/${RETAIN_PV}_default_local-path-retain-pvc"
   test -f "$RETAIN_HOST_PATH/retain.txt" && echo "Host file verified before deletion"
   ```

4. Delete the PVC:

   ```sh
   kubectl delete pvc local-path-retain-pvc
   ```

5. Verify backing directory and data remain intact on the host filesystem:

   ```sh
   test -f "$RETAIN_HOST_PATH/retain.txt" && echo "Data preserved under Retain reclaim policy"
   rm -rf "$RETAIN_HOST_PATH"
   kubectl delete pv "$RETAIN_PV" --ignore-not-found
   kubectl delete sc local-path-retain
   ```

---

## 4. CoreDNS Internal Resolution Under Denied Egress

CoreDNS provides cluster-internal name resolution without requiring access to upstream internet
DNS servers.

### 4.1 Resolution Check

Deploy a DNS probe pod using the offline pinned `busybox` image:

```sh
cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: dns-test-pod
  namespace: default
spec:
  restartPolicy: Never
  containers:
    - name: probe
      image: rancher/mirrored-library-busybox:1.37.0@sha256:498a000f370d8c37927118ed80afe8a86594149023036a7124611005648257d1
      command: ["sh", "-c", "sleep 3600"]
EOF
kubectl wait --for=condition=Ready pod/dns-test-pod --timeout=60s
```

Execute internal DNS queries:

```sh
kubectl exec pod/dns-test-pod -- nslookup kubernetes.default.svc.cluster.local
```

### 4.2 Expected Outcome

- `kubernetes.default.svc.cluster.local` resolves immediately to `10.43.0.1`:

  ```sh
  RES=$(kubectl exec pod/dns-test-pod -- nslookup kubernetes.default.svc.cluster.local)
  echo "$RES" | grep "10.43.0.1" || { echo "ERROR: Failed to resolve internal service"; exit 1; }
  ```

- Upstream public queries (e.g. `google.com`) fail or time out gracefully due to denied egress,
  without causing CoreDNS service crashes or lookup hanging:

  ```sh
  DNS_OUT=$(kubectl exec pod/dns-test-pod -- nslookup google.com 2>&1 || true)
  if echo "$DNS_OUT" | grep -iE 'connection timed out|no servers could be reached|can.t resolve|server can.t find'; then
    echo "External DNS blocked as expected"
  else
    echo "ERROR: Unexpected external DNS response or probe failure: $DNS_OUT" >&2
    exit 1
  fi
  ```

Clean up probe pod:

```sh
kubectl delete pod dns-test-pod
```

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

2. Record normalized resource snapshots (including spec, data, UID, and creation timestamp, excluding server-generated runtime fields) of all Portainer-owned objects:

   ```sh
   snapshot_portainer_resources() {
     local target_file="$1"
     local raw_json
     raw_json=$(kubectl get deployment,svc,sa,pvc,configmap,secret -n portainer -o json) || {
       echo "ERROR: Failed to retrieve Portainer resources via kubectl" >&2
       return 1
     }

     local normalized
     normalized=$(echo "$raw_json" | jq -e '
       .items |= sort_by(.kind, .metadata.name) |
       .items[] | {
         kind: .kind,
         name: .metadata.name,
         uid: .metadata.uid,
         creationTimestamp: .metadata.creationTimestamp,
         spec: .spec,
         data: .data
       }
     ') || {
       echo "ERROR: Failed to normalize Portainer resources with jq" >&2
       return 1
     }

     if [ -z "$normalized" ]; then
       echo "ERROR: Snapshot output is empty" >&2
       return 1
     fi

     echo "$normalized" > "$target_file"
   }

   snapshot_portainer_resources /tmp/portainer-resources-before.json
   ```

3. Trigger a second bootstrap execution (e.g., node restart, reconciler re-evaluation, or
   re-applying the bootstrap manifest) and wait for reconciliation:

   ```sh
   kubectl apply -f /var/lib/kubesolo/manifests/portainer.yaml
   kubectl rollout status deployment/portainer -n portainer --timeout=90s
   kubectl wait --for=condition=Ready pod -l app.kubernetes.io/name=portainer -n portainer --timeout=90s
   ```

4. Capture resources after second bootstrap and assert zero mutation across specs, data, and identities:

   ```sh
   snapshot_portainer_resources /tmp/portainer-resources-after.json

   diff -u /tmp/portainer-resources-before.json /tmp/portainer-resources-after.json || {
     echo "ERROR: Portainer resources mutated across bootstrap executions" >&2
     exit 1
   }
   echo "Zero Portainer resource mutation verified across idempotent bootstrap"
   ```

5. Verify that:
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

Execute an HTTPS request presenting the authorized client certificate and explicitly asserting HTTP 200 OK:

```sh
HTTP_STATUS=$(curl --cacert /var/lib/kubesolo/pki/d2k-ca.crt \
     --cert /var/lib/kubesolo/pki/d2k-client.crt \
     --key /var/lib/kubesolo/pki/d2k-client.key \
     --silent --output /tmp/d2k-version.json \
     --write-out "%{http_code}" \
     https://127.0.0.1:9443/version)

if [ "$HTTP_STATUS" != "200" ]; then
  echo "ERROR: Expected HTTP 200 OK from D2K, received: $HTTP_STATUS" >&2
  exit 1
fi
echo "D2K authenticated request succeeded with HTTP 200 OK"
```

**Expected Result**: TLS handshake succeeds; endpoint returns `HTTP 200 OK` with daemon version metadata.

### 6.3 Negative Verification: Missing Client Certificate

Execute an HTTPS request omitting client certificates:

```sh
HTTP_OUTPUT=$(curl -v --cacert /var/lib/kubesolo/pki/d2k-ca.crt \
     https://127.0.0.1:9443/version 2>&1) || CURL_EXIT=$?

# Expect curl exit code 35 (SSL connect error) or 56 (failure receiving network data / SSL alert)
if [ "$CURL_EXIT" != "35" ] && [ "$CURL_EXIT" != "56" ]; then
  echo "ERROR: Expected TLS failure exit code 35 or 56, got $CURL_EXIT"
  exit 1
fi

echo "$HTTP_OUTPUT" | grep -iE 'bad certificate|certificate required|tls alert|peer certificate' || {
  echo "ERROR: Expected TLS certificate alert in curl output"
  exit 1
}
echo "Unauthenticated request rejected at TLS layer"
```

**Expected Result**: Request is rejected during TLS negotiation with a client certificate required
alert (`bad certificate` or `certificate required`), yielding curl exit code 35 or 56. The connection is aborted without processing the request body.

### 6.4 Negative Verification: Foreign/Untrusted Certificate

Generate a temporary self-signed client certificate using an untrusted foreign CA:

```sh
openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
  -keyout /tmp/foreign.key -out /tmp/foreign.crt -subj "/CN=unauthorized-client"

HTTP_OUTPUT=$(curl -v --cacert /var/lib/kubesolo/pki/d2k-ca.crt \
     --cert /tmp/foreign.crt \
     --key /tmp/foreign.key \
     https://127.0.0.1:9443/version 2>&1) || CURL_EXIT=$?

# Expect curl exit code 35 (SSL connect error) or 56 (failure receiving network data / SSL alert)
if [ "$CURL_EXIT" != "35" ] && [ "$CURL_EXIT" != "56" ]; then
  echo "ERROR: Expected TLS failure exit code 35 or 56, got $CURL_EXIT"
  exit 1
fi

echo "$HTTP_OUTPUT" | grep -iE 'unknown ca|bad certificate|tls alert' || {
  echo "ERROR: Expected untrusted CA TLS alert in curl output"
  exit 1
}
echo "Untrusted client certificate rejected at TLS layer"
```

**Expected Result**: TLS handshake fails with `SSL alert number 48 (unknown CA / bad certificate)`
or connection reset (exit code 35 or 56). The request is dropped immediately.

### 6.5 Endpoint Readiness Gated on Live Container Health Probe

D2K service endpoint readiness is strictly gated on the live container health probe (`/healthz` on port 9443
configured with `periodSeconds: 5`, `failureThreshold: 3`). The endpoint must not be marked `Ready` merely because the pod controller reconciled or the pod scheduled; it transitions to `Ready` only after the live container process begins responding affirmatively to health checks over the authenticated transport.

1. **Pre-Ready Observation (Initialization State)**:
   During pod initialization before the `/healthz` probe succeeds, observe that the Service endpoint has 0 ready addresses:

   ```sh
   # Observe that .subsets[*].addresses is empty or IPs are in notReadyAddresses
   READY_IPS=$(kubectl get endpoints -n portainer d2k -o jsonpath='{.subsets[*].addresses[*].ip}' || true)
   if [ -n "$READY_IPS" ]; then
     echo "WARNING: Pre-ready endpoint already has ready addresses: $READY_IPS"
   fi
   NOT_READY_IPS=$(kubectl get endpoints -n portainer d2k -o jsonpath='{.subsets[*].notReadyAddresses[*].ip}' || true)
   echo "Pre-ready addresses pending probe: ${NOT_READY_IPS:-none}"
   ```

2. **Health Probe Execution**:
   The kubelet periodically executes the HTTP GET probe against `/healthz`. Once the probe returns `200 OK`, the endpoint controller publishes the pod IP into `addresses`.

3. **Post-Ready Observation (Healthy State)**:
   Verify endpoint readiness transition after health probe passes:

   ```sh
   kubectl wait --for=condition=Ready pod -l app.kubernetes.io/name=d2k -n portainer --timeout=60s
   READY_IP=$(kubectl get endpoints -n portainer d2k -o jsonpath='{.subsets[0].addresses[0].ip}')
   test -n "$READY_IP" && echo "D2K endpoint published ready IP: $READY_IP"
   ```

---

## 7. Qualification Receipt Validation

Live qualification evidence for Criterion 4 must be captured in a candidate-bound receipt adhering to `schema_version: 1` (`criterion-04-addons-and-egress.json` under `docs/release/receipts/`). Until authentic live Linux qualification is executed on disposable infrastructure, live qualification remains pending and not established.

To evaluate Criterion 4 against candidate inventory and verify fail-closed behavior:

```sh
cargo test -p rubix-dev --test suite -- test_criterion_4_fails_closed_when_receipt_absent
cargo test -p rubix-dev --test suite -- test_criterion_4_receipt_validation_and_tamper_rejection
cargo run -p rubix-dev --bin rubix-qualification
```

The receipt validator verifies:
1. Candidate source revision and artifact digests match `cell-inventory.json` and `SHA256SUMS`.
2. All qualification commands exited with code 0.
3. All qualification assertions passed.
4. Cleanup status is `complete` with 0 leaked containers and 0 leaked images.
5. Canonical SHA-256 payload integrity hash matches the receipt.
