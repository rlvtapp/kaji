//! Discoverable generated-artifact contracts; reexports preserve existing identity.
pub mod http {
    pub use crate::NativeSdk;
}
pub mod graphql {
    pub use crate::GraphqlClient;
}
