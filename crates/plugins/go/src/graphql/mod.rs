//! Fixed-operation GraphQL clients using the Go standard HTTP library.
mod render;
use render::render_capabilities;
mod models;
mod scalars;
pub use scalars::GraphqlScalarMapping;
#[cfg(test)]
mod capabilities;
#[cfg(test)]
mod tests;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{
        GraphqlIncrementalOperations, GraphqlOperationKind, GraphqlOperations,
        graphql_scalar_fields, graphql_scalar_shape,
    },
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
    const NAME: &'static str = "poolster.go.graphql-client.v1";
}
pub struct Graphql {
    meta: Meta,
    provider: Option<Handle<GraphqlOperations>>,
    incremental_provider: Option<Handle<GraphqlIncrementalOperations>>,
    incremental_mode: bool,
    subscriptions: bool,
    scalars: BTreeMap<String, GraphqlScalarMapping>,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        incremental_provider: None,
        incremental_mode: false,
        subscriptions: false,
        scalars: BTreeMap::new(),
        style: GraphqlStyle::Flat,
        groups: BTreeMap::new(),
    }
}
pub fn graphql_incremental(provider: Option<Handle<GraphqlIncrementalOperations>>) -> Graphql {
    let mut generator = graphql(None);
    generator.incremental_provider = provider;
    generator.incremental_mode = true;
    generator
}
impl Graphql {
    pub fn subscriptions(mut self) -> Self {
        self.subscriptions = true;
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
impl Plugin<crate::Go> for Graphql {
    fn kind(&self) -> &'static str {
        "go-graphql"
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
    fn generate(&self, cx: &mut PluginContext<'_, crate::Go>) -> Result<()> {
        let incremental = if self.incremental_mode {
            Some(cx.inputs.get::<GraphqlIncrementalOperations>()?)
        } else {
            None
        };
        let contract = if let Some(incremental) = incremental {
            &incremental.definition
        } else {
            cx.inputs.get::<GraphqlOperations>()?
        };
        scalars::validate(&self.scalars)?;
        let module = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or("graphqlclient");
        let (files, symbols) = render_capabilities(
            contract,
            module,
            self.style,
            &self.groups,
            self.subscriptions,
            &self.scalars,
            incremental,
        )?;
        for (path, source) in files {
            cx.files.emit(GeneratedFile::new(path, source)?)?;
        }
        cx.files.emit(GeneratedFile::new(
            "go.mod",
            format!("module {module}\n\ngo 1.22\n"),
        )?)?;
        cx.files.emit(GeneratedFile::new("README.md", "# Go GraphQL client\n\nSources stay in one Go package with graphql_runtime.go, graphql_client.go, graphql_transport.go and per-operation/model/group files. Public imports and methods stay unchanged. Atomic declarations exceeding the 128 KiB budget are retained with .poolster/source-layout-diagnostics.json.\n\nQuery and mutation operations generate selection-specific result and variable structs using the Go standard library (Go 1.22+). Create a client with NewClient(endpoint, httpClient). Flat clients expose Client.ReadUser(ctx, variables); idiomatic clients expose Client.Query().ReadUser(ctx, variables) or configured groups. Raw clients expose ReadUser(ctx, client, variables).\n\nOptional[T] distinguishes omitted fields from explicit null: use the zero value, Some(value), or Null[T](). Nullable required fields use pointers. Operation calls return a GraphQLResponse together with an error; GraphQLErrors can accompany partial data, so inspect the response even when an error is returned. HTTP status failures use HTTPError. Context cancellation and custom HTTP clients/headers are supported.\n\nEnabled subscriptions use graphql-sse distinct connections with Next/Close and context cancellation. Incremental packages use multipart/mixed deferSpec20220824 snapshots and final typed results. ScalarCodecs and TypedScalarCodec register selection-aware runtime callbacks, with optional generated Go scalar type mappings. Abstract selections generate typed alternatives when every variant selects a required __typename (aliases supported); unknown or missing discriminators fail decoding. Untagged abstract unions are rejected during generation. Enum wire values use strings and custom scalars use json.RawMessage. Decoding uses encoding/json: missing required result fields become zero values and non-null schema constraints are enforced by the GraphQL server, not revalidated by the client. Optional result fields preserve omission versus null. The retained operation document is sent unchanged; callers cannot dynamically choose result fields.\n")?)?;
        cx.publish(GraphqlClient {
            operations: symbols,
            style: self.style,
        })
    }
}
fn ident(value: &str) -> String {
    crate::go_type_name(value)
}
#[cfg(test)]
fn render(
    contract: &GraphqlOperations,
    module: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(BTreeMap<String, String>, BTreeMap<String, String>)> {
    render_capabilities(
        contract,
        module,
        style,
        groups,
        false,
        &BTreeMap::new(),
        None,
    )
}

fn filename(category: &str, name: &str) -> String {
    let prefix = ident(name)
        .chars()
        .take(80)
        .collect::<String>()
        .to_ascii_lowercase();
    let hash = name.as_bytes().iter().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    format!("graphql_{category}_{prefix}_{hash:016x}.go")
}
