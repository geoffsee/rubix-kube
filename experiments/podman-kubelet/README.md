# Podman-backed kubelet experiment (macOS)

Live evidence that `rubix-kube` runs real containers from `kubectl` on a Mac,
using the in-process kubelet (`crates/rubix-kubelet`) through its CRI-shaped
`RuntimeProvider`, implemented over podman by the container-engine adapter
(`src/engine.rs`) and the podman engine (`src/podman.rs`). CRI sandboxes are podman
pods, so each Pod has its own network namespace and IP. This is disposable evidence
for a development slice. It deliberately ignores the acceptance-matrix order and the
retained official kubelet and containerd boundary; it does not qualify anything in
the compatibility contract.

## Fixture

| File | Purpose |
| --- | --- |
| `main.go`, `go.mod` | `rubix-hello`: prints `rubix-ok`, stays alive for N seconds (default 60), exits 0; `fail` prints `rubix-fail` to stderr and exits 3. Static Linux arm64 binary built with Go. |
| `Containerfile` | `FROM scratch`, copies the binary, `USER 65534:65534`, `ENTRYPOINT ["/rubix-hello"]`. |
| `policy.json` | Signature policy for the local image store. |
| `hello.yaml` | The one-shot pod from the task, `restartPolicy: Never`, kept alive 600 s so the process is observable. |
| `short.yaml` | `hello-short` (exits 0 after 3 s) and `hello-fail` (exits 3), both `Never`. |
| `crash.yaml` | `crash`: default `restartPolicy: Always` with the `fail` command, to show restarts and `CrashLoopBackOff`. |

