//! Native selection-specific GraphQL Python clients.
use crate::{Python, python_identifier, python_module_name};
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile, GeneratedTree,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{
        GraphqlIncrementalOperations, GraphqlOperationKind, GraphqlOperations, ModelField,
        ModelKind, ModelType,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
#[derive(Clone, Copy, Debug, Default)]
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
    const NAME: &'static str = "poolster.python.graphql-client.v1";
}
pub struct Graphql {
    meta: Meta,
    provider: Option<Handle<GraphqlOperations>>,
    incremental_provider: Option<Handle<GraphqlIncrementalOperations>>,
    subscriptions: bool,
    incremental: bool,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        incremental_provider: None,
        subscriptions: false,
        incremental: false,
        style: GraphqlStyle::default(),
        groups: BTreeMap::new(),
    }
}
pub fn graphql_incremental(provider: Option<Handle<GraphqlIncrementalOperations>>) -> Graphql {
    let mut plugin = graphql(None);
    plugin.incremental_provider = provider;
    plugin.incremental = true;
    plugin
}
impl Graphql {
    pub fn subscriptions(mut self) -> Self {
        self.subscriptions = true;
        self
    }
    pub fn incremental_input(mut self, input: Handle<GraphqlIncrementalOperations>) -> Self {
        self.incremental_provider = Some(input);
        self.incremental = true;
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
impl Plugin<Python> for Graphql {
    fn kind(&self) -> &'static str {
        "python-graphql"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        if self.incremental {
            vec![Requirement::on(self.incremental_provider)]
        } else {
            vec![Requirement::on(self.provider)]
        }
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Python>) -> Result<()> {
        let contract = if self.incremental {
            &cx.inputs.get::<GraphqlIncrementalOperations>()?.definition
        } else {
            cx.inputs.get::<GraphqlOperations>()?
        };
        let (mut tree, methods) = render_advanced(
            contract,
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("graphql_client"),
            self.style,
            &self.groups,
            self.subscriptions,
            self.incremental,
        )?;
        if let Some(version) = &cx.common.package_version {
            let manifest = tree.get("pyproject.toml").unwrap().replace(
                "version = \"0.0.0\"",
                &format!("version = {:?}", crate::python_package_version(version)),
            );
            tree.replace(GeneratedFile::new("pyproject.toml", manifest)?)?;
        }
        cx.files.append(tree)?;
        cx.publish(GraphqlClient { methods })
    }
}
mod models;
use models::Models;
mod codec_layout;
mod layout;
fn base_import(root: Option<(String, String)>, group: bool) -> (String, String) {
    if let Some((path, class)) = root {
        let path = if group {
            if let Some(path) = path.strip_prefix("..groups.") {
                format!(".{path}")
            } else {
                format!(".._mixins{path}")
            }
        } else if let Some(path) = path.strip_prefix("..") {
            format!(".{path}")
        } else {
            format!("._mixins{path}")
        };
        (
            format!("from {path} import {class} as _Base\n"),
            "(_Base)".into(),
        )
    } else {
        (String::new(), String::new())
    }
}
#[cfg(test)]
fn render(
    contract: &GraphqlOperations,
    distribution: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    render_advanced(contract, distribution, style, groups, false, false)
}
mod render;
use render::render_advanced;

#[cfg(test)]
mod tests;

mod source_layout;
