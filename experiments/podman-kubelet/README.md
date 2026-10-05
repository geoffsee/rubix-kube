# Podman-backed kubelet experiment (macOS)

Live evidence that `rubix-kube` runs real containers from `kubectl` on a Mac,
using the in-process kubelet (`crates/rubix-kubelet`) with the container-engine
adapter (`src/engine.rs`, the dockershim shape) over the podman engine
(`src/podman.rs`). This is disposable evidence for a development slice. It
deliberately ignores the acceptance-matrix order and the retained official
kubelet and containerd boundary; it does not qualify anything in the
compatibility contract.

## Fixture

| File | Purpose |
| --- | --- |
| `main.go`, `go.mod` | `rubix-hello`: prints `rubix-ok`, stays alive for N seconds (default 60), exits 0; `fail` prints `rubix-fail` to stderr and exits 3. Static Linux arm64 binary built with Go. |
| `Containerfile` | `FROM scratch`, copies the binary, `ENTRYPOINT ["/rubix-hello"]`. |
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
podman ps --filter label=io.rubix.managed-by=rubix-kubelet
kc logs hello
kc delete pod hello                                  # graceful; returns when the kubelet has removed it
kc create -f experiments/podman-kubelet/hello.yaml
kc create -f experiments/podman-kubelet/short.yaml -f experiments/podman-kubelet/crash.yaml
kc get pods -o custom-columns='NAME:.metadata.name,PHASE:.status.phase,RESTARTS:.status.containerStatuses[0].restartCount,REASON:.status.containerStatuses[0].state.*.reason'
kc logs crash
kc delete pods --all
kill -TERM %1
```

## Evidence (2026-10-04, macOS arm64, podman 6.0.2 libkrun machine, kubectl v1.37.0)

Transcript of the sequence above against a fresh state directory. The pod
`containerID` is the podman container id, the PID is the process inside the podman
VM, containers carry the standard `io.kubernetes.*` labels, and the phases follow
the processes: the 600 s container is `Running`, the 3 s container ends `Succeeded`,
the `fail` container ends `Failed` with exit code 3, and the `Always` pod restarts
with `CrashLoopBackOff` back-off (10 s, then 20 s) and a growing `restartCount`.
`kubectl delete` sets `deletionTimestamp`, the kubelet sends `TERM`, records the
final status, removes the container and deletes the object; kubectl observes the
`DELETED` event through the watch stream and returns in a few seconds.

```text
### STATE=$SCRATCH/state11
$ kubectl get nodes
NAME         AGE
rubix-node   1s
$ kubectl get node -o jsonpath=... (Ready, runtime)
rubix-node  Ready=True  containerRuntimeVersion=podman://6.0.2
$ kubectl create -f hello.yaml   (no --validate=false)
pod/hello created
$ kubectl get pod hello -o yaml | status
status:
  conditions:
  - lastProbeTime: null
    lastTransitionTime: "2026-10-05T03:41:04Z"
    status: "True"
    type: PodReadyToStartContainers
  - lastProbeTime: null
    lastTransitionTime: "2026-10-05T03:41:04Z"
    status: "True"
    type: Initialized
  - lastProbeTime: null
    lastTransitionTime: "2026-10-05T03:41:04Z"
    status: "True"
    type: Ready
  - lastProbeTime: null
    lastTransitionTime: "2026-10-05T03:41:04Z"
    status: "True"
    type: ContainersReady
  - lastProbeTime: null
    lastTransitionTime: "2026-10-05T03:41:04Z"
    status: "True"
    type: PodScheduled
  containerStatuses:
  - containerID: podman://658f1c30f23756f9aa16cd73d036f6f3a81d5de9da27333dbdc19b7115f8a234
    image: localhost/rubix-hello:latest
    imageID: sha256:e24752d76ab2a99afca951281914c6f4c3d8d8fa59ff9c4ccc56f4f6f926b9a4
    name: hello
    ready: true
    restartCount: 0
    started: true
    state:
      running:
        startedAt: "2026-10-05T03:41:04Z"
  hostIP: 127.0.0.1
  phase: Running
  podIP: 127.0.0.1
  qosClass: BestEffort
  startTime: "2026-10-05T03:41:04Z"
