//! TypeScript renderers and package configuration live outside neutral core.
pub mod contracts;

mod events;
pub use events::{AsyncApi, KafkaClient, KafkaOperationSymbols, asyncapi};
mod workflow;
pub use workflow::{WorkflowClient, WorkflowRunner, workflow};
mod graphql;
pub use graphql::incremental::{GraphqlIncremental, GraphqlIncrementalClient, graphql_incremental};
pub use graphql::{
    Graphql, GraphqlClient, GraphqlOperationSymbols, GraphqlScalarMapping, GraphqlStyle, graphql,
};
mod oauth;
pub use oauth::{OAuth, oauth};
mod webhooks;
pub use webhooks::{Webhooks, webhooks};
mod auxiliary_fixture;
mod auxiliary_layout;
mod auxiliary_options;
mod auxiliary_validation;
pub use auxiliary_options::{CypressOperationOptions, CypressOptions, FixtureOptions};
pub use poolster_core::SourceLayout;
mod bundled_middleware;
mod clients;
pub mod composition;
pub use composition::{cypress, faker, msw, zod};
pub use composition::{graphql_react_query, graphql_swr, graphql_vue_query};
mod esm;
mod operation_tests;
pub use operation_tests::{OperationTests, operation_tests};
mod json;
mod models;
mod query_helpers;
mod render;
mod request_control;
mod sdk;
mod symbols;
mod workspace;
pub use models::{
    ArrayType, EnumConstCasing, EnumKeyCasing, EnumType, Int64Type, ModelOptions, OptionalType,
    Syntax,
};
pub use workspace::{Symbol, TsTypes, Workspace};

/// Standalone auxiliary renderers with explicit typed configuration. Plugin
/// authors can publish their files through the typed engine's emitter.
pub mod artifacts {
    pub use crate::render::{
        ArtifactOptions, McpToolManifest, Naming, ReDoc, TypeScriptCypress, TypeScriptFaker,
        TypeScriptModels, TypeScriptMsw, TypeScriptPackage, TypeScriptReactQuery, TypeScriptSwr,
        TypeScriptVueQuery, TypeScriptZod,
    };
}

use anyhow::Result;
use poolster_core::engine::{
    FinalizeContext, Handle, Language, Meta, Package, Plugin, PluginContext, Provision,
};
use poolster_core::{GeneratedFile, SdkClientStyle};
use sdk::{SdkSurface, SdkTransport};
use std::path::Path;

mod package;
pub use package::{PackageExt, Settings, TypeScript, package};
mod http;
pub use http::{Sdk, Types, sdk, types};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod idempotency_tests;
#[cfg(test)]
mod middleware_tests;

#[cfg(test)]
mod openapi32;

#[cfg(test)]
mod auxiliary_tests;
