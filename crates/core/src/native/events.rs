//! Broker-aware event contracts owned by Poolster; no parser implementation types.
use crate::{SchemaValue, engine::Contract};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KafkaBroker {
    pub brokers: Vec<String>,
    pub client_id: Option<String>,
    /// Retained native server binding details.
    pub bindings: Value,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventAction {
    Send,
    Receive,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventMessage {
    pub name: String,
    /// Canonical resolved native JSON pointer, independent of generated names.
    pub location: String,
    pub payload: SchemaValue,
    /// Resolved JSON Schema used by runtime validation, including constraints.
    pub payload_schema: Value,
    pub headers: Option<SchemaValue>,
    pub headers_schema: Option<Value>,
    pub key: Option<SchemaValue>,
    pub key_schema: Option<Value>,
    pub bindings: Value,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventOperation {
    pub name: String,
    pub action: EventAction,
    pub channel: String,
    pub topic: String,
    pub message: EventMessage,
    pub group_id: Option<String>,
    pub client_id: Option<String>,
    pub bindings: Value,
    pub channel_bindings: Value,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventOperations {
    /// Full native source retained for semantics outside this versioned contract.
    pub source: Value,
    pub broker: KafkaBroker,
    pub operations: Vec<EventOperation>,
}
impl Contract for EventOperations {
    const NAME: &'static str = "poolster.event-operations.v1";
}

impl crate::blocks::Block for EventMessage {
    const CONTRACT_NAME: &'static str = "poolster.event-message-blocks.v1";
}
impl EventOperations {
    /// Opt-in decomposition retains the authoritative whole event contract.
    pub fn message_blocks(&self, source: impl Into<String>) -> crate::blocks::Blocks<EventMessage> {
        use crate::blocks::{BlockId, BlockMetadata, Blocks, BuildingBlock};
        use std::collections::{BTreeMap, BTreeSet};
        let source = source.into();
        let mut items = BTreeMap::new();
        for op in &self.operations {
            let message = &op.message;
            let mut capabilities = BTreeSet::from([
                "messaging.json".into(),
                "broker.kafka".into(),
                "schema.json-schema.draft-07".into(),
            ]);
            if message.headers.is_some() {
                capabilities.insert("messaging.headers".into());
            }
            if message.key.is_some() {
                capabilities.insert("messaging.key".into());
            }
            items
                .entry(message.location.clone())
                .or_insert_with(|| BuildingBlock {
                    metadata: BlockMetadata {
                        parent: Some(crate::blocks::ContractReference::from_bytes(
                            <Self as crate::engine::Contract>::NAME,
                            source.clone(),
                            &serde_json::to_vec(self).expect("owned contract is serializable"),
                        )),
                        id: BlockId {
                            source: source.clone(),
                            local: message.location.clone(),
                        },
                        capabilities,
                        references: vec![],
                        location: Some(message.location.clone()),
                    },
                    value: message.clone(),
                });
        }
        Blocks {
            parent: Some(crate::blocks::ContractReference::from_bytes(
                <Self as crate::engine::Contract>::NAME,
                source.clone(),
                &serde_json::to_vec(self).expect("owned contract is serializable"),
            )),
            state: crate::blocks::CollectionState::Complete,
            items: items.into_values().collect(),
        }
    }
}
