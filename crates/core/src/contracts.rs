//! Cohesive public imports for Poolster-owned input/output contracts.
//!
//! These are re-exports, not replacement types. Existing root, `native` and
//! `blocks` paths remain compatible, including TypeId, NAME and serialization.
//! Protocol families retain their wire semantics; arbitrary community contracts
//! continue to use the engine's open Contract trait.

/// Semantically shared model vocabulary and contract/block provenance.
pub mod common {
    pub use crate::blocks::{
        Block, BlockId, BlockMetadata, BlockReference, Blocks, BuildingBlock, CollectionState,
        ContractReference, ModelBlock,
    };
    pub use crate::engine::Contract;
    pub use crate::native::{ModelField, ModelKind, ModelType};
}
/// HTTP models, operations, media and security contracts.
pub mod http {
    pub use crate::{
        AdaptedApi, AdditionalProperties, Api, Discriminator, Field, HttpMethod, OAuthFlow,
        Operation, OperationMediaType, OperationParameter, OperationRequestBody, OperationResponse,
        Schema, SchemaKind, SchemaValue, SecurityRequirement, SecurityScheme,
        SecuritySchemeCatalog, SecuritySchemeKind,
    };
}
/// Fixed GraphQL operations and separately supported incremental execution.
pub mod graphql {
    pub use crate::native::{
        GraphqlIncrementalCondition, GraphqlIncrementalDialect, GraphqlIncrementalKind,
        GraphqlIncrementalOperations, GraphqlIncrementalSelection, GraphqlOperation,
        GraphqlOperationKind, GraphqlOperations, graphql_scalar_fields, graphql_scalar_shape,
    };
}
/// Native RPC services/messages; official descriptor bytes retain wire details.
pub mod rpc {
    pub use crate::native::rpc::{
        RpcContract, RpcFile, RpcMethod, RpcMethodBlock, RpcService, RpcStreaming,
    };
}
/// Messages, channels and explicit broker bindings.
pub mod events {
    pub use crate::native::events::{
        EventAction, EventMessage, EventOperation, EventOperations, KafkaBroker,
    };
}
/// Source-resolved workflows; HTTP steps keep their protocol-specific metadata.
pub mod workflows {
    pub use crate::native::workflows::{
        HttpWorkflowOperation, Workflow, WorkflowOperations, WorkflowParameter, WorkflowStep,
        WorkflowStepBlock, WorkflowValue,
    };
}
/// Owned capability-RPC metadata. Complete native documents remain input-owned.
pub mod capabilities {
    pub use crate::native::capnproto::{CapabilityMethod, CapnpNodeBlock, CapnpNodeKind};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Contract;
    use std::any::TypeId;

    #[test]
    fn protocol_facades_preserve_original_type_identity() {
        macro_rules! same {
            ($new:ty, $old:ty) => {
                assert_eq!(TypeId::of::<$new>(), TypeId::of::<$old>());
            };
        }
        same!(http::AdaptedApi, crate::AdaptedApi);
        same!(http::Schema, crate::Schema);
        same!(common::ModelType, crate::native::ModelType);
        same!(
            common::Blocks<graphql::GraphqlOperation>,
            crate::blocks::Blocks<crate::native::GraphqlOperation>
        );
        same!(graphql::GraphqlOperations, crate::native::GraphqlOperations);
        same!(
            graphql::GraphqlIncrementalOperations,
            crate::native::GraphqlIncrementalOperations
        );
        same!(rpc::RpcContract, crate::native::rpc::RpcContract);
        same!(
            events::EventOperations,
            crate::native::events::EventOperations
        );
        same!(
            workflows::WorkflowOperations,
            crate::native::workflows::WorkflowOperations
        );
        same!(
            capabilities::CapnpNodeBlock,
            crate::native::capnproto::CapnpNodeBlock
        );
        assert_eq!(
            graphql::GraphqlOperations::NAME,
            crate::native::GraphqlOperations::NAME
        );
        assert_eq!(
            rpc::RpcContract::NAME,
            crate::native::rpc::RpcContract::NAME
        );
    }
    #[test]
    fn original_payloads_round_trip_through_cohesive_imports() {
        let original = crate::native::GraphqlOperations {
            schema_source: String::new(),
            operation_source: "query Read { value }".into(),
            operations: vec![],
            input_objects: Default::default(),
        };
        let bytes = serde_json::to_vec(&original).unwrap();
        let imported: graphql::GraphqlOperations = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(imported, original);
        assert_eq!(serde_json::to_vec(&imported).unwrap(), bytes);
        let original = crate::AdaptedApi::default();
        let bytes = serde_json::to_vec(&original).unwrap();
        let imported: http::AdaptedApi = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(serde_json::to_vec(&imported).unwrap(), bytes);
    }
}
