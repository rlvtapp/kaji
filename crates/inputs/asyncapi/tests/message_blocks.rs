use poolster_core::{
    blocks::Blocks,
    input::{InputPlugin, InputRegistry},
    native::events::{EventMessage, EventOperations},
};
use poolster_input_asyncapi::{AsyncApiDocument, AsyncApiInput};
use serde_json::json;
#[test]
fn registry_publishes_message_blocks_without_a_broker() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("events.yaml");
    std::fs::write(&path, include_str!("fixtures/events.yaml")).unwrap();
    let mut registry = InputRegistry::new();
    registry.register(AsyncApiInput).unwrap();
    let input = registry.load("asyncapi", None, &path).unwrap();
    let blocks = input.contract.get::<Blocks<EventMessage>>().unwrap();
    assert_eq!(blocks.items.len(), 1);
    assert_eq!(
        blocks.items[0].metadata.id.local,
        "#/channels/orders/messages/created"
    );
    assert_eq!(blocks.items[0].metadata.id.source, path.to_string_lossy());
    assert!(input.contract.get::<AsyncApiDocument>().is_ok());
    assert!(input.contract.get::<EventOperations>().is_err());
    assert!(
        input
            .contract
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "asyncapi-generation-unsupported")
    );
    let again = registry.load("asyncapi", None, &path).unwrap();
    assert_eq!(
        blocks,
        again.contract.get::<Blocks<EventMessage>>().unwrap()
    );
}
#[test]
fn directly_loaded_supported_broker_retains_whole_and_blocks_with_shared_ids() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("kafka.yaml");
    std::fs::write(&path, include_str!("fixtures/kafka-json.yaml")).unwrap();
    let input = AsyncApiInput.load(&path).unwrap();
    let blocks = input.get::<Blocks<EventMessage>>().unwrap();
    let events = input.get::<EventOperations>().unwrap();
    assert_eq!(blocks.items.len(), 1);
    assert_eq!(events.operations.len(), 2);
    blocks
        .require_parent(input.get_reference::<AsyncApiDocument>().unwrap().unwrap())
        .unwrap();
    assert_eq!(
        blocks.items[0].value.location,
        events.operations[0].message.location
    );
    assert_eq!(
        blocks.items[0].value.bindings["kafka"]["bindingVersion"],
        "0.5.0"
    );
    assert!(input.get::<AsyncApiDocument>().is_ok());
}
#[test]
fn unsupported_message_schema_reports_location_without_hiding_other_messages() {
    let mut value: serde_json::Value =
        serde_yaml_ng::from_str(include_str!("fixtures/events.yaml")).unwrap();
    value["channels"]["orders"]["messages"]["unsupported"] =
        json!({"payload":{"type":"string","format":"uuid"}});
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), value.to_string()).unwrap();
    let input = AsyncApiInput.load(file.path()).unwrap();
    assert_eq!(input.get::<Blocks<EventMessage>>().unwrap().items.len(), 1);
    assert!(
        matches!(&input.get::<Blocks<EventMessage>>().unwrap().state, poolster_core::blocks::CollectionState::Partial {diagnostics} if diagnostics == &vec![input.diagnostics[0].message.clone()])
    );
    assert_eq!(input.diagnostics.len(), 1);
    assert_eq!(
        input.diagnostics[0].code,
        "asyncapi-message-block-unsupported"
    );
    assert!(
        input.diagnostics[0]
            .message
            .contains("#/channels/orders/messages/unsupported")
    );
    assert!(input.get::<AsyncApiDocument>().is_ok());
}
#[test]
fn v26_model_blocks_preserve_native_direction_without_claiming_kafka_generation() {
    let value = json!({"asyncapi":"2.6.0","info":{"title":"Legacy","version":"1"},"channels":{"orders":{"publish":{"message":{"$ref":"#/components/messages/Created"}},"subscribe":{"message":{"$ref":"#/components/messages/Created"}}}},"components":{"messages":{"Created":{"payload":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false}}}}});
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), value.to_string()).unwrap();
    let input = AsyncApiInput.load(file.path()).unwrap();
    let blocks = input.get::<Blocks<EventMessage>>().unwrap();
    assert_eq!(blocks.items.len(), 1);
    assert_eq!(
        blocks.items[0].metadata.id.local,
        "#/components/messages/Created"
    );
    assert!(input.get::<EventOperations>().is_err());
    assert_eq!(
        input
            .get::<AsyncApiDocument>()
            .unwrap()
            .summary()
            .operations
            .len(),
        2
    );
}
