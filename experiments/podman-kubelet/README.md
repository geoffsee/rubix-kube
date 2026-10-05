# Podman-backed kubelet experiment (macOS)

Live evidence that `rubix-kube` runs a real container from `kubectl` on a Mac,
using the in-process kubelet (`crates/rubix-kubelet`) with the OCI runtime adapter
(`src/oci.rs`) over the podman engine (`src/podman.rs`). This is disposable evidence for a development slice. It deliberately
ignores the acceptance-matrix order and the retained official kubelet and
containerd boundary; it does not qualify anything in the compatibility contract.

## Fixture

| File | Purpose |
| --- | --- |
| `main.go`, `go.mod` | `rubix-hello`: prints `rubix-ok`, stays alive for N seconds (default 60), exits 0; `fail` prints `rubix-fail` to stderr and exits 3. Static Linux arm64 binary built with Go. |
| `Containerfile` | `FROM scratch`, copies the binary, `ENTRYPOINT ["/rubix-hello"]`. |
| `policy.json` | Signature policy for the local image store. |
| `hello.yaml` | The one-shot pod from the task, kept alive 600 s so the process is observable. |
| `short.yaml` | `hello-short` (exits 0 after 3 s) and `hello-fail` (exits 3). |

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

The build, graph, archive and binary outputs are not committed.

## Running the proof

```sh
cargo build --locked -p rubix-kube
export STATE=$(mktemp -d)/rubix
KUBESOLO_PATH=$STATE target/debug/rubix-kube &      # logs JSONL to stderr
kc() { kubectl --kubeconfig "$STATE/pki/admin.kubeconfig" "$@"; }
kc get nodes
kc create -f experiments/podman-kubelet/hello.yaml   # no --validate=false needed
kc get pod hello -o yaml
podman ps --filter label=io.rubix.managed=true
kc logs hello
kc delete pod hello
podman ps -a --filter label=io.rubix.managed=true
kc create -f experiments/podman-kubelet/hello.yaml
kc create -f experiments/podman-kubelet/short.yaml
kc get pods -o custom-columns='NAME:.metadata.name,PHASE:.status.phase,EXIT:.status.containerStatuses[0].state.terminated.exitCode'
kc delete pods --all
kill -TERM %1
```

## Evidence (2026-10-04, macOS arm64, podman 6.0.2 libkrun machine, kubectl v1.37.0)

Transcript of the sequence above against a fresh state directory. The pod
`containerID` is the podman container id, the PID is the process inside the podman
VM, and the phases follow the processes: the 600 s container is `Running`, the 3 s
container ends `Succeeded`, the `fail` container ends `Failed` with exit code 3.

