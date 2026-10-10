//! Language-owned generated artifact contracts for companion plugins.
//! Re-exports preserve existing provider handles and contract type identity.

/// HTTP artifact symbols and shared execution interfaces.
pub mod http {
    pub use crate::composition::{Client, Models, Operations, Transport};
}

/// Native GraphQL operation symbols and selected calling surface.
pub mod graphql {
    pub use crate::graphql::incremental::GraphqlIncrementalClient;
    pub use crate::graphql::{GraphqlClient, GraphqlOperationSymbols};
}

/// Standalone model-only artifacts.
pub use crate::TsTypes;