$ podman ps --filter label=io.rubix.managed-by=rubix-kubelet
CONTAINER ID  NAMES                                        STATE       PID         IMAGE
658f1c30f237  k8s_hello_hello_default_uid-pods-hello-18_0  running     125021      localhost/rubix-hello:latest
$ podman inspect (labels, pid)
id=658f1c30f23756f9aa16cd73d036f6f3a81d5de9da27333dbdc19b7115f8a234 pid=125021 status=running labels={"io.buildah.version":"1.45.1","io.kubernetes.container.name":"hello","io.kubernetes.container.restartCount":"0","io.kubernetes.pod.name":"hello","io.kubernetes.pod.namespace":"default","io.kubernetes.pod.uid":"uid-pods-hello-18","io.rubix.managed-by":"rubix-kubelet"}
$ kubectl logs hello
rubix-ok
$ kubectl logs --timestamps hello
2026-10-04T23:41:04.579339000-04:00 rubix-ok
$ kubectl delete pod hello   (graceful: TERM, final status, kubelet deletes; kubectl waits through the watch)
pod "hello" deleted from default namespace
delete_exit=0 elapsed=3s
$ kubectl get pod hello
Error from server (NotFound): resource not found: pods/default/hello
$ podman ps -a (managed)
container 658f1c30f237 is gone
$ kubectl create -f hello.yaml   (second create)
pod/hello created
Running  podman://a20812c8c27a837a3bb48516456b26a3e7db8130836b0f6ce6aa6b40447cfa3e
$ kubectl create -f short.yaml -f crash.yaml
pod/hello-short created
pod/hello-fail created
pod/crash created
$ kubectl get pods -o custom-columns=...  (t+10s)
NAME          PHASE       READY   RESTARTS   REASON             LAST-EXIT
crash         Running     false   1          CrashLoopBackOff   3
hello         Running     true    0          <none>             <none>
hello-fail    Failed      false   0          Error              <none>
hello-short   Succeeded   false   0          Completed          <none>
$ kubectl logs crash   (last attempt, while in back-off)
rubix-ok
rubix-fail
$ kubectl get pod crash -o jsonpath   (t+24s)
Running  restarts=2  {"waiting":{"message":"back-off 20s restarting failed container=hello pod=crash_default(uid-pods-crash-33)","reason":"CrashLoopBackOff"}}
$ kubectl get pod crash -o jsonpath (conditions)
PodReadyToStartContainers=True  2026-10-05T03:41:16Z
Initialized=True  2026-10-05T03:41:16Z
Ready=False ContainersNotReady 2026-10-05T03:41:16Z
ContainersReady=False ContainersNotReady 2026-10-05T03:41:16Z
PodScheduled=True  2026-10-05T03:41:16Z
$ podman ps -a --filter label=io.kubernetes.pod.name=crash
NAMES                                        STATE       EXIT CODE
k8s_hello_crash_default_uid-pods-crash-33_2  exited      3
$ kubectl create configmap demo-cm --from-literal=k=v; kubectl get cm; kubectl get ns
configmap/demo-cm created
NAME      AGE
demo-cm   <unknown>
NAME                 AGE
local-path-storage   37s
$ kubectl delete pods --all
pod "crash" deleted from default namespace
pod "hello" deleted from default namespace
pod "hello-fail" deleted from default namespace
pod "hello-short" deleted from default namespace
elapsed=4s
$ kubectl get pods; podman ps -a (managed)
No resources found in default namespace.
(none)
### stopping rubix-kube pid 75617 (SIGTERM)
node exited; 6443 free
### kubelet events from node log
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":1,"synced":1,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":1,"synced":1,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":3,"synced":4,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":0,"synced":2,"failed":0,"orphans_stopped":2}
```

## What this does and does not establish

- Established on this host: Ready node, pods bound by the kubelet loop, one real
  podman container per pod container, status written only from observed engine
  state (phases, conditions with `lastTransitionTime`, `restartCount`,
  `CrashLoopBackOff` with `lastState`), logs read from the process in time order
  with `--timestamps`, graceful deletion driven by `deletionTimestamp` with the
  kubelet performing the final delete, `kubectl delete` waiting through the watch
  stream, second create works, `kubectl get ns` and `kubectl create configmap`
  still work, and `cargo test --locked -p rubix-apiserver --test https_gateway`
  passes.
- Not implemented: pod sandboxes (containers of one pod do not share namespaces,
  `podIP` is the node IP), probes, volumes, exec, ports, pod-to-pod networking,
  kube-proxy, CoreDNS, init containers, preStop hooks, `kubectl logs --follow`
  and `--previous`, OpenAPI schemas. A restarted container's earlier attempts are
  removed, so only the latest attempt's log is kept. `kubectl get pods` prints
  only NAME and AGE because the server does not return Table responses. The
  `default`, `kube-system` and `kube-node-lease` namespaces are not created by
  startup (pre-existing).
- Unqualified: the official kubelet/containerd boundary, Linux hosts, anything in
  the compatibility contract. Historical captures elsewhere are unaffected.
