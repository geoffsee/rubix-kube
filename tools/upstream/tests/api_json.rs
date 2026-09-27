//! Consume independent official API-server observations with the selected published bindings.

use k8s_openapi::api::core::v1::{ConfigMap, Namespace, Pod, Service};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::{
    CustomResourceDefinition, JSON,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::WatchEvent;
use k8s_openapi::apimachinery::pkg::runtime::RawExtension;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

fn input_document(name: &str, frozen: &str) -> Value {
    if let Some(directory) = std::env::var_os("RUBIX_API_JSON_CAPTURE_DIR") {
        let path = std::path::PathBuf::from(directory).join(name);
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "explicit RUBIX_API_JSON_CAPTURE_DIR input {}: {error}",
                path.display()
            )
        });
        serde_json::from_str(&raw).unwrap_or_else(|error| {
            panic!(
                "invalid explicit API capture JSON {}: {error}",
                path.display()
            )
        })
    } else {
        serde_json::from_str(frozen).expect("valid frozen API capture JSON")
    }
}

fn fixtures() -> Value {
    input_document(
        "fixtures.json",
        include_str!("../../api-json/fixtures.json"),
    )
}

fn capture() -> Value {
    input_document(
        "result.json",
        include_str!("../../api-json/evidence/result.json"),
    )
}

fn observation(capture: &Value, name: &str) -> Value {
    let record = capture["http"]
        .as_array()
        .expect("HTTP observation array")
        .iter()
        .find(|record| record["name"] == name)
        .expect("named actual HTTP observation");
    // Parse the server's raw response, rather than trusting the redundant parsed record.
    serde_json::from_str(record["raw_response"].as_str().expect("raw JSON response"))
        .expect("server emitted valid JSON")
}

fn round_trip<T: DeserializeOwned + Serialize>(value: &Value) -> T {
    let decoded: T = serde_json::from_value(value.clone()).expect("published type decodes fixture");
    assert_eq!(
        serde_json::to_value(&decoded).expect("published type encodes fixture"),
        *value,
        "complete captured JSON must survive typed serialization"
    );
    decoded
}

#[test]
fn namespace_and_established_crd_preserve_complete_actual_server_documents() {
    let raw = capture();
    let namespace: Namespace = round_trip(&observation(&raw, "namespace"));
    assert_eq!(
        namespace.metadata.name.as_deref(),
        Some("serialization-fixture")
    );
    assert!(namespace.metadata.uid.is_some());
    assert!(namespace.metadata.resource_version.is_some());
    assert!(namespace.metadata.creation_timestamp.is_some());

    let definition: CustomResourceDefinition = round_trip(&observation(&raw, "crd-ready"));
    assert_eq!(definition.spec.group, "fixture.rubix.test");
    assert_eq!(definition.spec.scope, "Namespaced");
    assert_eq!(definition.spec.names.kind, "Sample");
    let version = &definition.spec.versions[0];
    assert_eq!(version.name, "v1");
    assert!(version.served && version.storage);
    let properties = version
        .schema
        .as_ref()
        .and_then(|schema| schema.open_api_v3_schema.as_ref())
        .and_then(|schema| schema.properties.as_ref())
        .expect("captured CRD structural schema");
    assert_eq!(
        properties["spec"].x_kubernetes_preserve_unknown_fields,
        Some(true)
    );
}

#[test]
fn initial_crd_null_conditions_are_omitted_by_typed_serialization() {
    let raw = observation(&capture(), "crd-create");
    assert!(
        raw["status"]
            .as_object()
            .expect("CRD status")
            .contains_key("conditions")
    );
    assert!(raw["status"]["conditions"].is_null());
    let typed: CustomResourceDefinition = serde_json::from_value(raw.clone()).expect("initial CRD");
    assert!(
        typed
            .status
            .as_ref()
            .expect("CRD status")
            .conditions
            .is_none()
    );
    let encoded = serde_json::to_value(typed).expect("serialize initial CRD");
    assert!(encoded["status"].get("conditions").is_none());
    let mut expected_typed = raw.clone();
    expected_typed["status"]
        .as_object_mut()
        .expect("status")
        .remove("conditions");
    assert_eq!(
        encoded, expected_typed,
        "only this observed null is omitted"
    );
    let dynamic: RawExtension = round_trip(&raw);
    assert!(dynamic.0["status"]["conditions"].is_null());
}

#[test]
fn pod_preserves_server_canonical_quantities_metadata_and_optional_fields() {
    let observed = fixtures();
    for name in ["pod-create", "pod-read"] {
        let pod: Pod = round_trip(&observed["cases"][name]);
        assert_eq!(
            pod.metadata.labels.as_ref().expect("labels")["fixture"],
            "json"
        );
        assert_eq!(
            pod.metadata.annotations.as_ref().expect("annotations")["example.test/text"],
            "literal: null"
        );
        assert!(pod.metadata.finalizers.is_none());
        assert!(pod.metadata.owner_references.is_none());
        let spec = pod.spec.expect("captured Pod spec");
        assert_eq!(spec.automount_service_account_token, Some(false));
        assert!(spec.node_selector.is_none());
        assert!(spec.volumes.is_none());
        let resources = spec.containers[0].resources.as_ref().expect("resources");
        let requests = resources.requests.as_ref().expect("requests");
        assert_eq!(requests["cpu"], Quantity("500m".into()));
        assert_eq!(requests["memory"], Quantity("1536Mi".into()));
        assert_eq!(requests["ephemeral-storage"], Quantity("1e3".into()));
        assert_eq!(
            resources.limits.as_ref().expect("limits")["cpu"],
            Quantity("1".into())
        );
    }
    // Quantity is a string carrier here. API canonicalization is independently observed;
    // the Rust newtype must not be mistaken for a Go-compatible quantity parser/default engine.
    let input: Quantity = serde_json::from_value(json!("0.5")).expect("quantity string");
    assert_eq!(
        serde_json::to_value(input).expect("serialize quantity"),
        json!("0.5")
    );
}

