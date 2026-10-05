//! Pod status must follow what the runtime actually observes: Pending while
//! nothing runs, Running while a process is alive or restarting, then Succeeded
//! or Failed from the exit code under the restart policy. Deleted pods are
//! terminated gracefully and removed by the kubelet.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use tempfile::TempDir;

use rubix_apiserver::{
    ApiserverConfig, ApiserverService, KubernetesApiClient, KubernetesStorage, PodLogOptions,
    PodLogReader,
};
use rubix_datastore::{DatastoreConfig, DatastoreEngine};
use rubix_kubelet::{
    ContainerRuntimeState, ContainerRuntimeStatus, KubeletConfigOptions, KubeletError,
    KubeletService, LogOptions, ManagedPodRef, PodRuntimeStatus, PodSignal, RuntimeProvider,
};
use rubix_pki::cluster::{ClusterPki, ClusterPkiConfig};

/// Scripted runtime: containers it "starts" sit in a chosen state until the test
/// moves them on, mirroring how an engine reports a long-lived or exited process.
#[derive(Debug, Default)]
struct ScriptedRuntime {
    containers: Mutex<BTreeMap<String, Vec<ContainerRuntimeStatus>>>,
    started: Mutex<Vec<String>>,
    stopped: Mutex<Vec<String>>,
    signals: Mutex<Vec<(String, PodSignal)>>,
}

impl ScriptedRuntime {
    fn set_state(&self, uid: &str, state: &ContainerRuntimeState) {
        let mut map = self.containers.lock().unwrap();
        for container in map.get_mut(uid).unwrap() {
            container.state = state.clone();
        }
    }

    fn started_count(&self) -> usize {
        self.started.lock().unwrap().len()
    }

    fn running(name: &str, uid: &str, attempt: u32, image: &str) -> ContainerRuntimeStatus {
        ContainerRuntimeStatus {
            name: name.to_string(),
            container_id: format!("scripted://{uid}-{name}-{attempt}"),
            image: image.to_string(),
            image_id: "sha256:image".to_string(),
            restart_count: attempt,
            state: ContainerRuntimeState::Running {
                started_at: "2026-10-04T12:00:00Z".to_string(),
            },
        }
    }
}

