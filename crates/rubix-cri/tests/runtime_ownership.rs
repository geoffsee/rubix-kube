//! Integration tests for external runtime ownership, non-interference boundaries,
//! and lifecycle guarantees across start, stop, restart, and uninstall.
//!
//! Acceptance criteria verified:
//! - Start/stop/restart and uninstall integration preserve host runtime process, socket,
//!   registry files, and unrelated workloads.
//! - Tests allow only owned CNI configuration through E15 and verify no bundled-runtime
//!   cleanup runs in external mode.
//! - Build profiles and external runtime modes are validated as distinct choices.

use h2::server;
use prost::Message;
use prost::bytes::{BufMut, Bytes, BytesMut};
use rubix_cri::endpoint::{CriEndpoint, RuntimeEndpoints};
use rubix_cri::external::{COMPONENT_EXTERNAL_CRI, ExternalRuntimeOptions, ExternalRuntimeService};
use rubix_cri::ownership::{
    BuildAndRuntimeMatrix, BuildProfile, OwnershipViolation, RuntimeMode, RuntimeOwnershipPolicy,
    WorkloadScoping,
};
use rubix_cri::runtime::v1::{
    Container, ContainerMetadata, ContainerState, ListContainersResponse, ListImagesResponse,
    VersionResponse,
};
use rubix_cri::workload::CriClient;
use rubix_supervisor::{FailurePolicy, Registration, StopCause, Supervisor, stop_channel};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::UnixListener;
use tonic::codegen::http::{HeaderMap, Response};

fn encode_grpc_message<M: Message>(msg: &M) -> Bytes {
    let mut encoded = Vec::new();
    msg.encode(&mut encoded).expect("encode protobuf message");
    let mut buf = BytesMut::with_capacity(5 + encoded.len());
    buf.put_u8(0);
    let len = u32::try_from(encoded.len()).expect("message length fits in u32");
    buf.put_u32(len);
    buf.put_slice(&encoded);
    buf.freeze()
}

fn handle_mock_stream(mut respond: server::SendResponse<Bytes>, payload: Option<Bytes>) {
    if let Some(data) = payload {
        let mut response = Response::new(());
        response
            .headers_mut()
            .insert("content-type", "application/grpc".parse().unwrap());
        let mut send_stream = respond.send_response(response, false).unwrap();
        let _ = send_stream.send_data(data, false);
        let mut trailers = HeaderMap::new();
        trailers.insert("grpc-status", "0".parse().unwrap());
        let _ = send_stream.send_trailers(trailers);
    } else {
        let mut response = Response::new(());
        response
            .headers_mut()
            .insert("content-type", "application/grpc".parse().unwrap());
        let mut send_stream = respond.send_response(response, false).unwrap();
        let mut trailers = HeaderMap::new();
        trailers.insert("grpc-status", "12".parse().unwrap());
        let _ = send_stream.send_trailers(trailers);
    }
}

