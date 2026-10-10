//! GraphQL operations render from owned contracts, independently of HTTP Api.
mod generate;
mod models;
mod scalars;
mod styles;
use crate::Rust;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{
        GraphqlIncrementalOperations, GraphqlOperationKind, GraphqlOperations,
        graphql_scalar_fields, graphql_scalar_shape,
    },
};
pub use scalars::GraphqlScalarMapping;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
pub use styles::GraphqlStyle;
#[derive(Clone, Debug)]
pub struct GraphqlOperationSymbols {
    pub function: String,
    pub variables: String,
    pub result: String,
    pub kind: GraphqlOperationKind,
    pub incremental: bool,
}
#[derive(Clone, Debug)]
pub struct GraphqlClient {
    pub operations: BTreeMap<String, GraphqlOperationSymbols>,
    pub style: GraphqlStyle,
    pub methods: BTreeMap<String, String>,
}
impl Contract for GraphqlClient {
    const NAME: &'static str = "poolster.rust.graphql-client.v1";
}
pub struct Graphql {
    meta: Meta,
    provider: Option<Handle<GraphqlOperations>>,
    incremental_provider: Option<Handle<GraphqlIncrementalOperations>>,
    subscriptions: bool,
    incremental_mode: bool,
    scalars: BTreeMap<String, GraphqlScalarMapping>,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        incremental_provider: None,
        subscriptions: false,
        incremental_mode: false,
        scalars: BTreeMap::new(),
        style: GraphqlStyle::Idiomatic,
        groups: BTreeMap::new(),
    }
}
/// Generate multipart incremental operations from the distinct capability contract.
pub fn graphql_incremental(provider: Option<Handle<GraphqlIncrementalOperations>>) -> Graphql {
    let mut generator = graphql(None);
    generator.incremental_provider = provider;
    generator.incremental_mode = true;
    generator
}
impl Graphql {
    /// Enable distinct-connection graphql-sse subscription operations.
    pub fn subscriptions(mut self) -> Self {
        self.subscriptions = true;
        self
    }
    /// Emit free operation functions taking an explicit HTTP transport.
    pub fn raw(mut self) -> Self {
        self.style = GraphqlStyle::Raw;
        self
    }
    /// Add bound snake_case methods such as `client.read_user(&variables).await`.
    /// Operations without variable declarations take no method arguments.
    pub fn flat(mut self) -> Self {
        self.style = GraphqlStyle::Flat;
        self
    }
    /// Group bound methods by operation kind: `client.query().read_user(&variables)`.
    /// Custom groups can override the operation-kind grouping.
    pub fn idiomatic(mut self) -> Self {
        self.style = GraphqlStyle::Idiomatic;
        self
    }
    pub fn namespaced(self) -> Self {
        self.idiomatic()
    }
    /// Assign an operation to a custom group and method, normalized to snake_case.
    /// This requires idiomatic style; unassigned operations keep their kind group.
    pub fn group(
        mut self,
        group: impl Into<String>,
        method: impl Into<String>,
        operation: impl Into<String>,
    ) -> Self {
        self.groups
            .entry(group.into())
            .or_default()
            .insert(method.into(), operation.into());
        self
    }
    pub fn groups(mut self, groups: BTreeMap<String, BTreeMap<String, String>>) -> Self {
        self.groups = groups;
        self
    }

    pub fn scalar(mut self, name: impl Into<String>, mapping: GraphqlScalarMapping) -> Self {
        self.scalars.insert(name.into(), mapping);
        self
    }
    pub fn scalars(mut self, mappings: BTreeMap<String, GraphqlScalarMapping>) -> Self {
        self.scalars = mappings;
        self
    }

    pub fn handle(&self) -> Handle<GraphqlClient> {
        self.meta.handle()
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
}
impl Plugin<Rust> for Graphql {
    fn kind(&self) -> &'static str {
        "rust-graphql"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        if self.incremental_mode {
            vec![Requirement::on(self.incremental_provider)]
        } else {
            vec![Requirement::on(self.provider)]
        }
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        generate::generate(self, cx)
    }
}
pub(crate) struct NativePackage {
    name: String,
    version: String,
}
impl NativePackage {
    pub(crate) fn finalize(
        &self,
        cx: &mut poolster_core::engine::FinalizeContext<'_, Rust>,
    ) -> Result<()> {
        ensure!(
            !cx.workspace.models && !cx.workspace.operations && cx.workspace.http_api.is_none(),
            "HTTP and GraphQL generators require separate Rust packages"
        );
        cx.files.emit(GeneratedFile::new("src/lib.rs", "pub mod graphql;\npub mod graphql_runtime;\npub mod graphql_codecs;\npub mod graphql_sse;\npub mod graphql_incremental;\npub use graphql_codecs::*;\npub use graphql_sse::*;\npub use graphql_incremental::*;\npub use graphql::*;\npub use graphql_runtime::*;\npub use reqwest;\n")?)?;
        let manifest = format!(
            "[package]\nname = {:?}\nversion = {:?}\nedition = \"2024\"\n\n[dependencies]\nreqwest = {{ version = \"=0.12.28\", default-features = false, features = [\"json\", \"rustls-tls\"] }}\nserde = {{ version = \"=1.0.229\", features = [\"derive\"] }}\nserde_json = \"=1.0.151\"\n",
            self.name, self.version
        );
        cx.files.emit(GeneratedFile::new("Cargo.toml", manifest)?)
    }
}

fn emit_source(
    files: &mut poolster_core::engine::Emitter<'_>,
    file: GeneratedFile,
    oversized: &mut Vec<serde_json::Value>,
) -> Result<()> {
    if file.contents.len() > 128 * 1024 {
        oversized.push(serde_json::json!({"path": file.path, "bytes": file.contents.len(), "max_file_bytes": 128 * 1024, "reason": "GraphQL source exceeds the grouping budget; retained intact. Review selection size or layout."}));
    }
    files.emit(file)
}