The image was built with [oci-builder](https://github.com/geoffsee/oci-builder)
(`buildah-ffi`), which boots a small Linux guest through Virtualization.framework on
macOS. `FROM scratch` plus `COPY` needs no `RUN`, so `chroot` isolation and the `vfs`
driver are enough. The guest shares the directory that holds `--signature-policy` at
`/mnt/policy`, which is how the docker-archive export lands back on the Mac.

```sh
# host prerequisites: go, /opt/podman/bin/podman with a running machine, oci-builder CLI
W=$(pwd)/experiments/podman-kubelet
CGO_ENABLED=0 GOOS=linux GOARCH=arm64 go build -C "$W" -trimpath -ldflags="-s -w" -o rubix-hello .
oci-builder --root "$W/graph" --runroot "$W/run" --storage-driver vfs --signature-policy "$W/policy.json" \
  build -f "$W/Containerfile" --context "$W" -t localhost/rubix-hello:latest --pull never --isolation chroot
oci-builder --root "$W/graph" --runroot "$W/run" --storage-driver vfs --signature-policy "$W/policy.json" \
  push localhost/rubix-hello:latest docker-archive:/mnt/policy/rubix-hello.tar:localhost/rubix-hello:latest
podman load -i "$W/rubix-hello.tar"
podman run --rm localhost/rubix-hello:latest 1      # prints rubix-ok, exits 0
```

The build graph, archive and binary outputs are not committed.

## Running the proof

```sh
cargo build --locked -p rubix-kube
export STATE=$(mktemp -d)/rubix
KUBESOLO_PATH=$STATE target/debug/rubix-kube &      # logs JSONL to stderr
kc() { kubectl --kubeconfig "$STATE/pki/admin.kubeconfig" "$@"; }
kc get nodes
kc create -f experiments/podman-kubelet/hello.yaml   # no --validate=false needed
kc get pod hello -o yaml
podman pod ps --filter label=io.rubix.managed-by=rubix-kubelet
podman ps --filter label=io.rubix.managed-by=rubix-kubelet
kc logs hello
kc delete pod hello                                  # graceful; returns when the kubelet has removed it
kc create -f experiments/podman-kubelet/hello.yaml
kc create -f experiments/podman-kubelet/short.yaml -f experiments/podman-kubelet/crash.yaml
kc get pods -o custom-columns='NAME:.metadata.name,PHASE:.status.phase,RESTARTS:.status.containerStatuses[0].restartCount,REASON:.status.containerStatuses[0].state.*.reason,IP:.status.podIP'
kc logs crash; kc logs --previous crash
kc delete pods --all
kill -TERM %1
```

## Evidence (2026-10-05, macOS arm64, podman 6.0.2 libkrun machine, kubectl v1.37.0)

Transcript of the sequence above against a fresh state directory. Each Pod is a
`k8s_POD_…` podman pod whose infra container holds the network namespace, so
`podIP` is the pod's own address on the engine network (`10.88.0.x`). The pod
`containerID` is the podman container id and the PID is the process inside the
podman VM. Phases follow the processes: the 600 s container is `Running`, the 3 s
container ends `Succeeded`, the `fail` container ends `Failed` with exit code 3, and
the `Always` pod restarts with `CrashLoopBackOff` back-off (10 s, then 20 s), a
growing `restartCount`, and its previous attempt kept for `kubectl logs --previous`.
`kubectl delete` sets `deletionTimestamp`; the kubelet issues `StopContainer` with the
grace period, records the final status, removes the sandbox and deletes the object;
kubectl observes the `DELETED` event through the watch stream and returns in seconds.

```text
### STATE=$SCRATCH/state13
$ kubectl get nodes
NAME         AGE
rubix-node   0s
$ kubectl get node -o jsonpath=... (Ready, runtime)
rubix-node  Ready=True  containerRuntimeVersion=podman://6.0.2
$ kubectl create -f hello.yaml   (no --validate=false)
pod/hello created
$ kubectl get pod hello -o yaml | status
status:
  conditions:
  - lastProbeTime: null
    lastTransitionTime: "2026-10-05T11:45:54Z"
    status: "True"
    type: PodReadyToStartContainers
  - lastProbeTime: null
    lastTransitionTime: "2026-10-05T11:45:54Z"
    status: "True"
    type: Initialized
  - lastProbeTime: null
    lastTransitionTime: "2026-10-05T11:45:54Z"
    status: "True"
    type: Ready
  - lastProbeTime: null
    lastTransitionTime: "2026-10-05T11:45:54Z"
    status: "True"
    type: ContainersReady
  - lastProbeTime: null
    lastTransitionTime: "2026-10-05T11:45:54Z"
    status: "True"
    type: PodScheduled
  containerStatuses:
  - containerID: podman://3378a94507b54532c325dd52dc7e6eec8c19b252075be46985fe46c857711ad7
    cpuset: 0-15
    exclusiveCPU: false
    image: localhost/rubix-hello:latest
    imageID: sha256:e24752d76ab2a99afca951281914c6f4c3d8d8fa59ff9c4ccc56f4f6f926b9a4
    name: hello
    ready: true
    restartCount: 0
    started: true
    state:
      running:
        startedAt: "2026-10-05T11:45:53Z"
  hostIP: 127.0.0.1
  phase: Running
  podIP: 10.88.0.65
  podIPs:
  - ip: 10.88.0.65
  qosClass: BestEffort
  startTime: "2026-10-05T11:45:54Z"
$ podman pod ps --filter label=io.rubix.managed-by=rubix-kubelet
POD ID        NAME                                       STATUS      # OF CONTAINERS
db29d9d2d112  k8s_POD_hello_default_uid-pods-hello-18_0  Running     2
$ podman ps --filter label=io.rubix.managed-by=rubix-kubelet   (infra + workload container share the pod)
CONTAINER ID  NAMES                                        STATE       PID         PODNAME
b233e291ed61  db29d9d2d112-infra                           running     195811      k8s_POD_hello_default_uid-pods-hello-18_0
3378a94507b5  k8s_hello_hello_default_uid-pods-hello-18_0  running     195860      k8s_POD_hello_default_uid-pods-hello-18_0
$ podman inspect (labels, pid, pod)
id=3378a94507b54532c325dd52dc7e6eec8c19b252075be46985fe46c857711ad7 pid=195860 status=running pod=db29d9d2d1125957918c96e0953e8542984e2e2ba31dd1a0d1664babdfcc24a0 labels={"io.buildah.version":"1.45.1","io.kubernetes.container.name":"hello","io.kubernetes.container.restartCount":"0","io.kubernetes.pod.name":"hello","io.kubernetes.pod.namespace":"default","io.kubernetes.pod.uid":"uid-pods-hello-18","io.rubix.managed-by":"rubix-kubelet"}
$ podman inspect infra (pod network namespace holder)
infra=b233e291ed619f21a1389dbe94e795d53a2c4d24e35c4e264cf1cae308c936fa ip=10.88.0.65 10.88.0.65
$ kubectl logs hello
rubix-ok
$ kubectl logs --timestamps hello
2026-10-05T07:45:53.859338000-04:00 rubix-ok
$ kubectl delete pod hello   (graceful: StopContainer with grace, final status, sandbox removed, kubectl waits through the watch)
pod "hello" deleted from default namespace
delete_exit=0 elapsed=3s
$ kubectl get pod hello
Error from server (NotFound): resource not found: pods/default/hello
$ podman pod ps / ps -a (managed)
container 3378a94507b5 is gone
$ kubectl create -f hello.yaml   (second create)
pod/hello created
Running  podman://89583d0442a8341704ad1a9ade9cfaa3d0913dfde90a9db42e402646befe189e  podIP=10.88.0.66
$ kubectl create -f short.yaml -f crash.yaml
pod/hello-short created
pod/hello-fail created
pod/crash created
$ kubectl get pods -o custom-columns=...  (t+12s)
NAME          PHASE       READY   RESTARTS   REASON             LAST-EXIT   IP
crash         Running     false   1          CrashLoopBackOff   3           10.88.0.67
hello         Running     true    0          <none>             <none>      10.88.0.66
hello-fail    Failed      false   0          Error              <none>      10.88.0.68
hello-short   Succeeded   false   0          Completed          <none>      10.88.0.69
$ kubectl logs crash
rubix-ok
rubix-fail
$ kubectl logs --previous crash
rubix-ok
rubix-fail
$ kubectl get pod crash -o jsonpath   (t+24s)
Running  restarts=2  {"waiting":{"message":"back-off 20s restarting failed container=hello pod=crash_default(uid-pods-crash-33)","reason":"CrashLoopBackOff"}}
$ kubectl get pod crash -o jsonpath (conditions)
PodReadyToStartContainers=True  2026-10-05T11:46:10Z
Initialized=True  2026-10-05T11:46:10Z
Ready=False ContainersNotReady 2026-10-05T11:46:10Z
ContainersReady=False ContainersNotReady 2026-10-05T11:46:10Z
PodScheduled=True  2026-10-05T11:46:10Z
$ podman ps -a --filter label=io.kubernetes.pod.name=crash   (current and previous attempt kept)
NAMES                                        STATE       EXIT CODE
9bbf43abd7e7-infra                           running     0
k8s_hello_crash_default_uid-pods-crash-33_1  exited      3
k8s_hello_crash_default_uid-pods-crash-33_2  exited      3
$ kubectl create configmap demo-cm --from-literal=k=v; kubectl get cm; kubectl get ns
configmap/demo-cm created
NAME      AGE
demo-cm   <unknown>
NAME                 AGE
local-path-storage   41s
$ kubectl delete pods --all
pod "crash" deleted from default namespace
pod "hello" deleted from default namespace
pod "hello-fail" deleted from default namespace
pod "hello-short" deleted from default namespace
elapsed=3s
$ kubectl get pods; podman pod ps / ps -a (managed)
No resources found in default namespace.
(none)
### stopping rubix-kube pid 61945 (SIGTERM)
node exited; 6443 free
### kubelet events from node log
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":1,"synced":1,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":1,"synced":1,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":3,"synced":4,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":0,"synced":2,"failed":0,"orphans_stopped":2}
```

## What this does and does not establish

- Established on this host: Ready node; pods bound by the kubelet loop; one podman
  pod per Pod with a real pod IP; one real podman container per attempt; status
  written only from CRI state (phases, conditions with `lastTransitionTime`,
  `restartCount`, `CrashLoopBackOff` with `lastState`); logs read from the process
  in time order with `--timestamps` and `--previous`; graceful deletion driven by
  `deletionTimestamp` with the kubelet performing the final delete; `kubectl delete`
  waiting through the watch stream; second create works; `kubectl get ns` and
  `kubectl create configmap` still work; `cargo test --locked -p rubix-apiserver
  --test https_gateway` passes.
- Implemented in [E34.02]: volume mounts into containers (secret, configMap,
  projected, emptyDir, hostPath via CRI Mounts), container port mappings, Linux
  container resource requests and limits, sequential init container lifecycle and
  status reporting, HTTP and TCP readiness/liveness probes, preStop lifecycle hooks,
  apiserver Table responses for `as=Table`, `kubectl logs --follow` chunked streaming,
  and bootstrap creation of `default`, `kube-system`, and `kube-node-lease` namespaces.
- Not implemented: OpenAPI schemas, CRI log files (engine logs are read instead).
- Unqualified: the official kubelet/containerd boundary, Linux hosts, anything in
  the compatibility contract. Historical captures elsewhere are unaffected.
