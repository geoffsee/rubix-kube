//! Pod status must follow what the runtime actually observes: Pending while
//! nothing runs, Running while a process is alive, then Succeeded or Failed
//! from the exit code. Deleted pods must have their containers stopped.

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
    KubeletService, ManagedPodRef, PodRuntimeStatus, RuntimeProvider,
};
use rubix_pki::cluster::{ClusterPki, ClusterPkiConfig};

/// Scripted runtime: containers it "starts" sit in a chosen state until the test
/// moves them on, mirroring how podman reports a long-lived or exited process.
#[derive(Debug, Default)]
struct ScriptedRuntime {
    containers: Mutex<BTreeMap<String, Vec<ContainerRuntimeStatus>>>,
    started: Mutex<Vec<String>>,
    stopped: Mutex<Vec<String>>,
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
            .map(|c| ContainerRuntimeStatus {
                name: c["name"].as_str().unwrap().to_string(),
                container_id: format!("scripted://{uid}-{}", c["name"].as_str().unwrap()),
                image: c["image"].as_str().unwrap().to_string(),
                image_id: "sha256:image".to_string(),
                state: ContainerRuntimeState::Running {
                    started_at: "2026-10-04T12:00:00Z".to_string(),
                },
            })
            .collect();
        self.containers
            .lock()
            .unwrap()
            .insert(uid.clone(), containers);
        self.started.lock().unwrap().push(uid.clone());
        Ok(uid)
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

    async fn get_container_logs(
        &self,
        pod_id: &str,
        container_name: &str,
        tail_lines: Option<usize>,
    ) -> Result<String, KubeletError> {
        Ok(format!(
            "rubix-ok {pod_id} {container_name} {tail_lines:?}\n"
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

fn one_shot_pod(name: &str) -> Value {
    json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": name, "namespace": "default" },
        "spec": {
            "restartPolicy": "Never",
            "containers": [{
                "name": "hello",
                "image": "localhost/rubix-hello:latest",
                "command": ["/rubix-hello"]
            }]
        }
    })
}

fn phase(pod: &Value) -> &str {
    pod["status"]["phase"].as_str().unwrap_or("")
}

struct Started {
    _dir: TempDir,
    kubelet: KubeletService,
    admin: KubernetesApiClient,
    runtime: Arc<ScriptedRuntime>,
}

/// Creates two unbound one-shot pods and runs the first kubelet pass over them.
async fn start_two_pods() -> Started {
    let dir = TempDir::new().unwrap();
    let (apiserver, options) = setup(&dir);
    apiserver.check_prerequisites().await.unwrap();
    apiserver.start().unwrap();
    let runtime = Arc::new(ScriptedRuntime::default());
    let kubelet = KubeletService::new(options, apiserver.clone(), runtime.clone());
    kubelet.start().await.unwrap();
    let admin = apiserver.admin_client();
    admin
        .create_pod("default", one_shot_pod("ok"))
        .await
        .unwrap();
    admin
        .create_pod("default", one_shot_pod("bad"))
        .await
        .unwrap();
    let report = kubelet.reconcile_once().await.unwrap();
    assert_eq!((report.bound, report.synced, report.failed), (2, 2, 0));
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
    } = start_two_pods().await;

    let ok = admin.get_pod("default", "ok").await.unwrap();
    assert_eq!(ok["spec"]["nodeName"], "test-node");
    assert_eq!(phase(&ok), "Running");
    let status = &ok["status"]["containerStatuses"][0];
    let uid = uid_of(&admin, "ok").await;
    assert_eq!(status["containerID"], format!("scripted://{uid}-hello"));
    assert_eq!(status["ready"], true);
    assert!(status["state"]["running"]["startedAt"].is_string());
    assert_eq!(runtime.started_count(), 2);

    // Logs are read through the kubelet for the stored pod object.
    let text = kubelet
        .log_source()
        .read_pod_log(&ok, &PodLogOptions::default())
        .await
        .unwrap();
    assert_eq!(text, format!("rubix-ok {uid} hello None\n"));

    // Nothing changed, so a second pass writes no status and starts nothing new.
    let before = resource_version(&admin, "ok").await;
    kubelet.reconcile_once().await.unwrap();
    assert_eq!(before, resource_version(&admin, "ok").await);
    assert_eq!(runtime.started_count(), 2);

    // A pod assigned elsewhere is ignored.
    let mut elsewhere = one_shot_pod("elsewhere");
    elsewhere["spec"]["nodeName"] = json!("other-node");
    admin.create_pod("default", elsewhere).await.unwrap();
    let report = kubelet.reconcile_once().await.unwrap();
    assert_eq!(report.bound, 0);
    assert_eq!(runtime.started_count(), 2);
    let elsewhere = admin.get_pod("default", "elsewhere").await.unwrap();
    assert!(elsewhere.get("status").is_none());
}

#[tokio::test]
async fn exited_processes_become_terminal_and_deleted_pods_are_stopped() {
    let Started {
        kubelet,
        admin,
        runtime,
        ..
    } = start_two_pods().await;
    let uid = uid_of(&admin, "ok").await;
    let bad_uid = uid_of(&admin, "bad").await;

    // The processes exit: 0 becomes Succeeded, 3 becomes Failed.
    runtime.set_state(
        &uid,
        &ContainerRuntimeState::Terminated {
            exit_code: 0,
            started_at: Some("2026-10-04T12:00:00Z".to_string()),
            finished_at: "2026-10-04T12:00:05Z".to_string(),
        },
    );
    runtime.set_state(
        &bad_uid,
        &ContainerRuntimeState::Terminated {
            exit_code: 3,
            started_at: None,
            finished_at: "2026-10-04T12:00:05Z".to_string(),
        },
    );
    kubelet.reconcile_once().await.unwrap();
    let ok = admin.get_pod("default", "ok").await.unwrap();
    assert_eq!(phase(&ok), "Succeeded");
    let terminated = &ok["status"]["containerStatuses"][0]["state"]["terminated"];
    assert_eq!(terminated["exitCode"], 0);
    assert_eq!(terminated["reason"], "Completed");
    assert_eq!(ok["status"]["containerStatuses"][0]["ready"], false);
    let ready = ok["status"]["conditions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["type"] == "Ready")
        .unwrap();
    assert_eq!(ready["status"], "False");
    assert_eq!(ready["reason"], "PodCompleted");
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

    // Deleting the Pod object stops its containers on the next pass.
    admin.delete_pod("default", "ok").await.unwrap();
    let report = kubelet.reconcile_once().await.unwrap();
    assert_eq!(report.orphans_stopped, 1);
    assert_eq!(*runtime.stopped.lock().unwrap(), vec![uid.clone()]);
    assert!(
        runtime
            .inspect_pod(&ok)
            .await
            .unwrap()
            .unwrap()
            .containers
            .is_empty()
    );
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
