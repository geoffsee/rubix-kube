#![allow(clippy::too_many_lines)]

use std::net::IpAddr;
use std::sync::Arc;
use tempfile::TempDir;

use rubix_apiserver::{ApiserverConfig, ApiserverService, KubernetesStorage};
use rubix_datastore::{DatastoreConfig, DatastoreEngine};
use rubix_kubelet::{ExecResult, KubeletConfigOptions, KubeletService, MockRuntimeProvider};
use rubix_pki::cluster::{ClusterPki, ClusterPkiConfig};

fn setup_test_environment(
    dir: &TempDir,
) -> (
    ApiserverService,
    KubeletConfigOptions,
    DatastoreEngine,
    std::path::PathBuf,
) {
    let node_ip: IpAddr = "192.0.2.1".parse().unwrap();
    let pki_dir = dir.path().join("pki");
    std::fs::create_dir_all(&pki_dir).unwrap();
    let datastore_dir = dir.path().join("datastore");
    let kubelet_dir = dir.path().join("kubelet");
    std::fs::create_dir_all(&kubelet_dir).unwrap();

    let pki_config = ClusterPkiConfig::new(pki_dir.clone(), "test-node".to_string(), node_ip);
    let pki = ClusterPki::new(pki_config);
    pki.reconcile().expect("PKI reconcile");

    let (engine, _) = DatastoreEngine::open(DatastoreConfig::new(datastore_dir)).unwrap();
    let storage = KubernetesStorage::new(engine.client(), "/registry");

    let apiserver_config = ApiserverConfig::default_for_pki(&pki_dir, node_ip);
    let apiserver_service = ApiserverService::new(apiserver_config, storage);

    let kubelet_options =
        KubeletConfigOptions::default_for_pki(&pki_dir, "test-node", "192.0.2.1", &kubelet_dir);

    (apiserver_service, kubelet_options, engine, pki_dir)
}

