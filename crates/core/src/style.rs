use serde::{Deserialize, Serialize};

/// Public instantiated-client layout. TypeScript retains direct function
/// exports in both modes; native language plugins use the same choice to emit
/// either their flat idiomatic client or a resource-namespaced facade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkClientStyle {
    /// `client.createMessage(...)` (or its native-language equivalent).
    Flat,
    /// `client.messages.create(...)` (or its native-language equivalent),
    /// using OpenAPI tags first and stable path-derived namespaces when a spec
    /// has no tags.
    #[default]
    Namespaced,
}