#[async_trait]
impl RuntimeProvider for ScriptedRuntime {
    fn provider_name(&self) -> &'static str {
        "scripted"
    }

    async fn run_pod(&self, pod: &Value) -> Result<String, KubeletError> {
        let uid = pod["metadata"]["uid"].as_str().unwrap().to_string();
        let containers = pod["spec"]["containers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                Self::running(
                    c["name"].as_str().unwrap(),
                    &uid,
                    0,
                    c["image"].as_str().unwrap(),
                )
            })
            .collect();
        self.containers
            .lock()
            .unwrap()
            .insert(uid.clone(), containers);
        self.started.lock().unwrap().push(uid.clone());
        Ok(uid)
    }

    async fn restart_container(
        &self,
        pod: &Value,
        container_name: &str,
        attempt: u32,
    ) -> Result<(), KubeletError> {
        let uid = pod["metadata"]["uid"].as_str().unwrap().to_string();
        let mut map = self.containers.lock().unwrap();
        let containers = map.get_mut(&uid).unwrap();
        let image = containers
            .iter()
            .find(|c| c.name == container_name)
            .map(|c| c.image.clone())
            .unwrap();
        containers.retain(|c| c.name != container_name);
        containers.push(Self::running(container_name, &uid, attempt, &image));
        self.started
            .lock()
            .unwrap()
            .push(format!("{uid}#{attempt}"));
        Ok(())
    }

    async fn signal_pod(&self, pod_id: &str, signal: PodSignal) -> Result<(), KubeletError> {
        self.signals
            .lock()
            .unwrap()
            .push((pod_id.to_string(), signal));
        // The scripted process honours TERM by exiting with 143.
        self.set_state(
            pod_id,
            &ContainerRuntimeState::Terminated {
                exit_code: 143,
                started_at: Some("2026-10-04T12:00:00Z".to_string()),
                finished_at: "2026-10-04T12:00:09Z".to_string(),
            },
        );
        Ok(())
    }

    async fn stop_pod(&self, pod_id: &str) -> Result<(), KubeletError> {
        self.containers.lock().unwrap().remove(pod_id);
        self.stopped.lock().unwrap().push(pod_id.to_string());
        Ok(())
    }

    async fn get_pod_status(&self, pod_id: &str) -> Result<String, KubeletError> {
        Ok(if self.containers.lock().unwrap().contains_key(pod_id) {
            "Running".to_string()
        } else {
            "Stopped".to_string()
        })
    }

    async fn inspect_pod(&self, pod: &Value) -> Result<Option<PodRuntimeStatus>, KubeletError> {
        let uid = pod["metadata"]["uid"].as_str().unwrap();
        let containers = self
            .containers
            .lock()
            .unwrap()
            .get(uid)
            .cloned()
            .unwrap_or_default();
        Ok(Some(PodRuntimeStatus { containers }))
    }

    async fn list_managed_pods(&self) -> Result<Vec<ManagedPodRef>, KubeletError> {
        Ok(self
            .containers
            .lock()
            .unwrap()
            .keys()
            .map(|uid| ManagedPodRef {
                pod_id: uid.clone(),
                namespace: "default".to_string(),
                name: uid.clone(),
                uid: uid.clone(),
            })
            .collect())
    }

    async fn read_container_logs(
        &self,
        pod_id: &str,
        container_name: &str,
        options: &LogOptions,
    ) -> Result<String, KubeletError> {
        Ok(format!(
            "rubix-ok {pod_id} {container_name} {:?} {}\n",
            options.tail_lines, options.timestamps
        ))
    }
}

fn setup(dir: &TempDir) -> (Arc<ApiserverService>, KubeletConfigOptions) {
    let node_ip: IpAddr = "192.0.2.1".parse().unwrap();
    let pki_dir = dir.path().join("pki");
    std::fs::create_dir_all(&pki_dir).unwrap();
    ClusterPki::new(ClusterPkiConfig::new(
        pki_dir.clone(),
        "test-node".to_string(),
        node_ip,
    ))
    .reconcile()
    .unwrap();
    let (engine, _) =
        DatastoreEngine::open(DatastoreConfig::new(dir.path().join("datastore"))).unwrap();
    let storage = KubernetesStorage::new(engine.client(), "/registry");
    let apiserver =
        ApiserverService::new(ApiserverConfig::default_for_pki(&pki_dir, node_ip), storage);
    let kubelet_dir = dir.path().join("kubelet");
    std::fs::create_dir_all(&kubelet_dir).unwrap();
    let options =
        KubeletConfigOptions::default_for_pki(&pki_dir, "test-node", "192.0.2.1", &kubelet_dir);
    (Arc::new(apiserver), options)
}

fn pod_with_policy(name: &str, policy: &str) -> Value {
    json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": name, "namespace": "default" },
        "spec": {
            "restartPolicy": policy,
            "containers": [{
                "name": "hello",
                "image": "localhost/rubix-hello:latest",
                "command": ["/rubix-hello"]
            }]
        }
    })
}

fn one_shot_pod(name: &str) -> Value {
    pod_with_policy(name, "Never")
}

fn phase(pod: &Value) -> &str {
    pod["status"]["phase"].as_str().unwrap_or("")
}

fn condition<'a>(pod: &'a Value, kind: &str) -> &'a Value {
    pod["status"]["conditions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["type"] == kind)
        .unwrap_or_else(|| panic!("condition {kind}"))
}

fn exited(code: i32) -> ContainerRuntimeState {
    ContainerRuntimeState::Terminated {
        exit_code: code,
        started_at: Some("2026-10-04T12:00:00Z".to_string()),
        finished_at: "2026-10-04T12:00:05Z".to_string(),
    }
}

