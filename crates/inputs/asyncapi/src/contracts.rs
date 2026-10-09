//! Public AsyncAPI contract surface for input and output plugin authors.
//!
//! These shared Poolster definitions retain their existing typed graph identities;
//! this module introduces no wrapper contracts or parser-library dependencies.
pub use poolster_core::native::events::{
    EventAction, EventMessage, EventOperation, EventOperations, KafkaBroker,
};
/// Independently consumable message models published by the input provider.
/// The authoritative native document and executable whole contract remain separate.
pub type MessageBlocks = poolster_core::blocks::Blocks<EventMessage>;
