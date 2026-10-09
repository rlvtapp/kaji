//! Public building-block API. Schema input models and selected operations differ.
pub use poolster_core::blocks::{Blocks, BuildingBlock, ModelBlock};
pub use poolster_core::native::GraphqlOperation as OperationBlock;
pub type InputModelBlocks = Blocks<ModelBlock>;
pub type OperationBlocks = Blocks<OperationBlock>;
pub fn input_models(
    contract: &super::contracts::GraphqlOperations,
    source: &str,
) -> InputModelBlocks {
    contract.input_model_blocks(source)
}
pub fn operations(contract: &super::contracts::GraphqlOperations, source: &str) -> OperationBlocks {
    contract.operation_blocks(source)
}
