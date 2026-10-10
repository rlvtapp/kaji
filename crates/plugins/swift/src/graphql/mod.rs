//! Fixed-operation GraphQL clients using Foundation URLSession.
mod emission;
use emission::*;
mod models;
#[cfg(test)]
mod tests;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{GraphqlOperationKind, GraphqlOperations},
};
use std::{collections::BTreeMap, fmt::Write};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphqlStyle {
    Raw,
    Flat,
    Idiomatic,
}
#[derive(Clone, Debug)]
pub struct GraphqlClient {
    pub methods: BTreeMap<String, String>,
    pub style: GraphqlStyle,
}
impl Contract for GraphqlClient {
    const NAME: &'static str = "poolster.swift.graphql-client.v1";
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
        style: GraphqlStyle::Flat,
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
impl Plugin<crate::Swift> for Graphql {
    fn kind(&self) -> &'static str {
        "swift-graphql"
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
    fn generate(&self, cx: &mut PluginContext<'_, crate::Swift>) -> Result<()> {
        let contract = cx.inputs.get::<GraphqlOperations>()?;
        let module = ident(cx.settings.package_name.as_deref().unwrap_or("GraphqlSdk"));
        let (files, symbols) = render_advanced(
            contract,
            self.style,
            &self.groups,
            self.subscriptions,
            false,
        )?;
        for (path, source) in files {
            let path = if path.ends_with(".swift") {
                format!("Sources/{module}/{path}")
            } else {
                path
            };
            cx.files.emit(GeneratedFile::new(path, source)?)?;
        }
        cx.files.emit(GeneratedFile::new("Package.swift", format!("// swift-tools-version: 5.9\nimport PackageDescription\nlet package = Package(name: \"{module}\", platforms: [.macOS(.v12), .iOS(.v15)], products: [.library(name: \"{module}\", targets: [\"{module}\"])], targets: [.target(name: \"{module}\")])\n"))?)?;
        cx.files.emit(GeneratedFile::new("README.md", "# Swift GraphQL client\n\nSources/<module>/ contains Runtime/, Client/, Operations/, Models/, and Groups/. Swift Package Manager discovers all sources. Each operation and model has its own file; indivisible models larger than the 128 KiB grouping budget are retained and listed in .poolster/source-layout-diagnostics.json.\n\nSwift 5.9+, Foundation URLSession async transport. GraphqlClient takes endpoint, session and headers. Raw calls are free functions; flat style exposes client.readUser(variables:); grouped style exposes client.query.readUser(variables:) or custom groups. No-variable calls omit variables. Results are selection-specific Codable classes with explicit GraphqlResponse data/errors/extensions; ensureSuccess() throws GraphqlFailure for GraphQL errors while preserving partial data on the envelope. HTTP failures throw GraphqlHTTPError. GraphqlField<T> distinguishes omitted, null, and value for optional input/result fields. Custom scalars use GraphqlJSON and enums use strings. Subscriptions are opt-in via .subscriptions(true) and return AsyncThrowingStream typed envelopes over distinct POST SSE connections. Configure named GraphqlScalarCodec callbacks via client scalarCodecs while keeping custom scalar model values as GraphqlJSON. Abstract selections generate Codable enums with associated payloads when every variant selects a required __typename (aliases supported); unknown or missing discriminators fail decoding. Untagged abstract unions are rejected during generation. Fixed operation documents are retained and sent unchanged. Required presence is validated while decoding; nonnull reference properties remain subject to Codable decoding.\n")?)?;
        cx.publish(GraphqlClient {
            methods: symbols,
            style: self.style,
        })
    }
}
fn ident(value: &str) -> String {
    crate::type_name(value)
}
fn member(value: &str) -> String {
    let id = ident(value);
    let mut chars = id.chars();
    let value = chars
        .next()
        .map(|c| c.to_ascii_lowercase().to_string() + chars.as_str())
        .unwrap_or_else(|| "value".into());
    format!("`{value}`")
}
fn literal(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                write!(out, "\\u{{{:x}}}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

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
        style: GraphqlStyle::Flat,
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
    pub fn groups(mut self, groups: BTreeMap<String, BTreeMap<String, String>>) -> Self {
        self.groups = groups;
        self
    }
    pub fn group(
        mut self,
        group: impl Into<String>,
        method: impl Into<String>,
        op: impl Into<String>,
    ) -> Self {
        self.groups
            .entry(group.into())
            .or_default()
            .insert(method.into(), op.into());
        self
    }
    pub fn handle(&self) -> Handle<GraphqlClient> {
        self.meta.handle()
    }
}
impl Plugin<crate::Swift> for GraphqlIncremental {
    fn kind(&self) -> &'static str {
        "swift-graphql-incremental"
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
    fn generate(&self, cx: &mut PluginContext<'_, crate::Swift>) -> Result<()> {
        let contract = cx
            .inputs
            .get::<poolster_core::native::GraphqlIncrementalOperations>()?;
        let module = ident(cx.settings.package_name.as_deref().unwrap_or("GraphqlSdk"));
        let (files, methods) =
            render_advanced(&contract.definition, self.style, &self.groups, false, true)?;
        for (path, source) in files {
            let path = if path.ends_with(".swift") {
                format!("Sources/{module}/{path}")
            } else {
                path
            };
            cx.files.emit(GeneratedFile::new(path, source)?)?;
        }
        cx.files.emit(GeneratedFile::new("Package.swift",format!("// swift-tools-version: 5.9\nimport PackageDescription\nlet package=Package(name:\"{module}\",platforms:[.macOS(.v12),.iOS(.v15)],products:[.library(name:\"{module}\",targets:[\"{module}\"])],targets:[.target(name:\"{module}\")])\n"))?)?;
        cx.files.emit(GeneratedFile::new("README.md","# Incremental Swift GraphQL\n\nExplicit path-based deferSpec=20220824 multipart. Calls return AsyncThrowingStream<GraphqlIncrementalSnapshot<Result>,Error>. Snapshots contain independent partial GraphqlJSON data, accumulated errors, original patches and complete; decodeData() decodes completed error-free selected data. Completion is distinct from application success. Configure named GraphqlScalarCodec callbacks in GraphqlClient scalarCodecs; custom scalar models stay GraphqlJSON. Cancellation closes the distinct streaming connection. Newer pending/id dialects and non-contiguous stream patches fail. JSON fallback is supported when the server delivers all selections at once.\n")?)?;
        cx.publish(GraphqlClient {
            methods,
            style: self.style,
        })
    }
}
