//! Rust-native, target-neutral primitives for OpenAPI code generation.

pub mod adapter;
pub mod ast;
pub mod conformance;
pub mod engine;
pub mod files;
pub mod filters;
pub mod httpmock;
pub mod manifest;
pub mod mocking;
pub mod plugin;
pub mod semantics;
pub mod style;

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
pub use semantics::{
    AuthAlternative, AuthScheme, DeclaredError, OperationSemantics, PaginationHint,
    PaginationSource, RequestBodyKind, RetryClass, SdkSemantics, StreamingKind, analyze_operation,
    analyze_sdk_semantics,
};
pub use style::SdkClientStyle;