#[tokio::test]
async fn test_pod_lifecycle_probes_and_logs_exec_managed_runtime() {
    let temp = TempDir::new().unwrap();
    let (apiserver, kubelet_options, _engine, _pki_dir) = setup_test_environment(&temp);

    apiserver.check_prerequisites().await.unwrap();
    apiserver.start().unwrap();
    let apiserver_arc = Arc::new(apiserver);

    let runtime = Arc::new(MockRuntimeProvider::new("managed-containerd"));
    let kubelet = KubeletService::new(kubelet_options, apiserver_arc.clone(), runtime.clone());
    kubelet.start().await.unwrap();

    let client = kubelet.client();

    // 1. Configure initial probe responses so both readiness and liveness pass
    runtime.set_exec_response(
        "cat /tmp/ready",
        ExecResult {
            exit_code: 0,
            stdout: "ready\n".to_string(),
            stderr: String::new(),
        },
    );
    runtime.set_exec_response(
        "cat /tmp/healthy",
        ExecResult {
            exit_code: 0,
            stdout: "healthy\n".to_string(),
            stderr: String::new(),
        },
    );

    // 2. Create pod with readinessProbe and livenessProbe
    let pod_spec = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "lifecycle-managed-pod",
            "namespace": "default",
            "uid": "uid-lifecycle-managed"
        },
        "spec": {
            "nodeName": "test-node",
            "restartPolicy": "Always",
            "containers": [
                {
                    "name": "web",
                    "image": "docker.io/library/nginx:1.27",
                    "readinessProbe": {
                        "exec": {
                            "command": ["cat", "/tmp/ready"]
                        }
                    },
                    "livenessProbe": {
                        "exec": {
                            "command": ["cat", "/tmp/healthy"]
                        }
                    }
                }
            ]
        }
    });

    client.create_pod("default", pod_spec).await.unwrap();

    // Initial reconciliation
    let count = kubelet
        .reconciler()
        .reconcile_namespace("default")
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Verify initial Running state with Ready=True
    let pod = client
        .get_pod("default", "lifecycle-managed-pod")
        .await
        .unwrap();
    assert_eq!(pod["status"]["phase"], "Running");
    assert_eq!(pod["status"]["containerStatuses"][0]["ready"], true);
    assert_eq!(pod["status"]["containerStatuses"][0]["restartCount"], 0);

    let conditions = pod["status"]["conditions"].as_array().unwrap();
    let ready_cond = conditions
        .iter()
        .find(|c| c["type"] == "Ready")
        .expect("Ready condition");
    assert_eq!(ready_cond["status"], "True");

    // 3. Test logs fetching and tailing
    runtime.set_container_logs(
        "web",
        "line 1: start\nline 2: init\nline 3: listening\nline 4: request\nline 5: done\n",
    );
    let all_logs = kubelet
        .get_container_logs("uid-lifecycle-managed", "web", None)
        .await
        .unwrap();
    assert!(all_logs.contains("line 1: start"));
    assert!(all_logs.contains("line 5: done"));

    let tailed_logs = kubelet
        .get_container_logs("uid-lifecycle-managed", "web", Some(2))
        .await
        .unwrap();
    assert!(!tailed_logs.contains("line 1: start"));
    assert!(tailed_logs.contains("line 4: request"));
    assert!(tailed_logs.contains("line 5: done"));

    // 4. Test exec execution
    let echo_res = kubelet
        .exec_in_container(
            "uid-lifecycle-managed",
            "web",
            &[
                "echo".to_string(),
                "hello".to_string(),
                "kubelet".to_string(),
            ],
        )
        .await
        .unwrap();
    assert_eq!(echo_res.exit_code, 0);
    assert_eq!(echo_res.stdout.trim(), "hello kubelet");

    runtime.set_exec_response(
        "uname -a",
        ExecResult {
            exit_code: 0,
            stdout: "Linux rubix-node 6.6.0\n".to_string(),
            stderr: String::new(),
        },
    );
    let custom_exec = kubelet
        .exec_in_container(
            "uid-lifecycle-managed",
            "web",
            &["uname".to_string(), "-a".to_string()],
        )
        .await
        .unwrap();
    assert_eq!(custom_exec.exit_code, 0);
    assert!(custom_exec.stdout.contains("Linux rubix-node"));

    // 5. Test readiness probe failure and recovery
    runtime.set_exec_response(
        "cat /tmp/ready",
        ExecResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: "not ready\n".to_string(),
        },
    );

    kubelet
        .reconciler()
        .reconcile_namespace("default")
        .await
        .unwrap();
    let pod_unready = client
        .get_pod("default", "lifecycle-managed-pod")
        .await
        .unwrap();
    assert_eq!(
        pod_unready["status"]["containerStatuses"][0]["ready"],
        false
    );
    let conditions = pod_unready["status"]["conditions"].as_array().unwrap();
    let ready_cond = conditions
        .iter()
        .find(|c| c["type"] == "Ready")
        .expect("Ready condition");
    assert_eq!(ready_cond["status"], "False");

    // Recover readiness probe
    runtime.set_exec_response(
        "cat /tmp/ready",
        ExecResult {
            exit_code: 0,
            stdout: "ready again\n".to_string(),
            stderr: String::new(),
        },
    );
    kubelet
        .reconciler()
        .reconcile_namespace("default")
        .await
        .unwrap();
    let pod_recovered = client
        .get_pod("default", "lifecycle-managed-pod")
        .await
        .unwrap();
    assert_eq!(
        pod_recovered["status"]["containerStatuses"][0]["ready"],
        true
    );
    let conditions = pod_recovered["status"]["conditions"].as_array().unwrap();
    let ready_cond = conditions
        .iter()
        .find(|c| c["type"] == "Ready")
        .expect("Ready condition");
    assert_eq!(ready_cond["status"], "True");

    // 6. Test liveness probe failure with container restart
    runtime.set_exec_response(
        "cat /tmp/healthy",
        ExecResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: "deadlock\n".to_string(),
        },
    );

    kubelet
        .reconciler()
        .reconcile_namespace("default")
        .await
        .unwrap();
    let pod_restarted = client
        .get_pod("default", "lifecycle-managed-pod")
        .await
        .unwrap();
    assert_eq!(
        pod_restarted["status"]["containerStatuses"][0]["restartCount"],
        1
    );
    assert_eq!(
        pod_restarted["status"]["containerStatuses"][0]["ready"],
        false
    );

    // Fix liveness probe
    runtime.set_exec_response(
        "cat /tmp/healthy",
        ExecResult {
            exit_code: 0,
            stdout: "healthy\n".to_string(),
            stderr: String::new(),
        },
    );
    kubelet
        .reconciler()
        .reconcile_namespace("default")
        .await
        .unwrap();
    let pod_stable = client
        .get_pod("default", "lifecycle-managed-pod")
        .await
        .unwrap();
    assert_eq!(
        pod_stable["status"]["containerStatuses"][0]["restartCount"],
        1
    );
    assert_eq!(pod_stable["status"]["containerStatuses"][0]["ready"], true);
}

