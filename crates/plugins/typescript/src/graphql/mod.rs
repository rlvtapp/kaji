//! Selection-specific SDKs consume Poolster contracts rather than parser ASTs.
mod facade;
mod generate;
pub(crate) mod helpers;
pub(crate) mod incremental;
mod render;
mod validation;
pub use facade::GraphqlStyle;
use validation::{identifier, validate_type};
mod codecs;
pub(crate) mod scalars;
use crate::TypeScript;
use anyhow::{Result, bail};
use poolster_core::{
    GeneratedFile,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{GraphqlOperationKind, GraphqlOperations, ModelField, ModelKind, ModelType},
};
pub(crate) use render::{render_fields, render_type};
pub use scalars::GraphqlScalarMapping;
use std::{collections::BTreeMap, fmt::Write, path::PathBuf};
/// Actual emitted GraphQL operation symbols for downstream generation plugins.
#[derive(Clone, Debug)]
pub struct GraphqlOperationSymbols {
    pub function: crate::Symbol,
    /// Canonical camelCase standalone operation, retaining the original function.
    pub raw_function: crate::Symbol,
    pub variables: crate::Symbol,
    pub result: crate::Symbol,
    pub kind: GraphqlOperationKind,
}
/// Output capability distinct from HTTP SDK contracts.
#[derive(Clone, Debug)]
pub struct GraphqlClient {
    pub operations: BTreeMap<String, GraphqlOperationSymbols>,
    /// Package-relative module without extension.
    pub runtime_module: PathBuf,
    pub style: GraphqlStyle,
    pub factory: Option<crate::Symbol>,
    pub methods: BTreeMap<String, String>,
    /// Authoritative selected contract used to render this client.
    pub definition: GraphqlOperations,
    pub scalars: BTreeMap<String, GraphqlScalarMapping>,
}
impl Contract for GraphqlClient {
    const NAME: &'static str = "poolster.typescript.graphql-client.v1";
}
pub struct Graphql {
    meta: Meta,
    provider: Option<Handle<GraphqlOperations>>,
    subscriptions: bool,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
    scalars: BTreeMap<String, GraphqlScalarMapping>,
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        subscriptions: false,
        style: GraphqlStyle::Idiomatic,
        groups: BTreeMap::new(),
        scalars: BTreeMap::new(),
    }
}
impl Graphql {
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

    pub fn raw(mut self) -> Self {
        self.style = GraphqlStyle::Raw;
        self
    }
    pub fn flat(mut self) -> Self {
        self.style = GraphqlStyle::Flat;
        self
    }
    pub fn idiomatic(mut self) -> Self {
        self.style = GraphqlStyle::Idiomatic;
        self
    }
    pub fn namespaced(self) -> Self {
        self.idiomatic()
    }

    /// Map a custom scalar's input and output wire types without installing codecs.
    pub fn scalar(mut self, name: impl Into<String>, mapping: GraphqlScalarMapping) -> Self {
        self.scalars.insert(name.into(), mapping);
        self
    }
    pub fn scalars(mut self, mappings: BTreeMap<String, GraphqlScalarMapping>) -> Self {
        self.scalars.extend(mappings);
        self
    }
    pub fn handle(&self) -> Handle<GraphqlClient> {
        self.meta.handle()
    }
    /// Enable subscriptions using a separately injected SubscriptionTransport.
    pub fn subscriptions(mut self) -> Self {
        self.subscriptions = true;
        self
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
}
impl Plugin<TypeScript> for Graphql {
    fn kind(&self) -> &'static str {
        "typescript-graphql"
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlClient>()]
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.provider)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        generate::generate(self, cx)
    }
}

fn named_dependencies(ty: &ModelType, names: &mut std::collections::BTreeSet<String>) {
    match &ty.kind {
        ModelKind::Named(name) => {
            names.insert(name.clone());
        }
        ModelKind::List(item) => named_dependencies(item, names),
        ModelKind::Object(fields) => {
            for field in fields {
                named_dependencies(&field.ty, names);
            }
        }
        ModelKind::Union(variants) => {
            for variant in variants {
                named_dependencies(variant, names);
            }
        }
        _ => {}
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
