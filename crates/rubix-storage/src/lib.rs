#![forbid(unsafe_code)]

//! Kubernetes local-path storage provisioner resources and configuration for KubeSolo/Rubix.
//!
//! Provides types, manifest generators, and configuration models for deploying
//! the `rancher.io/local-path` persistent storage provisioner, including RBAC,
//! `ConfigMap` resources, helper scripts, `Deployment`, and default `StorageClass`.

pub mod config;
pub mod error;
pub mod manifests;

pub use config::{
    DEFAULT_APP_LABEL_KEY, DEFAULT_APP_LABEL_VALUE, DEFAULT_BASE_STORAGE_PATH,
    DEFAULT_CONFIG_FILE_PATH, DEFAULT_CONFIG_MOUNT_PATH, DEFAULT_CPU_REQUEST, DEFAULT_HELPER_IMAGE,
    DEFAULT_MEMORY_LIMIT, DEFAULT_MEMORY_REQUEST, DEFAULT_NODE_PATH_KEY, DEFAULT_PROVISIONER_IMAGE,
    DEFAULT_RECLAIM_POLICY, DEFAULT_SETUP_SCRIPT, DEFAULT_STORAGE_DIR_NAME,
    DEFAULT_TEARDOWN_SCRIPT, DEFAULT_VOLUME_BINDING_MODE, LOCAL_PATH_CLUSTER_ROLE_BINDING_NAME,
    LOCAL_PATH_CLUSTER_ROLE_NAME, LOCAL_PATH_CONFIGMAP_NAME, LOCAL_PATH_DEPLOYMENT_NAME,
    LOCAL_PATH_NAMESPACE, LOCAL_PATH_PROVISIONER_NAME, LOCAL_PATH_ROLE_BINDING_NAME,
    LOCAL_PATH_ROLE_NAME, LOCAL_PATH_SERVICE_ACCOUNT_NAME, LOCAL_PATH_STORAGE_CLASS_NAME,
    LocalPathConfig,
};
pub use error::StorageError;
pub use manifests::{
    LocalPathManifests, generate_cluster_role, generate_cluster_role_binding, generate_config_json,
    generate_config_map, generate_deployment, generate_helper_pod_yaml, generate_namespace,
    generate_role, generate_role_binding, generate_service_account, generate_storage_class,
    provisioner_labels,
};
