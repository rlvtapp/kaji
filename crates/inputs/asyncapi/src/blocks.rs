//! Direct message collections and optional explicit decomposition for plugin authors.
//! Neither message models nor block extraction require a broker or routing contract.
pub use crate::contracts::MessageBlocks;
use crate::contracts::{EventMessage, EventOperations};
use anyhow::{Result, ensure};
pub use poolster_core::blocks::{
    Block, BlockId, BlockMetadata, BlockReference, Blocks, BuildingBlock,
};
use poolster_core::engine::{
    Handle, Language, Meta, Plugin, PluginContext, Provision, Requirement,
};
pub struct EventMessageBlocks {
    meta: Meta,
    provider: Option<Handle<EventOperations>>,
    source: String,
}
pub fn event_messages(
    provider: Option<Handle<EventOperations>>,
    source: impl Into<String>,
) -> EventMessageBlocks {
    EventMessageBlocks {
        meta: Meta::new(),
        provider,
        source: source.into(),
    }
}
impl EventMessageBlocks {
    pub fn handle(&self) -> Handle<MessageBlocks> {
        self.meta.handle()
    }
}
impl<L: Language> Plugin<L> for EventMessageBlocks {
    fn kind(&self) -> &'static str {
        "asyncapi-event-message-blocks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.provider)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Blocks<EventMessage>>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()> {
        ensure!(
            !self.source.trim().is_empty(),
            "event block source identity must not be empty"
        );
        let blocks = cx
            .inputs
            .get::<EventOperations>()?
            .message_blocks(&self.source);
        cx.publish(blocks)
    }
}

/// Lower independently useful message models without requiring routes or a broker.
/// Unsupported models become diagnostics while supported messages remain available.
pub fn lower_message_blocks(
    document: &crate::AsyncApiDocument,
    source: impl Into<String>,
) -> (MessageBlocks, Vec<poolster_core::input::InputDiagnostic>) {
    use serde_json::Value;
    use std::collections::{BTreeMap, BTreeSet};
    let source = source.into();
    let root = &document.source;
    let mut candidates = Vec::<(String, &Value)>::new();
    if let Some(messages) = root["components"]["messages"].as_object() {
        candidates.extend(messages.iter().map(|(name, value)| {
            (
                format!(
                    "#/components/messages/{}",
                    crate::lowering::pointer_segment(name)
                ),
                value,
            )
        }));
    }
    if let Some(channels) = root["channels"].as_object() {
        for (name, raw) in channels {
            let pointer = format!("#/channels/{}", crate::lowering::pointer_segment(name));
            if let Ok(channel) = crate::lowering::resolve(root, raw) {
                let channel_pointer =
                    crate::lowering::location(root, raw, &pointer).unwrap_or(pointer);
                if let Some(messages) = channel["messages"].as_object() {
                    candidates.extend(messages.iter().map(|(name, value)| {
                        (
                            format!(
                                "{channel_pointer}/messages/{}",
                                crate::lowering::pointer_segment(name)
                            ),
                            value,
                        )
                    }));
                }
                for action in ["publish", "subscribe"] {
                    if let Some(op) = channel.get(action) {
                        if let Ok(op) = crate::lowering::resolve(root, op) {
                            if let Some(message) = op.get("message") {
                                if let Some(variants) = message["oneOf"].as_array() {
                                    candidates
                                        .extend(variants.iter().enumerate().map(|(index, value)| {
                                        (
                                            format!(
                                                "{channel_pointer}/{action}/message/oneOf/{index}"
                                            ),
                                            value,
                                        )
                                    }));
                                } else {
                                    candidates.push((
                                        format!("{channel_pointer}/{action}/message"),
                                        message,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let mut blocks = BTreeMap::new();
    let mut diagnostics = Vec::new();
    let mut seen = BTreeSet::new();
    for (pointer, value) in candidates {
        let location = match crate::lowering::location(root, value, &pointer) {
            Ok(location) => location,
            Err(error) => {
                diagnostics.push(block_diagnostic(&pointer, error));
                continue;
            }
        };
        if !seen.insert(location.clone()) {
            continue;
        }
        let result = (|| -> Result<EventMessage> {
            let message = crate::lowering::resolve(root, value)?;
            let (payload, payload_schema) = crate::lowering::schema(
                root,
                message
                    .get("payload")
                    .ok_or_else(|| anyhow::anyhow!("message payload missing"))?,
                &mut BTreeSet::new(),
            )?;
            let headers = message
                .get("headers")
                .map(|value| crate::lowering::schema(root, value, &mut BTreeSet::new()))
                .transpose()?;
            let key = message["bindings"]["kafka"]
                .get("key")
                .map(|value| crate::lowering::schema(root, value, &mut BTreeSet::new()))
                .transpose()?;
            Ok(EventMessage {
                name: message["name"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        location
                            .rsplit('/')
                            .next()
                            .unwrap_or("message")
                            .replace("~1", "/")
                            .replace("~0", "~")
                    }),
                location: location.clone(),
                payload,
                payload_schema,
                headers: headers.as_ref().map(|value| value.0.clone()),
                headers_schema: headers.map(|value| value.1),
                key: key.as_ref().map(|value| value.0.clone()),
                key_schema: key.map(|value| value.1),
                bindings: message
                    .get("bindings")
                    .cloned()
                    .unwrap_or(serde_json::json!({})),
            })
        })();
        match result {
            Ok(message) => {
                let mut capabilities = BTreeSet::from([
                    "messaging.model".into(),
                    "schema.json-schema.draft-07".into(),
                ]);
                if message.bindings.get("kafka").is_some() {
                    capabilities.insert("broker.kafka".into());
                }
                if message.headers.is_some() {
                    capabilities.insert("messaging.headers".into());
                }
                if message.key.is_some() {
                    capabilities.insert("messaging.key".into());
                }
                blocks.insert(
                    location.clone(),
                    BuildingBlock {
                        metadata: BlockMetadata {
                            parent: Some(poolster_core::blocks::ContractReference::from_bytes(
                                <crate::AsyncApiDocument as poolster_core::engine::Contract>::NAME,
                                source.clone(),
                                &serde_json::to_vec(&document.source)
                                    .expect("native JSON is serializable"),
                            )),
                            id: BlockId {
                                source: source.clone(),
                                local: location.clone(),
                            },
                            capabilities,
                            references: vec![],
                            location: Some(location),
                        },
                        value: message,
                    },
                );
            }
            Err(error) => diagnostics.push(block_diagnostic(&location, error)),
        }
    }
    (
        Blocks {
            parent: Some(poolster_core::blocks::ContractReference::from_bytes(
                <crate::AsyncApiDocument as poolster_core::engine::Contract>::NAME,
                source.clone(),
                &serde_json::to_vec(&document.source).expect("native JSON is serializable"),
            )),
            state: if diagnostics.is_empty() {
                poolster_core::blocks::CollectionState::Complete
            } else {
                poolster_core::blocks::CollectionState::Partial {
                    diagnostics: diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.clone())
                        .collect(),
                }
            },
            items: blocks.into_values().collect(),
        },
        diagnostics,
    )
}
fn block_diagnostic(location: &str, error: anyhow::Error) -> poolster_core::input::InputDiagnostic {
    poolster_core::input::InputDiagnostic {
        code: "asyncapi-message-block-unsupported".into(),
        message: format!("{location}: {error:#}"),
    }
}
