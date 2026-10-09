//! Public, parser-independent contracts published by this input provider.
//!
//! These Poolster-owned types live in core so output plugins can consume them
//! without depending on this provider or its parser libraries. Re-exporting them
//! here makes the provider's supported contract surface explicit.
pub use poolster_core::native::workflows::{
    HttpWorkflowOperation, Workflow, WorkflowOperations, WorkflowParameter, WorkflowStep,
    WorkflowValue,
};
