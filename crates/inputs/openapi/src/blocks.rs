//! Public block API produced by the OpenAPI input.
//!
//! Blocks reuse the rich HTTP definitions rather than losing schema constraints
//! or inventing a second endpoint representation. These names are aliases of
//! exactly the types existing typed consumers require.
pub use poolster_core::blocks::{
    Block, BlockId, BlockMetadata, BlockReference, Blocks, BuildingBlock,
};
pub use poolster_core::{Operation as EndpointBlock, Schema as ModelBlock};

pub type ModelBlocks = Blocks<ModelBlock>;
pub type EndpointBlocks = Blocks<EndpointBlock>;

/// Extract model blocks while retaining the authoritative whole HTTP contract.
pub fn models(contract: &super::contracts::HttpContract, source: &str) -> ModelBlocks {
    contract.schema_blocks(source)
}
/// Extract endpoints; reusable security definitions remain in HttpContract.
pub fn endpoints(contract: &super::contracts::HttpContract, source: &str) -> EndpointBlocks {
    contract.endpoint_blocks(source)
}
