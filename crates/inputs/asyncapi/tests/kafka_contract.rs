use poolster_core::{
    input::{InputOptions, InputPlugin},
    native::events::{EventAction, EventOperations},
};
use poolster_input_asyncapi::{AsyncApiInput, lower_operations, parse};
const SOURCE: &str = include_str!("fixtures/kafka-json.yaml");
#[test]
fn kafka_lowering_preserves_presence_json_constraints_key_headers_and_bindings() {
    let doc = parse(SOURCE).unwrap();
    let contract = lower_operations(&doc, &InputOptions::default()).unwrap();
    assert_eq!(contract.broker.brokers, ["127.0.0.1:29092"]);
    let receive = contract
        .operations
        .iter()
        .find(|o| o.action == EventAction::Receive)
        .unwrap();
    assert_eq!(receive.group_id.as_deref(), Some("poolster-alpha2-orders"));
    assert_eq!(
        receive.message.payload_schema["properties"]["quantity"]["minimum"],
        1
    );
    assert!(receive.message.key.is_some());
    assert!(receive.message.headers.is_some());
    assert_eq!(receive.channel_bindings["topic"], "poolster-orders");
    assert_eq!(contract.source, doc.source);
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), SOURCE).unwrap();
    let loaded = AsyncApiInput
        .load_with_options(file.path(), &InputOptions::default())
        .unwrap();
    assert_eq!(loaded.get::<EventOperations>().unwrap().operations.len(), 2);
}
#[test]
fn kafka_rejects_unsupported_generation_without_breaking_inspection() {
    let upstream = parse(include_str!("corpus/adeo-kafka-3.1.yaml")).unwrap();
    assert!(lower_operations(&upstream, &InputOptions::default()).is_err());
    for source in [
        SOURCE.replace("protocol: kafka", "protocol: amqp"),
        SOURCE.replace("address: poolster-orders", "address: '{topic}'"),
        SOURCE.replace("minimum: 1", "format: int32"),
        SOURCE.replace("bindingVersion: '0.5.0'", "bindingVersion: '9.9.9'"),
    ] {
        let result =
            parse(&source).and_then(|doc| lower_operations(&doc, &InputOptions::default()));
        assert!(result.is_err());
    }
}