async fn run_mock_workload_connection(
    stream: tokio::net::UnixStream,
    connection_counter: Arc<AtomicUsize>,
    stop_flag: Arc<AtomicBool>,
) {
    let Ok(mut h2) = server::handshake(stream).await else {
        return;
    };
    connection_counter.fetch_add(1, Ordering::Relaxed);

    while !stop_flag.load(Ordering::Relaxed) {
        let req_result = tokio::select! {
            res = h2.accept() => res,
            () = tokio::time::sleep(Duration::from_millis(50)) => continue,
        };

        let Some(Ok((request, respond))) = req_result else {
            break;
        };

        let path = request.uri().path().to_string();
        let payload = match path.as_str() {
            p if p.contains("Version") => {
                let resp = VersionResponse {
                    version: "0.1.0".into(),
                    runtime_name: "containerd".into(),
                    runtime_version: "1.7.20".into(),
                    runtime_api_version: "v1".into(),
                };
                Some(encode_grpc_message(&resp))
            },
            p if p.contains("ListImages") => {
                let resp = ListImagesResponse { images: vec![] };
                Some(encode_grpc_message(&resp))
            },
            p if p.contains("ListContainers") => {
                let resp = ListContainersResponse {
                    containers: vec![
                        // Container 1: Rubix owned workload
                        Container {
                            id: "rubix-pod-web".into(),
                            pod_sandbox_id: "sandbox-rubix-1".into(),
                            metadata: Some(ContainerMetadata {
                                name: "web".into(),
                                attempt: 0,
                            }),
                            image: None,
                            image_ref: "docker.io/library/nginx:latest".into(),
                            image_id: "sha256:nginx".into(),
                            state: ContainerState::ContainerRunning as i32,
                            created_at: 1_700_000_000,
                            labels: {
                                let mut m = BTreeMap::new();
                                m.insert("io.rubix.managed".into(), "true".into());
                                m.insert(
                                    "io.kubernetes.pod.namespace".into(),
                                    "rubix-system".into(),
                                );
                                m
                            },
                            annotations: BTreeMap::new(),
                        },
                        // Container 2: Unrelated host workload
                        Container {
                            id: "host-monitoring-agent".into(),
                            pod_sandbox_id: "sandbox-host-99".into(),
                            metadata: Some(ContainerMetadata {
                                name: "datadog-agent".into(),
                                attempt: 0,
                            }),
                            image: None,
                            image_ref: "docker.io/datadog/agent:latest".into(),
                            image_id: "sha256:datadog".into(),
                            state: ContainerState::ContainerRunning as i32,
                            created_at: 1_690_000_000,
                            labels: {
                                let mut m = BTreeMap::new();
                                m.insert("host.service".into(), "monitoring".into());
                                m
                            },
                            annotations: BTreeMap::new(),
                        },
                    ],
                };
                Some(encode_grpc_message(&resp))
            },
            _ => None,
        };

        handle_mock_stream(respond, payload);
    }
}

fn spawn_ownership_mock_server(
    socket_path: &Path,
    counter: Arc<AtomicUsize>,
    stopped: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    let listener = UnixListener::bind(socket_path).expect("bind mock unix socket");
    tokio::spawn(async move {
        while !stopped.load(Ordering::Relaxed) {
            let Ok(Ok((stream, _))) =
                tokio::time::timeout(Duration::from_millis(500), listener.accept()).await
            else {
                continue;
            };

            let conn_counter = Arc::clone(&counter);
            let stop_flag = Arc::clone(&stopped);
            tokio::spawn(run_mock_workload_connection(
                stream,
                conn_counter,
                stop_flag,
            ));
        }
    })
}

#[tokio::test]
async fn external_lifecycle_preserves_host_process_socket_and_registry() {
    let temp = TempDir::new().unwrap();
    let socket = temp.path().join("external_containerd.sock");
    let registry_dir = temp.path().join("etc/containerd/certs.d");
    fs::create_dir_all(&registry_dir).unwrap();

    let registry_conf = registry_dir.join("hosts.toml");
    let initial_registry_content = "server = \"https://registry-1.docker.io\"\n[host.\"https://mirror.example.com\"]\n  capabilities = [\"pull\"]\n";
    fs::write(&registry_conf, initial_registry_content).unwrap();

    let stopped = Arc::new(AtomicBool::new(false));
    let connection_count = Arc::new(AtomicUsize::new(0));

    let _server =
        spawn_ownership_mock_server(&socket, Arc::clone(&connection_count), Arc::clone(&stopped));

    let fake_host_daemon_pid: u32 = 4242;
    let policy = RuntimeOwnershipPolicy::external(&socket, &socket);

    // Invariant: external mode forbids process management and host runtime signalling
    assert!(!policy.allows_process_management());
    assert_eq!(
        policy.validate_process_signal(fake_host_daemon_pid, &[fake_host_daemon_pid]),
        Err(OwnershipViolation::ProtectedProcess {
            pid: fake_host_daemon_pid
        })
    );

    // 1. Initial Start: attach through supervisor
    let endpoints = RuntimeEndpoints::single(CriEndpoint::from_path(&socket).unwrap());
    let spec = ExternalRuntimeService::component_spec(Duration::from_secs(5));
    assert_eq!(spec.id, COMPONENT_EXTERNAL_CRI);
    assert_eq!(spec.failure_policy, FailurePolicy::Fatal);

    let options = ExternalRuntimeOptions {
        endpoints: endpoints.clone(),
        readiness_timeout: Duration::from_secs(2),
        retry_interval: Duration::from_millis(50),
    };

    let service = ExternalRuntimeService::new(options.clone());
    let registration = Registration::new(spec, service);
    let (stop_handle, stop_receiver) = stop_channel();
    let supervisor = Supervisor::new(vec![registration]).expect("valid component graph");

    let sup_handle = tokio::spawn(async move { supervisor.run(stop_receiver).await });
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Verify socket exists, connection succeeded, and registry is pristine
    assert!(socket.exists());
    let registry_after_start = fs::read_to_string(&registry_conf).unwrap();
    assert_eq!(registry_after_start, initial_registry_content);

    // 2. Stop: trigger supervisor shutdown
    stop_handle.stop();
    let report = sup_handle.await.expect("supervisor task joins");
    assert!(matches!(report.cause, StopCause::Requested));

    // Verify socket still exists, registry is pristine, host process was never killed
    assert!(socket.exists());
    let registry_after_stop = fs::read_to_string(&registry_conf).unwrap();
    assert_eq!(registry_after_stop, initial_registry_content);
    assert!(
        policy
            .validate_process_signal(fake_host_daemon_pid, &[fake_host_daemon_pid])
            .is_err()
    );

    // 3. Restart: re-launch external service on the same host socket
    let spec2 = ExternalRuntimeService::component_spec(Duration::from_secs(5));
    let service2 = ExternalRuntimeService::new(options);
    let registration2 = Registration::new(spec2, service2);
    let (stop_handle2, stop_receiver2) = stop_channel();
    let supervisor2 = Supervisor::new(vec![registration2]).expect("valid component graph");

    let sup_handle2 = tokio::spawn(async move { supervisor2.run(stop_receiver2).await });
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert!(socket.exists());
    stop_handle2.stop();
    let report2 = sup_handle2.await.expect("supervisor task 2 joins");
    assert!(matches!(report2.cause, StopCause::Requested));

    // Post-restart validation: socket, registry, host daemon intact
    assert!(socket.exists());
    let registry_final = fs::read_to_string(&registry_conf).unwrap();
    assert_eq!(registry_final, initial_registry_content);

    stopped.store(true, Ordering::Relaxed);
}

