use poolster_core::input::InputPlugin;
use poolster_input_asyncapi::{AsyncApiDocument, AsyncApiInput, AsyncApiModel, parse};
use serde_json::{Value, json};
const EXAMPLE: &str = include_str!("fixtures/events.yaml");
fn fixture() -> Value {
    serde_yaml_ng::from_str(EXAMPLE).unwrap()
}

#[test]
fn yaml_and_json_have_identical_summaries_and_sources() {
    let json = fixture().to_string();
    let yaml = parse(EXAMPLE).unwrap();
    let document = parse(&json).unwrap();
    assert_eq!(document.summary(), yaml.summary());
    assert_eq!(document.source, yaml.source);
}
#[test]
fn operation_aliases_preserve_receive_action() {
    let mut value = fixture();
    value["components"]["operations"] =
        json!({"receive":{"action":"receive","channel":{"$ref":"#/channels/orders"}}});
    value["operations"] = json!({"read":{"$ref":"#/components/operations/receive"}});
    let document = parse(&value.to_string()).unwrap();
    assert_eq!(document.summary().operations[0].name, "read");
    assert_eq!(document.summary().operations[0].kind, "receive");
}
#[test]
fn channel_reference_supports_valid_punctuation() {
    let mut value = fixture();
    let channel = value["channels"]["orders"].take();
    value["channels"] = json!({"orders.created-event_v1":channel});
    value["operations"]["emitCreated"]["channel"]["$ref"] =
        json!("#/channels/orders.created-event_v1");
    parse(&value.to_string()).unwrap();
}
#[test]
fn missing_and_cyclic_references_fail() {
    let mut value = fixture();
    value["operations"]["emitCreated"]["channel"]["$ref"] = json!("#/channels/missing");
    assert!(parse(&value.to_string()).is_err());
    value["channels"] = json!({"a":{"$ref":"#/channels/b"},"b":{"$ref":"#/channels/a"}});
    value["operations"] = json!({});
    assert!(parse(&value.to_string()).is_err());
}
#[test]
fn v26_publish_and_subscribe_keep_names_and_direction() {
    let value = json!({"asyncapi":"2.6.0","info":{"title":"Events","version":"1"},"channels":{"orders":{"publish":{"operationId":"publishOrders","message":{"payload":{"type":"string"}}},"subscribe":{"message":{"payload":{"type":"string"}}}}}});
    let document = parse(&value.to_string()).unwrap();
    assert!(matches!(document.model, AsyncApiModel::V2_6(_)));
    let summary = document.summary();
    assert_eq!(summary.operations.len(), 2);
    assert!(
        summary
            .operations
            .iter()
            .any(|op| op.name == "publishOrders" && op.kind == "publish")
    );
    assert!(
        summary
            .operations
            .iter()
            .any(|op| op.name == "orders:subscribe" && op.kind == "subscribe")
    );
}
#[test]
fn extensions_and_arbitrary_protocol_bindings_are_retained() {
    let mut value = fixture();
    value["x-team"] = json!({"owners":["platform"]});
    value["channels"]["orders"]["bindings"]["kafka"]["x-custom"] = json!({"retention":7});
    let document = parse(&value.to_string()).unwrap();
    assert_eq!(document.source, value);
    if let AsyncApiModel::V3_0(model) = document.model {
        assert_eq!(
            serde_json::to_value(model).unwrap()["x-team"],
            value["x-team"]
        );
    } else {
        panic!("wrong model")
    }
}
#[test]
fn malformed_required_fields_and_versions_fail() {
    for source in [
        "{",
        "[]",
        "asyncapi: 3",
        "asyncapi: 3.1.1",
        "asyncapi: 3.0.0\ninfo: {title: Events}",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
    let mut value = fixture();
    value["operations"]["emitCreated"]["action"] = json!("publish");
    assert!(parse(&value.to_string()).is_err());
}
#[test]
fn provider_loads_native_contract_and_reports_file_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("events.yaml");
    std::fs::write(&path, EXAMPLE).unwrap();
    let provider = AsyncApiInput;
    assert_eq!(provider.id(), "asyncapi.roas");
    assert_eq!(provider.format(), "asyncapi");
    let loaded = provider.load(&path).unwrap();
    assert_eq!(
        loaded.get::<AsyncApiDocument>().unwrap().summary(),
        loaded.summary
    );
    assert!(loaded.diagnostics.is_empty());
    assert!(
        format!(
            "{:#}",
            provider
                .load(&directory.path().join("missing.yaml"))
                .err()
                .unwrap()
        )
        .contains("cannot read contract")
    );
    std::fs::write(&path, "invalid").unwrap();
    assert!(provider.load(&path).is_err());
    std::fs::write(&path, [0xff, 0xfe]).unwrap();
    assert!(provider.load(&path).is_err());
}
