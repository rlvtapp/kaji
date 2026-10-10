//! Independently selectable SDK providers. Contracts describe emitted modules,
//! so consumers never reconstruct imports from operation IDs or output recipes.
mod options;
use crate::{ModelOptions, Symbol, TypeScript, render, sdk};
use anyhow::{Context, Result};
use poolster_core::{
    GeneratedFile,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Models {
    pub schemas: BTreeMap<String, Symbol>,
    pub operation_modules: BTreeMap<String, PathBuf>,
    pub options: ModelOptions,
}
impl Contract for Models {
    const NAME: &'static str = "typescript.sdk-models";
}
/// A module implementing Poolster's runtime ABI: Options, RequestResult,
/// ResponseResult, ClientConfig, ClientInstance, client, createClient,
/// resolveResponse and stream. Community transports may publish this contract.
#[derive(Clone)]
pub struct Transport {
    pub module: PathBuf,
    /// The runtime consumes schema-directed JSON plans without losing digits.
    pub lossless_json: bool,
}
impl Transport {
    pub fn new(module: impl Into<PathBuf>) -> Self {
        Self {
            module: module.into(),
            lossless_json: false,
        }
    }
    pub fn lossless_json(mut self, supported: bool) -> Self {
        self.lossless_json = supported;
        self
    }
}
impl Contract for Transport {
    const NAME: &'static str = "typescript.transport";
}
#[derive(Clone)]
pub struct Operations {
    pub functions: BTreeMap<String, Symbol>,
}
impl Contract for Operations {
    const NAME: &'static str = "typescript.operations";
}
#[derive(Clone)]
pub struct Client {
    pub symbol: Symbol,
}
impl Contract for Client {
    const NAME: &'static str = "typescript.client";
}

#[derive(Clone, Copy)]
enum Part {
    Models,
    Transport,
    Operations,
    Client,
}
/// Uses the same maintained renderers as `sdk()`, emitting only its owned part.
pub struct Provider {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    part: Part,
    config: sdk::SdkConfig,
    output: String,
    models: Option<Handle<Models>>,
    transport: Option<Handle<Transport>>,
    operations: Option<Handle<Operations>>,
}
fn provider(part: Part, output: &str) -> Provider {
    let mut config = sdk::SdkConfig::new("__package");
    config.group_by_tag = false;
    config.client_style = poolster_core::SdkClientStyle::Flat;
    Provider {
        http_input: Default::default(),
        meta: Meta::new(),
        part,
        config,
        output: output.into(),
        models: None,
        transport: None,
        operations: None,
    }
}
pub fn models() -> Provider {
    provider(Part::Models, "models")
}
pub fn transport() -> Provider {
    provider(Part::Transport, ".poolster/client")
}
pub fn operations() -> Provider {
    provider(Part::Operations, "operations")
}
pub fn client() -> Provider {
    provider(Part::Client, "client")
}

fn module_import(module: &Path, file: &Path) -> Result<String> {
    Symbol {
        module: module.into(),
        name: String::new(),
    }
    .import_from(file)
}
mod provider;
mod query;
pub use query::{
    Query, QueryFramework, QueryKind, graphql_react_query, graphql_swr, graphql_vue_query,
    react_query, swr, vue_query,
};

/// Auxiliary artifacts tied to the selected model and operation providers.
pub struct Auxiliary {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    kind: AuxiliaryKind,
    graphql: bool,
    graphql_client: Option<Handle<crate::GraphqlClient>>,
    output: String,
    models: Option<Handle<Models>>,
    operations: Option<Handle<Operations>>,
    max_file_bytes: usize,
    layout: Option<poolster_core::SourceLayout>,
    fixture_options: crate::FixtureOptions,
    cypress_options: crate::CypressOptions,
}
enum AuxiliaryKind {
    Zod,
    Faker,
    Msw,
    Cypress,
}
fn auxiliary(kind: AuxiliaryKind, output: &str) -> Auxiliary {
    Auxiliary {
        http_input: Default::default(),
        meta: Meta::new(),
        kind,
        graphql: false,
        graphql_client: None,
        output: output.into(),
        models: None,
        operations: None,
        max_file_bytes: 128 * 1024,
        layout: None,
        fixture_options: Default::default(),
        cypress_options: Default::default(),
    }
}
pub fn zod() -> Auxiliary {
    auxiliary(AuxiliaryKind::Zod, "zod")
}
pub fn faker() -> Auxiliary {
    auxiliary(AuxiliaryKind::Faker, "faker")
}
pub fn msw() -> Auxiliary {
    auxiliary(AuxiliaryKind::Msw, "msw")
}
pub fn cypress() -> Auxiliary {
    auxiliary(AuxiliaryKind::Cypress, "cypress")
}

mod auxiliary;

#[cfg(test)]
mod tests;