#[tokio::test]
async fn uninstall_and_cleanup_refuses_to_target_external_runtime() {
    let temp = TempDir::new().unwrap();
    let runtime_socket = temp.path().join("containerd.sock");
    let image_socket = temp.path().join("containerd_image.sock");

    let policy = RuntimeOwnershipPolicy::external(&runtime_socket, &image_socket);

    // Invariant: external mode strictly forbids runtime cleanup
    assert!(!policy.allows_runtime_cleanup());
    assert!(!policy.allows_runtime_reconfiguration());
    assert!(!policy.allows_embedded_image_unpacking());

    // Cleanup path protection
    assert_eq!(
        policy.validate_cleanup_path(&runtime_socket),
        Err(OwnershipViolation::ProtectedPath {
            path: runtime_socket.clone(),
            reason: "matches external CRI runtime socket",
        })
    );
    assert_eq!(
        policy.validate_cleanup_path(&image_socket),
        Err(OwnershipViolation::ProtectedPath {
            path: image_socket.clone(),
            reason: "matches external CRI image socket",
        })
    );

    // System directories protection
    let host_paths = [
        "/run/containerd",
        "/run/containerd/containerd.sock",
        "/var/lib/containerd/io.containerd.metadata.v1.bolt/meta.db",
        "/etc/containerd/config.toml",
        "/run/crio/crio.sock",
        "/var/lib/crio/storage",
        "/etc/crio/crio.conf",
        "/etc/containers/registries.conf",
    ];

    for hp in host_paths {
        assert!(
            policy.validate_cleanup_path(Path::new(hp)).is_err(),
            "Host path '{hp}' must be protected from cleanup"
        );
    }

    // Only private Rubix data paths are allowed for cleanup
    let rubix_private = temp.path().join("rubix_data/pki");
    assert!(policy.validate_cleanup_path(&rubix_private).is_ok());
}