#[test]
fn explicit_null_and_omitted_optional_input_both_decode_as_absent() {
    let raw = capture();
    let request = raw["http"]
        .as_array()
        .expect("HTTP observations")
        .iter()
        .find(|record| record["name"] == "pod-create")
        .expect("Pod request")["request"]
        .clone();
    assert!(
        request["metadata"]
            .as_object()
            .expect("metadata")
            .contains_key("finalizers")
    );
    assert!(request["metadata"]["finalizers"].is_null());
    assert!(request["spec"]["nodeSelector"].is_null());
    let explicit: Pod = serde_json::from_value(request.clone()).expect("explicit null input");
    let mut omitted = request;
    omitted["metadata"]
        .as_object_mut()
        .expect("metadata")
        .remove("finalizers");
    omitted["spec"]
        .as_object_mut()
        .expect("spec")
        .remove("nodeSelector");
    let absent: Pod = serde_json::from_value(omitted.clone()).expect("omitted input");
    assert_eq!(explicit, absent);
    assert_eq!(
        serde_json::to_value(explicit).expect("serialize input"),
        omitted
    );
}

#[test]
fn service_retains_named_and_integer_target_port_variants() {
    let observed = fixtures();
    for name in ["service-create", "service-read"] {
        let service: Service = round_trip(&observed["cases"][name]);
        assert!(service.metadata.annotations.is_none());
        let spec = service.spec.expect("Service spec");
        assert_eq!(spec.cluster_ip.as_deref(), Some("None"));
        let ports = spec.ports.expect("Service ports");
        assert_eq!(
            ports[0].target_port,
            Some(IntOrString::String("http".into()))
        );
        assert_eq!(ports[1].target_port, Some(IntOrString::Int(8080)));
    }
    assert!(serde_json::from_value::<IntOrString>(json!(false)).is_err());
    assert!(serde_json::from_value::<IntOrString>(json!(8080.5)).is_err());
    assert_eq!(
        serde_json::from_value::<IntOrString>(json!("8080")).expect("numeric string stays string"),
        IntOrString::String("8080".into())
    );
}

#[test]
fn dynamic_custom_resource_preserves_arbitrary_json_without_typed_field_loss() {
    let observed = fixtures();
    let expected = json!({"metadata":{"uid":"user-value"},"null":null,"bool":false,
        "integer":17,"decimal":1.25,"list":[null,true,"7",7,{"nested":"value"}]});
    for name in ["custom-create", "custom-read"] {
        let value = &observed["cases"][name];
        let dynamic: RawExtension = round_trip(value);
        assert_eq!(dynamic.0["apiVersion"], "fixture.rubix.test/v1");
        assert_eq!(dynamic.0["kind"], "Sample");
        assert_eq!(dynamic.0["spec"]["unknown"], expected);
        let arbitrary: JSON = round_trip(&value["spec"]);
        assert_eq!(arbitrary.0["unknown"], expected);
        assert!(arbitrary.0["unknown"]["bool"].is_boolean());
        assert!(arbitrary.0["unknown"]["integer"].is_i64());
        assert!(arbitrary.0["unknown"]["decimal"].is_f64());
    }
}

#[test]
fn typed_config_map_watch_events_keep_envelopes_and_deleted_final_value() {
    let observed = fixtures();
    let events = observed["watch"].as_array().expect("watch events");
    assert_eq!(events.len(), 3);
    for (index, value) in events.iter().enumerate() {
        let event: WatchEvent<ConfigMap> = round_trip(value);
        let ((0, WatchEvent::Added(object))
        | (1, WatchEvent::Modified(object))
        | (2, WatchEvent::Deleted(object))) = (index, event)
        else {
            panic!("capture must retain ADDED, MODIFIED, DELETED variants");
        };
        assert_eq!(object.metadata.name.as_deref(), Some("watched"));
        assert_eq!(
            object.data.expect("ConfigMap data")["value"],
            if index == 0 { "first" } else { "second" }
        );
    }
    let mut unknown = events[0].clone();
    unknown["type"] = json!("UNRECOGNIZED");
    assert!(serde_json::from_value::<WatchEvent<ConfigMap>>(unknown).is_err());
}

#[test]
fn typed_builtin_objects_do_not_claim_universal_unknown_field_preservation() {
    let mut extended = fixtures()["cases"]["pod-read"].clone();
    extended["futureUnknownField"] = json!({"retainedOnlyInDynamicPath":true});
    let typed: Pod = serde_json::from_value(extended.clone()).expect("unknown fields accepted");
    let serialized = serde_json::to_value(typed).expect("typed serialization");
    assert!(serialized.get("futureUnknownField").is_none());
    let dynamic: RawExtension = round_trip(&extended);
    assert_eq!(
        dynamic.0["futureUnknownField"]["retainedOnlyInDynamicPath"],
        true
    );
}
