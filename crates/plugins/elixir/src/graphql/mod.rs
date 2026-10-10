//! Native GraphQL clients consume selection contracts, never the HTTP AST.
mod emission;
use emission::*;
mod capabilities;
mod layout;
mod models;
pub use capabilities::{GraphqlIncremental, graphql_incremental};
#[cfg(test)]
mod tests;
use crate::Elixir;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile, GeneratedTree,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{GraphqlOperationKind, GraphqlOperations},
};
use std::{collections::BTreeMap, fmt::Write};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GraphqlStyle {
    Raw,
    Flat,
    #[default]
    Idiomatic,
}
#[derive(Clone, Debug)]
pub struct GraphqlClient {
    pub methods: BTreeMap<String, String>,
}
impl Contract for GraphqlClient {
    const NAME: &'static str = "poolster.elixir.graphql-client.v1";
}
pub struct Graphql {
    meta: Meta,
    provider: Option<Handle<GraphqlOperations>>,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
    subscriptions: bool,
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        style: GraphqlStyle::default(),
        groups: BTreeMap::new(),
        subscriptions: false,
    }
}
impl Graphql {
    /// Enable HTTP SSE distinct-connection subscriptions.
    pub fn subscriptions(mut self) -> Self {
        self.subscriptions = true;
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
    pub fn groups(mut self, groups: BTreeMap<String, BTreeMap<String, String>>) -> Self {
        self.groups = groups;
        self
    }
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
    pub fn handle(&self) -> Handle<GraphqlClient> {
        self.meta.handle()
    }
}
impl Plugin<Elixir> for Graphql {
    fn kind(&self) -> &'static str {
        "elixir-graphql"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.provider)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Elixir>) -> Result<()> {
        let (mut tree, methods) = render_capabilities(
            cx.inputs.get::<GraphqlOperations>()?,
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("graphql_client"),
            self.style,
            &self.groups,
            self.subscriptions,
            false,
            None,
        )?;
        if let Some(version) = &cx.common.package_version {
            let path = "mix.exs";
            let content = tree
                .get(path)
                .unwrap()
                .replace("0.0.0", &crate::package_version(version));
            tree.replace(GeneratedFile::new(path, content)?)?;
        }
        cx.files.append(tree)?;
        cx.publish(GraphqlClient { methods })
    }
}

fn elixir_string(value: &str) -> String {
    format!(
        "\"{}\"",
        crate::escape_elixir_string(value).replace("#{", "\\#{")
    )
}