```text
### STATE=$SCRATCH/state2
$ kubectl get nodes
NAME         AGE
rubix-node   5d1h
$ kubectl get node -o jsonpath=... (Ready, runtime)
rubix-node  Ready=True  containerRuntimeVersion=podman://6.0.2
$ kubectl create -f hello.yaml   (no --validate=false)
pod/hello created
$ kubectl get pod hello -o jsonpath (phase, nodeName, containerID)
Running  nodeName=rubix-node  containerID=podman://58f1b8273a7fc3d4224719f9badca92d9bd7e9688be411df62b698e2d5870f76
$ kubectl get pod hello -o yaml | status
status:
  conditions:
  - message: pod assigned to node
    reason: PodScheduled
    status: "True"
    type: PodScheduled
  - message: all init containers completed
    reason: PodInitialized
    status: "True"
    type: Initialized
  - message: pod is ready
    reason: PodReady
    status: "True"
    type: ContainersReady
  - message: pod is ready
    reason: PodReady
    status: "True"
    type: Ready
  containerStatuses:
  - containerID: podman://58f1b8273a7fc3d4224719f9badca92d9bd7e9688be411df62b698e2d5870f76
    image: localhost/rubix-hello:latest
    imageID: e24752d76ab2a99afca951281914c6f4c3d8d8fa59ff9c4ccc56f4f6f926b9a4
    name: hello
    ready: true
    restartCount: 0
    started: true
    state:
      running:
        startedAt: "2026-10-05T01:21:31Z"
  hostIP: 127.0.0.1
  phase: Running
  podIP: 127.0.0.1
  qosClass: BestEffort
  startTime: "2026-10-05T01:21:30Z"
$ podman ps --filter label=io.rubix.managed=true
CONTAINER ID  NAMES                                   STATE       PID         IMAGE
58f1b8273a7f  rubix_default_hello_hello_ods-hello-24  running     107519      localhost/rubix-hello:latest
$ podman inspect (pid inside the podman VM)
id=58f1b8273a7fc3d4224719f9badca92d9bd7e9688be411df62b698e2d5870f76 pid=107519 status=running
$ kubectl logs hello
rubix-ok
$ kubectl delete pod hello
pod "hello" deleted from default namespace
$ podman ps -a --filter label=io.rubix.managed=true   (after delete)
container 58f1b8273a7f is gone
$ kubectl create -f hello.yaml   (second create)
pod/hello created
Running  containerID=podman://86a96669d53732de37333e3b2a73d9b9f26cdc4e7b4b794288ceb9ecad54ccd1
rubix_default_hello_hello_ods-hello-29 running pid=107755
rubix-ok
$ kubectl create -f short.yaml   (3s one-shot and exit-3 pods)
pod/hello-short created
pod/hello-fail created
$ kubectl get pods -o custom-columns=...
NAME          PHASE       EXIT     REASON      READY
hello         Running     <none>   <none>      True
hello-fail    Failed      3        Error       False
hello-short   Succeeded   0        Completed   False
$ kubectl logs hello-fail
rubix-ok
rubix-fail
$ kubectl create configmap demo-cm --from-literal=k=v; kubectl get cm; kubectl get ns
configmap/demo-cm created
NAME      AGE
demo-cm   <unknown>
NAME                 AGE
local-path-storage   5d1h
$ kubectl delete pods --all
pod "hello" deleted from default namespace
pod "hello-fail" deleted from default namespace
pod "hello-short" deleted from default namespace
$ podman ps -a --filter label=io.rubix.managed=true
(none)
### stopping rubix-kube pid 51660 (SIGTERM)
(the script checked for 10 s; the process exited shortly after, once the pre-existing optional
 coredns and local-path adapters hit their cleanup timeouts; final node log line:)
{"degraded":false,"event":"supervisor_finished","finished":true,"level":"info","schema":1}
### kubelet events from node log
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":1,"synced":1,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":0,"synced":0,"failed":0,"orphans_stopped":1}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":1,"synced":1,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":2,"synced":3,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":0,"synced":0,"failed":0,"orphans_stopped":3}
12
```

Kubelet lifecycle events from the node log for the same run:

```text
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":1,"synced":1,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":0,"synced":0,"failed":0,"orphans_stopped":1}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":1,"synced":1,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":2,"synced":3,"failed":0,"orphans_stopped":0}
{"schema":1,"level":"info","component":"kubelet","event":"kubelet_reconcile","bound":0,"synced":0,"failed":0,"orphans_stopped":3}
```

## What this does and does not establish

- Established on this host: Ready node, pod bound by the kubelet loop, real podman
  container per pod container, status written only from observed podman state,
  logs read from the process, delete stops and removes the container, second create
  works, `kubectl get ns` and `kubectl create configmap` still work, and
  `cargo test --locked -p rubix-apiserver --test https_gateway` passes.
- Not implemented: container restarts (`restartPolicy` other than `Never` is not
  honoured), probes, volumes, exec, ports, pod-to-pod networking, kube-proxy, CoreDNS,
  init containers, graceful deletion with `deletionTimestamp`, HTTP watch, OpenAPI
  schemas. `kubectl get pods` prints only NAME and AGE because the server does not
  return Table responses. The `default`, `kube-system` and `kube-node-lease` namespaces
  are not created by startup (pre-existing).
- Unqualified: the official kubelet/containerd boundary, Linux hosts, anything in
  the compatibility contract. Historical captures elsewhere are unaffected.
