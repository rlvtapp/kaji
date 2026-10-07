//! Rust-native, target-neutral primitives for OpenAPI code generation.

pub mod adapter;
pub mod api_reference;
pub use api_reference::api_reference;
pub mod ast;
pub mod customization;
pub mod engine;
pub mod files;
pub mod filters;
pub mod httpmock;
pub mod idempotency;
pub mod manifest;
pub mod mocking;
pub mod openapi32;
pub mod pagination;
pub mod release;
pub mod samples;
pub mod semantics;
pub mod style;
pub mod vendor;

pub use adapter::{AdaptedApi, Adapter};
pub use ast::{
    AdditionalProperties, Api, Discriminator, Field, HttpMethod, OAuthFlow, Operation,
    OperationMediaType, OperationParameter, OperationRequestBody, OperationResponse, Schema,
    SchemaKind, SchemaValue, SecurityRequirement, SecurityScheme, SecuritySchemeCatalog,
    SecuritySchemeKind,
};
pub use files::{GeneratedFile, GeneratedTree, OutputChanges};
pub use filters::{
    OperationContext, OperationFilter, OperationSelection, OverrideFilter, OverrideRule,
    OverrideRules, wildcard_matches,
};
pub use httpmock::{HttpmockFixtureConfig, generate_httpmock_fixtures};
pub use manifest::{GeneratedManifest, MANIFEST_VERSION, ManifestEntry};
pub use mocking::{
    MockRequestMatch, MockResponse, MockScenario, extract_mock_scenarios,
    extract_operation_mock_scenarios, mock_scenario_matches,
};
pub use semantics::{
    AuthAlternative, AuthScheme, DeclaredError, OperationSemantics, PaginationHint,
    PaginationSource, RequestBodyKind, RetryClass, SdkSemantics, StreamingKind, analyze_operation,
    analyze_sdk_semantics,
};
pub use style::SdkClientStyle;