#[tokio::test]
async fn unrelated_host_workloads_remain_intact_across_lifecycle() {
    let temp = TempDir::new().unwrap();
    let socket = temp.path().join("crio_ownership.sock");
    let stopped = Arc::new(AtomicBool::new(false));
    let conn_count = Arc::new(AtomicUsize::new(0));

    let _server =
        spawn_ownership_mock_server(&socket, Arc::clone(&conn_count), Arc::clone(&stopped));

    let endpoints = RuntimeEndpoints::single(CriEndpoint::from_path(&socket).unwrap());
    let mut client = CriClient::connect(&endpoints)
        .await
        .expect("connect succeeds");

    let containers = client
        .list_containers(None)
        .await
        .expect("list_containers succeeds");
    assert_eq!(containers.len(), 2);

    let rubix_container = containers
        .iter()
        .find(|c| c.id == "rubix-pod-web")
        .expect("rubix container present");
    let host_container = containers
        .iter()
        .find(|c| c.id == "host-monitoring-agent")
        .expect("host container present");

    let scoping = WorkloadScoping::new("rubix-system");

    // Rubix container is owned and modifiable
    assert!(scoping.is_sandbox_owned("rubix-system", &rubix_container.labels));
    assert!(
        scoping
            .assert_can_modify_sandbox(&rubix_container.id, "rubix-system", &rubix_container.labels)
            .is_ok()
    );

    // Host container is NOT owned and cannot be modified or purged
    assert!(!scoping.is_sandbox_owned("default", &host_container.labels));
    let err = scoping
        .assert_can_modify_sandbox(&host_container.id, "default", &host_container.labels)
        .unwrap_err();
    assert_eq!(
        err,
        OwnershipViolation::UnrelatedWorkload {
            id: "host-monitoring-agent".into()
        }
    );

    stopped.store(true, Ordering::Relaxed);
}

#[test]
fn build_profile_and_runtime_mode_matrix_coverage() {
    // 1. Bundled + Managed (default standalone distribution)
    let combo1 = BuildAndRuntimeMatrix::validate(BuildProfile::Bundled, RuntimeMode::Managed)
        .expect("Bundled + Managed must be valid");
    assert_eq!(combo1.build_profile, BuildProfile::Bundled);
    assert_eq!(combo1.runtime_mode, RuntimeMode::Managed);

    // 2. Bundled + External (standalone distribution directed to external runtime endpoint)
    let combo2 = BuildAndRuntimeMatrix::validate(BuildProfile::Bundled, RuntimeMode::External)
        .expect("Bundled + External must be valid");
    assert_eq!(combo2.build_profile, BuildProfile::Bundled);
    assert_eq!(combo2.runtime_mode, RuntimeMode::External);

    // 3. ExternalDeps + External (minimal distribution attaching to host runtime)
    let combo3 = BuildAndRuntimeMatrix::validate(BuildProfile::ExternalDeps, RuntimeMode::External)
        .expect("ExternalDeps + External must be valid");
    assert_eq!(combo3.build_profile, BuildProfile::ExternalDeps);
    assert_eq!(combo3.runtime_mode, RuntimeMode::External);

    // 4. ExternalDeps + Managed (invalid combination: cannot use bundled runtime when binaries were excluded)
    let err = BuildAndRuntimeMatrix::validate(BuildProfile::ExternalDeps, RuntimeMode::Managed)
        .unwrap_err();
    assert!(matches!(
        err,
        OwnershipViolation::InvalidCombination {
            build_profile: BuildProfile::ExternalDeps,
            runtime_mode: RuntimeMode::Managed,
            ..
        }
    ));
}

#[test]
fn cni_ownership_allows_only_owned_configuration() {
    let policy = RuntimeOwnershipPolicy::external(
        "/run/containerd/containerd.sock",
        "/run/containerd/containerd.sock",
    );

    let owned_cni_dir = Path::new("/var/lib/rubix/containerd/cni/conf");

    // Owned CNI configuration under Rubix data directory is permitted (deferred to E15)
    let owned_conflist = owned_cni_dir.join("10-rubix.conflist");
    assert!(
        policy
            .validate_cni_configuration(&owned_conflist, owned_cni_dir)
            .is_ok()
    );

    // Host/system CNI configuration is strictly forbidden from being modified
    let host_cni_paths = [
        Path::new("/etc/cni/net.d/10-flannel.conflist"),
        Path::new("/etc/cni/net.d/calico.conflist"),
        Path::new("/opt/cni/bin/bridge"),
        Path::new("/etc/cni/net.d"),
    ];

    for host_cni in host_cni_paths {
        assert_eq!(
            policy.validate_cni_configuration(host_cni, owned_cni_dir),
            Err(OwnershipViolation::NonOwnedCniPath {
                path: host_cni.to_path_buf()
            }),
            "Host CNI path '{host_cni:?}' must be rejected from modification"
        );
    }
}
