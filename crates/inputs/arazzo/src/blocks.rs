//! Resolved workflow blocks published alongside the complete workflow contract.
//!
//! Blocks preserve containing workflow context. Execution consumers still need
//! the whole contract to preserve sequencing and dependency semantics. Unresolved
//! inspection documents cannot produce executable blocks.
pub use poolster_core::native::workflows::WorkflowStepBlock;

/// The provider's independently selectable workflow step collection.
pub type WorkflowStepBlocks = poolster_core::blocks::Blocks<WorkflowStepBlock>;

/// Extract blocks with a stable caller-supplied document identity.
pub fn step_blocks(
    contract: &crate::contracts::WorkflowOperations,
    source: impl Into<String>,
) -> WorkflowStepBlocks {
    contract.step_blocks(source)
}
