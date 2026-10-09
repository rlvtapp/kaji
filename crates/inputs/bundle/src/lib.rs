//! Convenience registry for independently packaged input providers.
use anyhow::Result;
pub use poolster_core::input::{
    InputContract, InputDiagnostic, InputOperation, InputPlugin, InputPluginInfo, InputProvider,
    InputRegistry, InputSummary, LoadedInput,
};

#[cfg(feature = "graphql")]
pub use poolster_input_graphql as graphql;
#[cfg(feature = "graphql")]
pub use poolster_input_graphql::GraphqlInput;

#[cfg(feature = "asyncapi")]
pub use poolster_input_asyncapi as asyncapi;
#[cfg(feature = "asyncapi")]
pub use poolster_input_asyncapi::AsyncApiInput;

#[cfg(feature = "arazzo")]
pub use poolster_input_arazzo as arazzo;
#[cfg(feature = "arazzo")]
pub use poolster_input_arazzo::ArazzoInput;

#[cfg(feature = "protobuf")]
pub use poolster_input_protobuf as protobuf;
#[cfg(feature = "protobuf")]
pub use poolster_input_protobuf::ProtobufInput;

#[cfg(feature = "capnproto")]
pub use poolster_input_capnproto as capnproto;
#[cfg(feature = "capnproto")]
pub use poolster_input_capnproto::CapnProtoInput;

/// Register the feature-enabled providers. Alternative providers can be added
/// using the same core interface without depending on this bundle.
#[cfg(feature = "openapi")]
pub use poolster_input_openapi as openapi;
#[cfg(feature = "openapi")]
pub use poolster_input_openapi::OpenApiInput;

pub fn default_registry() -> Result<InputRegistry> {
    #[allow(unused_mut)]
    let mut registry = InputRegistry::new();
    #[cfg(feature = "graphql")]
    registry.register(GraphqlInput)?;
    #[cfg(feature = "openapi")]
    registry.register(OpenApiInput)?;
    #[cfg(feature = "asyncapi")]
    registry.register(AsyncApiInput)?;
    #[cfg(feature = "arazzo")]
    registry.register(ArazzoInput)?;
    #[cfg(feature = "protobuf")]
    registry.register(ProtobufInput)?;
    #[cfg(feature = "capnproto")]
    registry.register(CapnProtoInput)?;
    Ok(registry)
}