#[tokio::test]
async fn test_pod_lifecycle_probes_and_logs_exec_external_runtime() {
    let temp = TempDir::new().unwrap();
    let (apiserver, kubelet_options, _engine, _pki_dir) = setup_test_environment(&temp);

    apiserver.check_prerequisites().await.unwrap();
    apiserver.start().unwrap();
    let apiserver_arc = Arc::new(apiserver);

    let runtime = Arc::new(MockRuntimeProvider::new_external("external-podman"));
    let kubelet = KubeletService::new(kubelet_options, apiserver_arc.clone(), runtime.clone());
    kubelet.start().await.unwrap();

    let client = kubelet.client();

    runtime.set_exec_response(
        "cat /tmp/ready",
        ExecResult {
            exit_code: 0,
            stdout: "ok\n".to_string(),
            stderr: String::new(),
        },
    );
    runtime.set_exec_response(
        "cat /tmp/healthy",
        ExecResult {
            exit_code: 0,
            stdout: "ok\n".to_string(),
            stderr: String::new(),
        },
    );

    let pod_spec = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "lifecycle-external-pod",
            "namespace": "default",
            "uid": "uid-lifecycle-external"
        },
        "spec": {
            "nodeName": "test-node",
            "restartPolicy": "Always",
            "containers": [
                {
                    "name": "worker",
                    "image": "docker.io/library/redis:7.2",
                    "readinessProbe": {
                        "exec": {
                            "command": ["cat", "/tmp/ready"]
                        }
                    },
                    "livenessProbe": {
                        "exec": {
                            "command": ["cat", "/tmp/healthy"]
                        }
                    }
                }
            ]
        }
    });

    client.create_pod("default", pod_spec).await.unwrap();

    let count = kubelet
        .reconciler()
        .reconcile_namespace("default")
        .await
        .unwrap();
    assert_eq!(count, 1);

    let pod = client
        .get_pod("default", "lifecycle-external-pod")
        .await
        .unwrap();
    assert_eq!(pod["status"]["phase"], "Running");
    assert_eq!(pod["status"]["containerStatuses"][0]["ready"], true);
    assert_eq!(pod["status"]["containerStatuses"][0]["restartCount"], 0);

    // Verify logs
    runtime.set_container_logs("worker", "redis 1\nredis 2\nredis 3\n");
    let logs = kubelet
        .get_container_logs("uid-lifecycle-external", "worker", Some(1))
        .await
        .unwrap();
    assert_eq!(logs.trim(), "redis 3");

    // Verify exec
    let res = kubelet
        .exec_in_container(
            "uid-lifecycle-external",
            "worker",
            &["echo".to_string(), "ping".to_string()],
        )
        .await
        .unwrap();
    assert_eq!(res.exit_code, 0);
    assert_eq!(res.stdout.trim(), "ping");

    // Liveness failure and restart on external runtime
    runtime.set_exec_response(
        "cat /tmp/healthy",
        ExecResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: "redis frozen\n".to_string(),
        },
    );

    kubelet
        .reconciler()
        .reconcile_namespace("default")
        .await
        .unwrap();
    let pod_restarted = client
        .get_pod("default", "lifecycle-external-pod")
        .await
        .unwrap();
    assert_eq!(
        pod_restarted["status"]["containerStatuses"][0]["restartCount"],
        1
    );
    assert_eq!(
        pod_restarted["status"]["containerStatuses"][0]["ready"],
        false
    );
}

