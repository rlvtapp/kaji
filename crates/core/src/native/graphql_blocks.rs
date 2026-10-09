//! Optional GraphQL input-model exposure; selections stay in the whole contract.
use super::{GraphqlOperations, ModelKind, ModelType};
use crate::blocks::{BlockId, BlockMetadata, Blocks, BuildingBlock, ModelBlock};
use std::collections::BTreeSet;

impl GraphqlOperations {
    /// Expose input object shapes without treating schema objects as query results.
    /// Consumers still need GraphqlOperations for operations and execution semantics.
    pub fn input_model_blocks(&self, source: impl Into<String>) -> Blocks<ModelBlock> {
        let source = source.into();
        Blocks {
            parent: Some(crate::blocks::ContractReference::from_bytes(
                <Self as crate::engine::Contract>::NAME,
                source.clone(),
                &serde_json::to_vec(self).expect("owned contract is serializable"),
            )),
            state: crate::blocks::CollectionState::Complete,
            items: self
                .input_objects
                .iter()
                .map(|(name, fields)| BuildingBlock {
                    metadata: BlockMetadata {
                        parent: Some(crate::blocks::ContractReference::from_bytes(
                            <Self as crate::engine::Contract>::NAME,
                            source.clone(),
                            &serde_json::to_vec(self).expect("owned contract is serializable"),
                        )),
                        id: BlockId {
                            source: source.clone(),
                            local: format!("input:{name}"),
                        },
                        capabilities: BTreeSet::from(["model".into(), "graphql.input".into()]),
                        references: vec![],
                        location: Some(name.clone()),
                    },
                    value: ModelBlock {
                        name: name.clone(),
                        ty: ModelType {
                            nullable: false,
                            kind: ModelKind::Object(fields.clone()),
                        },
                    },
                })
                .collect(),
        }
    }
}

impl crate::blocks::Block for super::GraphqlOperation {
    const CONTRACT_NAME: &'static str = "poolster.graphql-operation-blocks.v1";
}
impl GraphqlOperations {
    pub fn operation_blocks(&self, source: impl Into<String>) -> Blocks<super::GraphqlOperation> {
        let source = source.into();
        Blocks {
            parent: Some(crate::blocks::ContractReference::from_bytes(
                <Self as crate::engine::Contract>::NAME,
                source.clone(),
                &serde_json::to_vec(self).expect("owned contract is serializable"),
            )),
            state: crate::blocks::CollectionState::Complete,
            items: self
                .operations
                .iter()
                .map(|operation| BuildingBlock {
                    metadata: BlockMetadata {
                        parent: Some(crate::blocks::ContractReference::from_bytes(
                            <Self as crate::engine::Contract>::NAME,
                            source.clone(),
                            &serde_json::to_vec(self).expect("owned contract is serializable"),
                        )),
                        id: BlockId {
                            source: source.clone(),
                            local: operation.name.clone(),
                        },
                        capabilities: [
                            "graphql.operation".into(),
                            format!(
                                "graphql.{}",
                                match operation.kind {
                                    super::GraphqlOperationKind::Query => "query",
                                    super::GraphqlOperationKind::Mutation => "mutation",
                                    super::GraphqlOperationKind::Subscription => "subscription",
                                }
                            ),
                        ]
                        .into(),
                        references: vec![],
                        location: Some(operation.name.clone()),
                    },
                    value: operation.clone(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::ModelField;
    #[test]
    fn input_blocks_preserve_presence_nullability_and_defaults() {
        let field = ModelField {
            name: "nickname".into(),
            ty: ModelType {
                nullable: true,
                kind: ModelKind::Scalar("String".into()),
            },
            optional: true,
            default_value: Some("\"guest\"".into()),
        };
        let contract = GraphqlOperations {
            schema_source: String::new(),
            operation_source: String::new(),
            operations: vec![],
            input_objects: [("Profile".into(), vec![field.clone()])].into(),
        };
        let blocks = contract.input_model_blocks("schema");
        assert_eq!(blocks.items[0].metadata.id.local, "input:Profile");
        assert_eq!(blocks.with_capability("graphql.input").count(), 1);
        assert_eq!(
            blocks.items[0].value.ty.kind,
            ModelKind::Object(vec![field])
        );
    }
}
