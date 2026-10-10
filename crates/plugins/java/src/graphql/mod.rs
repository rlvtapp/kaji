//! Native GraphQL clients consume selection contracts, never the HTTP AST.
mod emission;
mod facades;
use emission::*;
mod layout;
mod models;
#[cfg(test)]
mod tests;
use crate::Java;
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
    const NAME: &'static str = "poolster.java.graphql-client.v1";
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
    pub fn subscriptions(mut self, enabled: bool) -> Self {
        self.subscriptions = enabled;
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
impl Plugin<Java> for Graphql {
    fn kind(&self) -> &'static str {
        "java-graphql"
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
    fn generate(&self, cx: &mut PluginContext<'_, Java>) -> Result<()> {
        let (mut tree, methods) = render_advanced(
            cx.inputs.get::<GraphqlOperations>()?,
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("io.poolster.graphql"),
            self.style,
            &self.groups,
            self.subscriptions,
            false,
        )?;
        if let Some(version) = &cx.common.package_version {
            for path in ["pom.xml", "build.gradle"] {
                let content = tree
                    .get(path)
                    .unwrap()
                    .replace("0.0.0", &crate::package_version(version));
                tree.replace(GeneratedFile::new(path, content)?)?;
            }
        }
        cx.files.append(tree)?;
        cx.publish(GraphqlClient { methods })
    }
}

/// Explicit path-based incremental execution using the same Java client call styles.
pub struct GraphqlIncremental {
    meta: Meta,
    provider: Option<Handle<poolster_core::native::GraphqlIncrementalOperations>>,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
pub fn graphql_incremental(
    provider: Option<Handle<poolster_core::native::GraphqlIncrementalOperations>>,
) -> GraphqlIncremental {
    GraphqlIncremental {
        meta: Meta::new(),
        provider,
        style: GraphqlStyle::Idiomatic,
        groups: BTreeMap::new(),
    }
}
impl GraphqlIncremental {
    pub fn input(
        mut self,
        input: Handle<poolster_core::native::GraphqlIncrementalOperations>,
    ) -> Self {
        self.provider = Some(input);
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
    pub fn handle(&self) -> Handle<GraphqlClient> {
        self.meta.handle()
    }
}
impl Plugin<Java> for GraphqlIncremental {
    fn kind(&self) -> &'static str {
        "java-graphql-incremental"
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
    fn generate(&self, cx: &mut PluginContext<'_, Java>) -> Result<()> {
        let value = cx
            .inputs
            .get::<poolster_core::native::GraphqlIncrementalOperations>()?;
        let (mut tree, methods) = render_advanced(
            &value.definition,
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("io.poolster.graphql"),
            self.style,
            &self.groups,
            false,
            true,
        )?;
        tree.replace(GeneratedFile::new("README.md","# Incremental Java GraphQL

Explicit deferSpec=20220824 path-based multipart transport. Calls return closeable Stream<IncrementalSnapshot<Result>>; use try-with-resources. Snapshots expose independent partial JsonNode data, accumulated errors, original patches and completion. decodeData() only decodes completed error-free selected data; completion does not imply application success. HttpTransport accepts named ScalarCodec callbacks transforming JsonNode values; defaults preserve raw custom scalar JSON. Callbacks apply once to each emitted snapshot; aggregate wire data is retained separately. Newer pending/id dialects, non-contiguous stream patches and mutations/subscriptions are unsupported for this capability. Ordinary JSON fallback is supported.
")?)?;
        cx.files.append(tree)?;
        cx.publish(GraphqlClient { methods })
    }
}