#[tokio::test]
async fn test_secret_and_configmap_volume_mounts() {
    let temp = TempDir::new().unwrap();
    let (apiserver, kubelet_options, _engine, _pki_dir) = setup_test_environment(&temp);

    apiserver.check_prerequisites().await.unwrap();
    apiserver.start().unwrap();
    let apiserver_arc = Arc::new(apiserver);

    let runtime = Arc::new(MockRuntimeProvider::new("managed-containerd"));
    let kubelet = KubeletService::new(kubelet_options, apiserver_arc.clone(), runtime);
    kubelet.start().await.unwrap();

    let admin_client = apiserver_arc.admin_client();
    let client = kubelet.client();

    // 1. Create Secret in default namespace
    let mut secret_data = std::collections::BTreeMap::new();
    secret_data.insert(
        "username".to_string(),
        rubix_pki::base64_encode(b"admin-user"),
    );
    secret_data.insert(
        "password".to_string(),
        rubix_pki::base64_encode(b"super-secret-pwd"),
    );
    admin_client
        .create_secret("default", "db-secret", secret_data, None)
        .await
        .unwrap();

    // 2. Create ConfigMap in default namespace
    let mut cm_data = std::collections::BTreeMap::new();
    cm_data.insert(
        "app.conf".to_string(),
        "database_url = postgres://admin-user:super-secret-pwd@db:5432/main".to_string(),
    );
    cm_data.insert("features.env".to_string(), "ENABLE_METRICS=1".to_string());
    admin_client
        .create_configmap("default", "app-config", cm_data)
        .await
        .unwrap();

    // 3. Create Pod referencing both Secret and ConfigMap volumes
    let pod_spec = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "vol-test-pod",
            "namespace": "default",
            "uid": "uid-vol-test-123"
        },
        "spec": {
            "nodeName": "test-node",
            "containers": [
                {
                    "name": "app",
                    "image": "docker.io/library/alpine:3.20"
                }
            ],
            "volumes": [
                {
                    "name": "secret-vol-mapped",
                    "secret": {
                        "secretName": "db-secret",
                        "items": [
                            {
                                "key": "username",
                                "path": "custom-user.txt"
                            }
                        ]
                    }
                },
                {
                    "name": "secret-vol-all",
                    "secret": {
                        "secretName": "db-secret"
                    }
                },
                {
                    "name": "cm-vol-mapped",
                    "configMap": {
                        "name": "app-config",
                        "items": [
                            {
                                "key": "app.conf",
                                "path": "config/app.conf"
                            }
                        ]
                    }
                },
                {
                    "name": "cm-vol-all",
                    "configMap": {
                        "name": "app-config"
                    }
                }
            ]
        }
    });

    client.create_pod("default", pod_spec).await.unwrap();

    // Reconcile pod
    let count = kubelet
        .reconciler()
        .reconcile_namespace("default")
        .await
        .unwrap();
    assert_eq!(count, 1);

    // 4. Verify volume staging paths on host filesystem
    let sec_mapped_dir = kubelet.get_pod_volume_dir(
        "uid-vol-test-123",
        "kubernetes.io~secret",
        "secret-vol-mapped",
    );
    assert!(sec_mapped_dir.exists());
    let user_content =
        std::fs::read_to_string(sec_mapped_dir.join("custom-user.txt")).expect("custom-user.txt");
    assert_eq!(user_content, "admin-user");

    let sec_all_dir =
        kubelet.get_pod_volume_dir("uid-vol-test-123", "kubernetes.io~secret", "secret-vol-all");
    assert!(sec_all_dir.exists());
    let all_username =
        std::fs::read_to_string(sec_all_dir.join("username")).expect("username file");
    assert_eq!(all_username, "admin-user");
    let all_password =
        std::fs::read_to_string(sec_all_dir.join("password")).expect("password file");
    assert_eq!(all_password, "super-secret-pwd");

    let cm_mapped_dir = kubelet.get_pod_volume_dir(
        "uid-vol-test-123",
        "kubernetes.io~configmap",
        "cm-vol-mapped",
    );
    assert!(cm_mapped_dir.exists());
    let app_conf = std::fs::read_to_string(cm_mapped_dir.join("config/app.conf"))
        .expect("config/app.conf file");
    assert!(app_conf.contains("postgres://admin-user:super-secret-pwd@db:5432/main"));

    let cm_all_dir =
        kubelet.get_pod_volume_dir("uid-vol-test-123", "kubernetes.io~configmap", "cm-vol-all");
    assert!(cm_all_dir.exists());
    let features_env =
        std::fs::read_to_string(cm_all_dir.join("features.env")).expect("features.env file");
    assert_eq!(features_env, "ENABLE_METRICS=1");
}

