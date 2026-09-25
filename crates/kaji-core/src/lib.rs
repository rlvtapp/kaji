//! Rust-native, target-neutral primitives for OpenAPI code generation.

pub mod adapter;
pub mod ast;
pub mod conformance;
pub mod files;
pub mod filters;
pub mod httpmock;
pub mod manifest;
pub mod mocking;
pub mod plugin;
pub mod plugins;
pub mod sdk;
pub mod semantics;

pub use ast::{
    AdditionalProperties, Api, Discriminator, Field, HttpMethod, OAuthFlow, Operation,
    OperationMediaType, OperationParameter, OperationRequestBody, OperationResponse, Schema,
    SchemaKind, SchemaValue, SecurityRequirement, SecurityScheme, SecuritySchemeCatalog,
    SecuritySchemeKind,
};
pub use conformance::{
    GenerationLifecycle, GenerationResult, PluginInvocation, generate_with_plan,
    generate_with_plan_and_lifecycle, ordered_plugins,
};
pub use files::{GeneratedFile, GeneratedTree};
pub use filters::{
    OperationContext, OperationFilter, OperationSelection, OverrideFilter, OverrideRule,
    OverrideRules, wildcard_matches,
};
pub use httpmock::{HttpmockFixtureConfig, generate_httpmock_fixtures};
pub use manifest::{GeneratedManifest, MANIFEST_VERSION, ManifestEntry};
pub use mocking::{
    MockRequestMatch, MockResponse, MockScenario, extract_mock_scenarios,
    extract_operation_mock_scenarios,
};
pub use plugin::{CodegenPlugin, GeneratorConfig, generate};
pub use sdk::{
    SdkClientStyle, SdkLanguage, SdkProfile, SdkStyle, SdkSurface, SdkTransport,
    generate_openapi_sdks, generate_sdks, generate_sdks_with_security_catalog,
};
pub use semantics::{
    AuthAlternative, AuthScheme, DeclaredError, OperationSemantics, PaginationHint,
    PaginationSource, RequestBodyKind, RetryClass, SdkSemantics, StreamingKind, analyze_operation,
    analyze_sdk_semantics,
};