struct Started {
    _dir: TempDir,
    kubelet: KubeletService,
    admin: KubernetesApiClient,
    runtime: Arc<ScriptedRuntime>,
}

/// Creates the given pods unbound and runs the first kubelet pass over them.
async fn start_pods(pods: Vec<Value>) -> Started {
    let dir = TempDir::new().unwrap();
    let (apiserver, options) = setup(&dir);
    apiserver.check_prerequisites().await.unwrap();
    apiserver.start().unwrap();
    let runtime = Arc::new(ScriptedRuntime::default());
    let kubelet = KubeletService::new(options, apiserver.clone(), runtime.clone());
    kubelet.start().await.unwrap();
    let admin = apiserver.admin_client();
    let count = pods.len();
    for pod in pods {
        admin.create_pod("default", pod).await.unwrap();
    }
    let report = kubelet.reconcile_once().await.unwrap();
    assert_eq!(
        (report.bound, report.synced, report.failed),
        (count, count, 0)
    );
    Started {
        _dir: dir,
        kubelet,
        admin,
        runtime,
    }
}

async fn uid_of(admin: &KubernetesApiClient, name: &str) -> String {
    admin.get_pod("default", name).await.unwrap()["metadata"]["uid"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn resource_version(admin: &KubernetesApiClient, name: &str) -> Value {
    admin.get_pod("default", name).await.unwrap()["metadata"]["resourceVersion"].clone()
}

#[tokio::test]
async fn unbound_pod_is_bound_started_and_reported_running() {
    let Started {
        kubelet,
        admin,
        runtime,
        ..
    } = start_pods(vec![one_shot_pod("ok")]).await;

    let ok = admin.get_pod("default", "ok").await.unwrap();
    assert_eq!(ok["spec"]["nodeName"], "test-node");
    assert_eq!(phase(&ok), "Running");
    let status = &ok["status"]["containerStatuses"][0];
    let uid = uid_of(&admin, "ok").await;
    assert_eq!(status["containerID"], format!("scripted://{uid}-hello-0"));
    assert_eq!(status["ready"], true);
    assert_eq!(status["restartCount"], 0);
    assert!(status["state"]["running"]["startedAt"].is_string());
    for kind in [
        "PodReadyToStartContainers",
        "Initialized",
        "Ready",
        "ContainersReady",
        "PodScheduled",
    ] {
        let c = condition(&ok, kind);
        assert_eq!(c["status"], "True", "{kind}");
        assert!(c["lastTransitionTime"].is_string(), "{kind}");
    }
    assert_eq!(runtime.started_count(), 1);

    // Logs are read through the kubelet for the stored pod object, with options.
    let options = PodLogOptions {
        tail_lines: Some(2),
        timestamps: true,
        ..PodLogOptions::default()
    };
    let text = kubelet
        .log_source()
        .read_pod_log(&ok, &options)
        .await
        .unwrap();
    assert_eq!(text, format!("rubix-ok {uid} hello Some(2) true\n"));

    // Nothing changed, so a second pass writes no status and starts nothing new.
    let before = resource_version(&admin, "ok").await;
    kubelet.reconcile_once().await.unwrap();
    assert_eq!(before, resource_version(&admin, "ok").await);
    assert_eq!(runtime.started_count(), 1);

    // A pod assigned elsewhere is ignored.
    let mut elsewhere = one_shot_pod("elsewhere");
    elsewhere["spec"]["nodeName"] = json!("other-node");
    admin.create_pod("default", elsewhere).await.unwrap();
    let report = kubelet.reconcile_once().await.unwrap();
    assert_eq!(report.bound, 0);
    assert_eq!(runtime.started_count(), 1);
    let elsewhere = admin.get_pod("default", "elsewhere").await.unwrap();
    assert!(elsewhere.get("status").is_none());
}

#[tokio::test]
async fn never_policy_pods_become_terminal_and_keep_transition_times() {
    let Started {
        kubelet,
        admin,
        runtime,
        ..
    } = start_pods(vec![one_shot_pod("ok"), one_shot_pod("bad")]).await;
    let uid = uid_of(&admin, "ok").await;
    let bad_uid = uid_of(&admin, "bad").await;
    let ready_before = condition(&admin.get_pod("default", "ok").await.unwrap(), "Ready").clone();
    let scheduled_before = condition(
        &admin.get_pod("default", "ok").await.unwrap(),
        "PodScheduled",
    )["lastTransitionTime"]
        .clone();

    runtime.set_state(&uid, &exited(0));
    runtime.set_state(&bad_uid, &exited(3));
    kubelet.reconcile_once().await.unwrap();

    let ok = admin.get_pod("default", "ok").await.unwrap();
    assert_eq!(phase(&ok), "Succeeded");
    let terminated = &ok["status"]["containerStatuses"][0]["state"]["terminated"];
    assert_eq!(terminated["exitCode"], 0);
    assert_eq!(terminated["reason"], "Completed");
    assert_eq!(ok["status"]["containerStatuses"][0]["ready"], false);
    let ready = condition(&ok, "Ready");
    assert_eq!(ready["status"], "False");
    assert_eq!(ready["reason"], "PodCompleted");
    assert_ne!(ready_before["status"], ready["status"]);
    // Unchanged conditions keep their transition time; changed ones get a new one.
    assert_eq!(
        condition(&ok, "PodScheduled")["lastTransitionTime"],
        scheduled_before
    );

    let bad = admin.get_pod("default", "bad").await.unwrap();
    assert_eq!(phase(&bad), "Failed");
    let terminated = &bad["status"]["containerStatuses"][0]["state"]["terminated"];
    assert_eq!(terminated["exitCode"], 3);
    assert_eq!(terminated["reason"], "Error");

    // Terminal pods are left alone: no restart, no new start, no status rewrite.
    let before = resource_version(&admin, "ok").await;
    kubelet.reconcile_once().await.unwrap();
    assert_eq!(before, resource_version(&admin, "ok").await);
    assert_eq!(runtime.started_count(), 2);

    // A terminal pod is deleted at once by the apiserver; the kubelet then stops
    // its leftover containers as orphans.
    admin.delete_pod("default", "ok").await.unwrap();
    assert!(admin.get_pod("default", "ok").await.is_err());
    let report = kubelet.reconcile_once().await.unwrap();
    assert_eq!(report.orphans_stopped, 1);
    assert_eq!(*runtime.stopped.lock().unwrap(), vec![uid.clone()]);
}

#[tokio::test]
async fn always_policy_restarts_with_crash_loop_back_off() {
    let Started {
        kubelet,
        admin,
        runtime,
        ..
    } = start_pods(vec![pod_with_policy("loop", "Always")]).await;
    let uid = uid_of(&admin, "loop").await;

    // First exit restarts at once as attempt 1; the pod stays Running.
    runtime.set_state(&uid, &exited(1));
    kubelet.reconcile_once().await.unwrap();
    let pod = admin.get_pod("default", "loop").await.unwrap();
    assert_eq!(phase(&pod), "Running");
    let status = &pod["status"]["containerStatuses"][0];
    assert_eq!(status["restartCount"], 1);
    assert_eq!(status["containerID"], format!("scripted://{uid}-hello-1"));
    assert!(status["state"]["running"].is_object());
    assert_eq!(runtime.started_count(), 2);

    // A second exit inside the back-off window waits as CrashLoopBackOff.
    runtime.set_state(&uid, &exited(1));
    kubelet.reconcile_once().await.unwrap();
    let pod = admin.get_pod("default", "loop").await.unwrap();
    assert_eq!(phase(&pod), "Running");
    let status = &pod["status"]["containerStatuses"][0];
    assert_eq!(status["restartCount"], 1);
    assert_eq!(status["ready"], false);
    assert_eq!(status["state"]["waiting"]["reason"], "CrashLoopBackOff");
    let message = status["state"]["waiting"]["message"].as_str().unwrap();
    assert!(
        message.starts_with("back-off 10s restarting failed container=hello pod=loop_default("),
        "{message}"
    );
    assert_eq!(status["lastState"]["terminated"]["exitCode"], 1);
    let ready = condition(&pod, "Ready");
    assert_eq!(ready["status"], "False");
    assert_eq!(ready["reason"], "ContainersNotReady");
    assert_eq!(ready["message"], "containers with unready status: [hello]");
    assert_eq!(runtime.started_count(), 2);

    // OnFailure treats a clean exit as completion.
    admin
        .create_pod("default", pod_with_policy("onfail", "OnFailure"))
        .await
        .unwrap();
    kubelet.reconcile_once().await.unwrap();
    let onfail_uid = uid_of(&admin, "onfail").await;
    runtime.set_state(&onfail_uid, &exited(0));
    kubelet.reconcile_once().await.unwrap();
    assert_eq!(
        phase(&admin.get_pod("default", "onfail").await.unwrap()),
        "Succeeded"
    );
}

#[tokio::test]
async fn deleting_a_running_pod_terminates_gracefully_then_removes_it() {
    let Started {
        kubelet,
        admin,
        runtime,
        ..
    } = start_pods(vec![pod_with_policy("web", "Always")]).await;
    let uid = uid_of(&admin, "web").await;

    // Graceful deletion marks the object instead of removing it.
    let marked = admin
        .delete_pod_options("default", "web", None)
        .await
        .unwrap()
        .expect("running pod is marked, not removed");
    assert!(marked["metadata"]["deletionTimestamp"].is_string());
    assert_eq!(marked["metadata"]["deletionGracePeriodSeconds"], 30);
    assert!(admin.get_pod("default", "web").await.is_ok());

    // First pass: TERM is sent once; the process exits with 143.
    kubelet.reconcile_once().await.unwrap();
    assert_eq!(
        *runtime.signals.lock().unwrap(),
        vec![(uid.clone(), PodSignal::Terminate)]
    );
    assert!(admin.get_pod("default", "web").await.is_ok());

    // Second pass: final status is recorded, containers removed, object deleted.
    kubelet.reconcile_once().await.unwrap();
    assert!(admin.get_pod("default", "web").await.is_err());
    assert_eq!(*runtime.stopped.lock().unwrap(), vec![uid.clone()]);
    // Even under restartPolicy Always the exited container was not restarted.
    assert_eq!(runtime.started_count(), 1);

    // A second create of the same name works and gets a fresh uid.
    admin
        .create_pod("default", pod_with_policy("web", "Always"))
        .await
        .unwrap();
    kubelet.reconcile_once().await.unwrap();
    let again = admin.get_pod("default", "web").await.unwrap();
    assert_eq!(phase(&again), "Running");
    assert_ne!(again["metadata"]["uid"], uid);
}

#[tokio::test]
async fn log_reader_rejects_pods_that_are_not_running_here() {
    let dir = TempDir::new().unwrap();
    let (apiserver, options) = setup(&dir);
    let runtime = Arc::new(ScriptedRuntime::default());
    let source = KubeletService::new(options, apiserver, runtime).log_source();
    let options = PodLogOptions::default();

    let unbound = one_shot_pod("unbound");
    let err = source.read_pod_log(&unbound, &options).await.unwrap_err();
    assert!(err.to_string().contains("not assigned"), "{err}");

    let mut waiting = one_shot_pod("waiting");
    waiting["metadata"]["uid"] = json!("uid-waiting");
    waiting["spec"]["nodeName"] = json!("test-node");
    waiting["status"] = json!({
        "containerStatuses": [{ "name": "hello", "state": { "waiting": { "reason": "ContainerCreating" } } }]
    });
    let err = source.read_pod_log(&waiting, &options).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("waiting to start: ContainerCreating"),
        "{err}"
    );
}