#[tokio::test]
async fn test_projected_volume_and_persistence_across_kubelet_restart() {
    let temp = TempDir::new().unwrap();
    let (apiserver, kubelet_options, _engine, _pki_dir) = setup_test_environment(&temp);

    apiserver.check_prerequisites().await.unwrap();
    apiserver.start().unwrap();
    let apiserver_arc = Arc::new(apiserver);

    let runtime1 = Arc::new(MockRuntimeProvider::new("managed-containerd"));
    let kubelet1 = KubeletService::new(
        kubelet_options.clone(),
        apiserver_arc.clone(),
        runtime1.clone(),
    );
    kubelet1.start().await.unwrap();

    let admin_client = apiserver_arc.admin_client();
    let client = kubelet1.client();

    // Create supporting ConfigMap and Secret
    let mut cm_data = std::collections::BTreeMap::new();
    cm_data.insert(
        "ca.crt".to_string(),
        "-----BEGIN CERTIFICATE-----\nMOCK_CA\n-----END CERTIFICATE-----".to_string(),
    );
    admin_client
        .create_configmap("default", "kube-root-ca", cm_data)
        .await
        .unwrap();

    let mut sec_data = std::collections::BTreeMap::new();
    sec_data.insert(
        "api-key".to_string(),
        rubix_pki::base64_encode(b"vault-secret-api-key-xyz"),
    );
    admin_client
        .create_secret("default", "vault-creds", sec_data, None)
        .await
        .unwrap();

    // Create Pod with Projected volume
    let pod_spec = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "projected-vol-pod",
            "namespace": "default",
            "uid": "uid-proj-999"
        },
        "spec": {
            "nodeName": "test-node",
            "serviceAccountName": "default",
            "containers": [
                {
                    "name": "vault-agent",
                    "image": "docker.io/hashicorp/vault:1.15"
                }
            ],
            "volumes": [
                {
                    "name": "kube-api-access",
                    "projected": {
                        "sources": [
                            {
                                "serviceAccountToken": {
                                    "path": "token",
                                    "audience": "vault",
                                    "expirationSeconds": 3600
                                }
                            },
                            {
                                "configMap": {
                                    "name": "kube-root-ca",
                                    "items": [
                                        {
                                            "key": "ca.crt",
                                            "path": "ca.crt"
                                        }
                                    ]
                                }
                            },
                            {
                                "downwardAPI": {
                                    "items": [
                                        {
                                            "path": "namespace",
                                            "fieldRef": {
                                                "apiVersion": "v1",
                                                "fieldPath": "metadata.namespace"
                                            }
                                        },
                                        {
                                            "path": "pod-name",
                                            "fieldRef": {
                                                "apiVersion": "v1",
                                                "fieldPath": "metadata.name"
                                            }
                                        },
                                        {
                                            "path": "pod-uid",
                                            "fieldRef": {
                                                "apiVersion": "v1",
                                                "fieldPath": "metadata.uid"
                                            }
                                        }
                                    ]
                                }
                            },
                            {
                                "secret": {
                                    "name": "vault-creds"
                                }
                            }
                        ]
                    }
                }
            ]
        }
    });

    client.create_pod("default", pod_spec).await.unwrap();

    // Reconcile on first Kubelet instance
    let count = kubelet1
        .reconciler()
        .reconcile_namespace("default")
        .await
        .unwrap();
    assert_eq!(count, 1);

    let proj_dir =
        kubelet1.get_pod_volume_dir("uid-proj-999", "kubernetes.io~projected", "kube-api-access");
    assert!(proj_dir.exists());

    // Verify token projection
    let token_content = std::fs::read_to_string(proj_dir.join("token")).expect("token file");
    assert!(!token_content.trim().is_empty());

    // Verify CA bundle
    let ca_content = std::fs::read_to_string(proj_dir.join("ca.crt")).expect("ca.crt file");
    assert!(ca_content.contains("BEGIN CERTIFICATE"));

    // Verify downwardAPI metadata files
    let ns_content = std::fs::read_to_string(proj_dir.join("namespace")).expect("namespace file");
    assert_eq!(ns_content, "default");
    let name_content = std::fs::read_to_string(proj_dir.join("pod-name")).expect("pod-name file");
    assert_eq!(name_content, "projected-vol-pod");
    let uid_content = std::fs::read_to_string(proj_dir.join("pod-uid")).expect("pod-uid file");
    assert_eq!(uid_content, "uid-proj-999");

    // Verify projected secret
    let secret_key = std::fs::read_to_string(proj_dir.join("api-key")).expect("api-key file");
    assert_eq!(secret_key, "vault-secret-api-key-xyz");

    // 5. Test persistence across Kubelet restart with external runtime
    kubelet1.stop();

    let runtime2 = Arc::new(MockRuntimeProvider::new_external("external-podman"));
    let kubelet2 = KubeletService::new(
        kubelet_options.clone(),
        apiserver_arc.clone(),
        runtime2.clone(),
    );
    kubelet2.start().await.unwrap();

    // Re-reconcile with restarted Kubelet
    let count2 = kubelet2
        .reconciler()
        .reconcile_namespace("default")
        .await
        .unwrap();
    assert_eq!(count2, 1);

    let proj_dir_after_restart =
        kubelet2.get_pod_volume_dir("uid-proj-999", "kubernetes.io~projected", "kube-api-access");
    assert!(proj_dir_after_restart.exists());
    assert_eq!(
        std::fs::read_to_string(proj_dir_after_restart.join("pod-name")).unwrap(),
        "projected-vol-pod"
    );
    assert_eq!(
        std::fs::read_to_string(proj_dir_after_restart.join("api-key")).unwrap(),
        "vault-secret-api-key-xyz"
    );

    let pod_after = kubelet2
        .client()
        .get_pod("default", "projected-vol-pod")
        .await
        .unwrap();
    assert_eq!(pod_after["status"]["phase"], "Running");
    assert_eq!(pod_after["status"]["containerStatuses"][0]["ready"], true);
}
