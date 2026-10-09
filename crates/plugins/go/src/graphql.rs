//! Fixed-operation GraphQL clients using the Go standard HTTP library.
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
    const NAME: &'static str = "poolster.go.graphql-client.v1";
}
pub struct Graphql {
    meta: Meta,
    provider: Option<Handle<GraphqlOperations>>,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        style: GraphqlStyle::Flat,
        groups: BTreeMap::new(),
    }
}
impl Graphql {
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
        vec![Requirement::on(self.provider)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Go>) -> Result<()> {
        let contract = cx.inputs.get::<GraphqlOperations>()?;
        let module = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or("graphqlclient");
        let (source, symbols) = render(contract, module, self.style, &self.groups)?;
        cx.files.emit(GeneratedFile::new("graphql.go", source)?)?;
        cx.files.emit(GeneratedFile::new(
            "go.mod",
            format!("module {module}\n\ngo 1.22\n"),
        )?)?;
        cx.files.emit(GeneratedFile::new("README.md", "# Go GraphQL client\n\nQuery and mutation operations generate selection-specific result and variable structs using the Go standard library (Go 1.22+). Create a client with NewClient(endpoint, httpClient). Flat clients expose Client.ReadUser(ctx, variables); idiomatic clients expose Client.Query().ReadUser(ctx, variables) or configured groups. Raw clients expose ReadUser(ctx, client, variables).\n\nOptional[T] distinguishes omitted fields from explicit null: use the zero value, Some(value), or Null[T](). Nullable required fields use pointers. Operation calls return a GraphQLResponse together with an error; GraphQLErrors can accompany partial data, so inspect the response even when an error is returned. HTTP status failures use HTTPError. Context cancellation and custom HTTP clients/headers are supported.\n\nSubscriptions, incremental responses and abstract union selections are unsupported. Enum wire values use strings and custom scalars use json.RawMessage. Decoding uses encoding/json: missing required result fields become zero values and non-null schema constraints are enforced by the GraphQL server, not revalidated by the client. Optional result fields preserve omission versus null. The retained operation document is sent unchanged; callers cannot dynamically choose result fields.\n")?)?;
        cx.publish(GraphqlClient {
            operations: symbols,
            style: self.style,
        })
    }
}
fn ident(value: &str) -> String {
    crate::go_type_name(value)
}
fn render(
    contract: &GraphqlOperations,
    module: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(String, BTreeMap<String, String>)> {
    ensure!(
        !module.is_empty()
            && module
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c))
            && !module.contains(".."),
        "invalid Go module name"
    );
    ensure!(
        !contract.operations.is_empty(),
        "Go GraphQL generation requires operations"
    );
    let package = crate::go_package_name(module.rsplit('/').next().unwrap_or(module));
    ensure!(!package.is_empty(), "invalid Go package name");
    let mut source = format!("// Code generated by Poolster. DO NOT EDIT.\npackage {package}\n");
    source.push_str(include_str!("graphql/runtime.go.tmpl"));
    let mut models = models::Models::default();
    for (name, fields) in &contract.input_objects {
        models.object(&ident(name), fields, true)?;
    }
    let mut symbols = BTreeMap::new();
    for op in &contract.operations {
        ensure!(
            op.kind != GraphqlOperationKind::Subscription,
            "Go GraphQL subscriptions require a separate unsupported transport"
        );
        let name = ident(&op.name);
        ensure!(
            !symbols.values().any(|v| v == &name),
            "GraphQL operation naming collision: {name}"
        );
        ensure!(
            !["Execute", "Query", "Mutation"].contains(&name.as_str()),
            "GraphQL operation conflicts with reserved client method {name}"
        );
        models.object(&format!("{name}Variables"), &op.variables, true)?;
        models.named_type(&format!("{name}Result"), &op.result, false)?;
        symbols.insert(op.name.clone(), name.clone());
        let document = serde_json::to_string(&op.document)?;
        let operation = serde_json::to_string(&op.name)?;
        writeln!(source, "const {name}Document = {document}\nfunc {name}(ctx context.Context, client *Client, variables {name}Variables) (*GraphQLResponse[{name}Result], error) {{ return execute[{name}Result](ctx, client, {operation}, {name}Document, variables) }}").unwrap();
        if style == GraphqlStyle::Flat {
            writeln!(source,"func (c *Client) {name}(ctx context.Context, variables {name}Variables) (*GraphQLResponse[{name}Result], error) {{ return {name}(ctx,c,variables) }}").unwrap();
        }
    }
    if style == GraphqlStyle::Idiomatic {
        let mut groups = groups.clone();
        if groups.is_empty() {
            for op in &contract.operations {
                let group = if op.kind == GraphqlOperationKind::Query {
                    "Query"
                } else {
                    "Mutation"
                };
                groups
                    .entry(group.into())
                    .or_default()
                    .insert(op.name.clone(), op.name.clone());
            }
        }
        let mut group_names = std::collections::BTreeSet::new();
        for (group, methods) in groups {
            let group = ident(&group);
            ensure!(
                group_names.insert(group.clone()),
                "GraphQL group naming collision"
            );
            ensure!(
                group != "Execute",
                "GraphQL group conflicts with client method"
            );
            writeln!(source,"type {group}Client struct {{ client *Client }}\nfunc (c *Client) {group}() *{group}Client {{ return &{group}Client{{client:c}} }}").unwrap();
            let mut names = std::collections::BTreeSet::new();
            for (method, operation) in methods {
                let method = ident(&method);
                ensure!(
                    names.insert(method.clone()),
                    "GraphQL method naming collision"
                );
                let name = symbols
                    .get(&operation)
                    .ok_or_else(|| anyhow::anyhow!("unknown GraphQL operation {operation}"))?;
                writeln!(source,"func (g *{group}Client) {method}(ctx context.Context, variables {name}Variables) (*GraphQLResponse[{name}Result], error) {{ return {name}(ctx,g.client,variables) }}").unwrap();
            }
        }
    } else {
        ensure!(
            groups.is_empty(),
            "GraphQL custom groups require idiomatic style"
        );
    }
    source.push_str(&models.source);
    let mut declarations = std::collections::BTreeSet::new();
    for line in source.lines() {
        let Some(rest) = line
            .strip_prefix("type ")
            .or_else(|| line.strip_prefix("const "))
            .or_else(|| line.strip_prefix("func "))
        else {
            continue;
        };
        if rest.starts_with('(') {
            continue;
        }
        let name = rest.split([' ', '[', '(']).next().unwrap_or_default();
        ensure!(
            declarations.insert(name.to_string()),
            "GraphQL generated declaration collision: {name}"
        );
    }
    Ok((source, symbols))
}
