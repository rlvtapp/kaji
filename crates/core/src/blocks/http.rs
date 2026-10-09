//! Lossless projections of the existing HTTP model.
use super::{Block, BlockId, BlockMetadata, Blocks, BuildingBlock};
use crate::{AdaptedApi, Operation, Schema};
impl Block for Schema {
    const CONTRACT_NAME: &'static str = "poolster.schema-blocks.v1";
}
impl Block for Operation {
    const CONTRACT_NAME: &'static str = "poolster.http-operation-blocks.v1";
}
fn metadata(source: &str, local: String, tag: &str) -> BlockMetadata {
    BlockMetadata {
        parent: None,
        id: BlockId {
            source: source.into(),
            local,
        },
        capabilities: [tag.into()].into(),
        references: vec![],
        location: None,
    }
}
impl AdaptedApi {
    pub fn schema_blocks(&self, source: &str) -> Blocks<Schema> {
        Blocks {
            parent: None,
            state: crate::blocks::CollectionState::Complete,
            items: self
                .api
                .schemas
                .iter()
                .map(|schema| BuildingBlock {
                    metadata: metadata(source, schema.name.clone(), "model"),
                    value: schema.clone(),
                })
                .collect(),
        }
    }
    /// Security definitions remain available through the whole AdaptedApi.
    pub fn endpoint_blocks(&self, source: &str) -> Blocks<Operation> {
        Blocks {
            parent: None,
            state: crate::blocks::CollectionState::Complete,
            items: self
                .api
                .operations
                .iter()
                .map(|operation| BuildingBlock {
                    metadata: metadata(
                        source,
                        format!("{} {}", operation.method.as_str(), operation.path),
                        "http.endpoint",
                    ),
                    value: operation.clone(),
                })
                .collect(),
        }
    }
}
