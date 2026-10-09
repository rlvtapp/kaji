use poolster_core::{
    engine::Contract,
    input::{InputPlugin, InputRegistry},
};
use poolster_input_asyncapi::{
    AsyncApiDocument, AsyncApiInput,
    blocks::{MessageBlocks, lower_message_blocks},
    contracts::{EventMessage, EventOperations},
};
#[test]
fn public_modules_preserve_contract_identities_and_registry_publication() {
    assert_eq!(
        EventOperations::NAME,
        <poolster_core::native::events::EventOperations as Contract>::NAME
    );
    assert_eq!(
        MessageBlocks::NAME,
        <poolster_core::blocks::Blocks<EventMessage> as Contract>::NAME
    );
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), include_str!("fixtures/kafka-json.yaml")).unwrap();
    let mut registry = InputRegistry::new();
    registry.register(AsyncApiInput).unwrap();
    let loaded = registry.load("asyncapi", None, file.path()).unwrap();
    assert_eq!(
        loaded
            .contract
            .get::<EventOperations>()
            .unwrap()
            .operations
            .len(),
        2
    );
    let blocks = loaded.contract.get::<MessageBlocks>().unwrap();
    assert_eq!(blocks.items.len(), 1);
    assert!(blocks.items[0].metadata.id.local.starts_with("#/channels/"));
    let whole = loaded.contract.get::<AsyncApiDocument>().unwrap();
    let (extracted, diagnostics) = lower_message_blocks(whole, file.path().to_string_lossy());
    assert!(diagnostics.is_empty());
    assert_eq!(&extracted, blocks);
}
#[test]
fn collection_api_needs_no_broker_or_whole_event_contract() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), include_str!("fixtures/events.yaml")).unwrap();
    let loaded = AsyncApiInput.load(file.path()).unwrap();
    let blocks: &poolster_input_asyncapi::contracts::MessageBlocks = loaded.get().unwrap();
    assert_eq!(blocks.with_capability("messaging.model").count(), 1);
    assert!(loaded.get::<EventOperations>().is_err());
}
