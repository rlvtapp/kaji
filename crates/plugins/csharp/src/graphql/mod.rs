//! Fixed-operation GraphQL clients using the Go standard HTTP library.
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
    pub operations: BTreeMap<String, String>,
    pub style: GraphqlStyle,
}
impl Contract for GraphqlClient {
    const NAME: &'static str = "poolster.csharp.graphql-client.v1";
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
impl Plugin<crate::CSharp> for Graphql {
    fn kind(&self) -> &'static str {
        "csharp-graphql"
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
    fn generate(&self, cx: &mut PluginContext<'_, crate::CSharp>) -> Result<()> {
        self.render_into(cx)
    }
}

impl Graphql {
    fn render_into<L: poolster_core::engine::Language<Settings = crate::Settings>>(
        &self,
        cx: &mut PluginContext<'_, L>,
    ) -> Result<()> {
        let contract = cx.inputs.get::<GraphqlOperations>()?;
        let namespace =
            crate::dotnet_namespace(cx.settings.package_name.as_deref().unwrap_or("GraphqlSdk"));
        let (files, symbols) = render_advanced(
            contract,
            &namespace,
            self.style,
            &self.groups,
            self.subscriptions,
            false,
        )?;
        for (path, source) in files {
            cx.files.emit(GeneratedFile::new(path, source)?)?;
        }
        cx.files.emit(GeneratedFile::new("GraphqlSdk.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable></PropertyGroup></Project>")?)?;
        cx.files.emit(GeneratedFile::new("README.md", "# C# GraphQL client\n\nSources are organized into Runtime/, Client/, Operations/, Models/, and Groups/. Public APIs stay unchanged across files; wide records use partial declarations with a 128 KiB budget. Indivisible oversized declarations are listed in .poolster/source-layout-diagnostics.json.\n\n.NET 8, HttpClient and CancellationToken. Raw operation functions are static GraphqlOperations methods. Flat style uses GraphqlClient.ReadUserAsync; grouped style uses GraphqlClient.Query.ReadUserAsync or custom groups. Calls return GraphqlResponse<T> containing Data, Errors and Extensions: GraphQL errors and partial data remain explicit; call EnsureSuccess() to throw. HTTP failures throw HttpRequestException. Optional<T> preserves missing versus explicit null; default values omit optional input properties. Selection-specific records reflect fixed operation documents. Custom scalars use JsonElement, enums retain string wire values. Subscriptions are opt-in via .subscriptions(true), returning IAsyncEnumerable typed envelopes using distinct POST SSE connections. Named ScalarCodecs callbacks transform JsonNode values without changing public JsonElement model types. Abstract selections generate typed alternatives when every variant selects a required __typename (aliases supported); unknown or missing discriminators fail decoding. Untagged abstract unions are rejected during generation.\n")?)?;
        cx.publish(GraphqlClient {
            operations: symbols,
            style: self.style,
        })
    }
}
fn ident(value: &str) -> String {
    crate::pascal_case(value)
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
    fn render_into<L: poolster_core::engine::Language<Settings = crate::Settings>>(
        &self,
        cx: &mut PluginContext<'_, L>,
    ) -> Result<()> {
        let value = cx
            .inputs
            .get::<poolster_core::native::GraphqlIncrementalOperations>()?;
        let namespace =
            crate::dotnet_namespace(cx.settings.package_name.as_deref().unwrap_or("GraphqlSdk"));
        let (files, operations) = render_advanced(
            &value.definition,
            &namespace,
            self.style,
            &self.groups,
            false,
            true,
        )?;
        for (path, source) in files {
            cx.files.emit(GeneratedFile::new(path, source)?)?;
        }
        cx.files.emit(GeneratedFile::new("GraphqlSdk.csproj","<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable></PropertyGroup></Project>")?)?;
        cx.files.emit(GeneratedFile::new("README.md","# Incremental C# GraphQL\n\nExplicit deferSpec=20220824 path-based multipart. Calls return IAsyncEnumerable<GraphqlIncrementalSnapshot<Result>> with CancellationToken. Snapshots expose independent partial JsonNode Data, accumulated Errors, original Patches and Complete. DecodeData() only decodes complete error-free selected data. Configure client.ScalarCodecs with named JsonNode callbacks via object initializer; public custom scalars stay JsonElement. Completion does not mean application success. Newer pending/id formats and non-contiguous stream patches fail. JSON fallback supported.\n")?)?;
        cx.publish(GraphqlClient {
            operations,
            style: self.style,
        })
    }
}
macro_rules! incremental_plugin {
    ($language:ty,$kind:literal) => {
        impl Plugin<$language> for GraphqlIncremental {
            fn kind(&self) -> &'static str {
                $kind
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
            fn generate(&self, cx: &mut PluginContext<'_, $language>) -> Result<()> {
                self.render_into(cx)
            }
        }
    };
}
incremental_plugin!(crate::CSharp, "csharp-graphql-incremental");
