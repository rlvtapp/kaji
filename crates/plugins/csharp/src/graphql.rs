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
    const NAME: &'static str = "poolster.csharp.graphql-client.v1";
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
impl Plugin<crate::DotNet> for Graphql {
    fn kind(&self) -> &'static str {
        "dotnet-graphql"
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
    fn generate(&self, cx: &mut PluginContext<'_, crate::DotNet>) -> Result<()> {
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
        let (source, symbols) = render(contract, &namespace, self.style, &self.groups)?;
        cx.files.emit(GeneratedFile::new("Graphql.cs", source)?)?;
        cx.files.emit(GeneratedFile::new("GraphqlSdk.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable></PropertyGroup></Project>")?)?;
        cx.files.emit(GeneratedFile::new("README.md", "# C# GraphQL client\n\n.NET 8, HttpClient and CancellationToken. Raw operation functions are static GraphqlOperations methods. Flat style uses GraphqlClient.ReadUserAsync; grouped style uses GraphqlClient.Query.ReadUserAsync or custom groups. Calls return GraphqlResponse<T> containing Data, Errors and Extensions: GraphQL errors and partial data remain explicit; call EnsureSuccess() to throw. HTTP failures throw HttpRequestException. Optional<T> preserves missing versus explicit null; default values omit optional input properties. Selection-specific records reflect fixed operation documents. Custom scalars use JsonElement, enums retain string wire values. Subscriptions, incremental responses and abstract union selections are unsupported.\n")?)?;
        cx.publish(GraphqlClient {
            operations: symbols,
            style: self.style,
        })
    }
}
fn ident(value: &str) -> String {
    crate::pascal_case(value)
}
fn render(
    contract: &GraphqlOperations,
    namespace: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(String, BTreeMap<String, String>)> {
    ensure!(
        !contract.operations.is_empty(),
        "C# GraphQL generation requires operations"
    );
    let mut source =
        format!("// Generated by Poolster.\n#nullable enable\nnamespace {namespace};\n");
    // C# using directives precede all declarations.
    source = source.replace(&format!("namespace {namespace};\n"), "");
    let runtime = include_str!("graphql/runtime.cs.tmpl");
    let split = runtime.find("public readonly").unwrap();
    source.push_str(&runtime[..split]);
    source.push_str(&format!("namespace {namespace};\n"));
    source.push_str(&runtime[split..]);
    let mut models = models::Models::default();
    for (name, fields) in &contract.input_objects {
        models.object(&ident(name), fields)?;
    }
    let mut symbols = BTreeMap::new();
    let mut functions = String::from("public static class GraphqlOperations {\n");
    for op in &contract.operations {
        ensure!(
            op.kind != GraphqlOperationKind::Subscription,
            "C# GraphQL subscriptions require a separate unsupported transport"
        );
        let name = ident(&op.name);
        ensure!(
            !symbols.values().any(|v| v == &name),
            "GraphQL operation naming collision {name}"
        );
        ensure!(
            ![
                "Execute",
                "Query",
                "Mutation",
                "GraphqlClient",
                "Json",
                "Equals",
                "GetHashCode",
                "ToString"
            ]
            .contains(&name.as_str()),
            "GraphQL operation conflicts with runtime {name}"
        );
        models.object(&format!("{name}Variables"), &op.variables)?;
        let result = models.ty(&format!("{name}Result"), &op.result)?;
        let args = if op.variables.is_empty() {
            String::new()
        } else {
            format!("{name}Variables variables, ")
        };
        let variables = if op.variables.is_empty() {
            "new {}"
        } else {
            "variables"
        };
        writeln!(functions,"public static Task<GraphqlResponse<{result}>> {name}Async(GraphqlClient client, {args}CancellationToken cancellationToken = default) => client.ExecuteAsync<{result}>({}, {}, {variables}, cancellationToken);",serde_json::to_string(&op.name)?,serde_json::to_string(&op.document)?).unwrap();
        if style == GraphqlStyle::Flat {
            writeln!(source,"public sealed partial class GraphqlClient {{ public Task<GraphqlResponse<{result}>> {name}Async({args}CancellationToken cancellationToken = default) => GraphqlOperations.{name}Async(this, {}cancellationToken); }}",if op.variables.is_empty(){""}else{"variables, "}).unwrap();
        }
        symbols.insert(op.name.clone(), name);
    }
    if style == GraphqlStyle::Idiomatic {
        let mut groups = groups.clone();
        if groups.is_empty() {
            for op in &contract.operations {
                groups
                    .entry(
                        if op.kind == GraphqlOperationKind::Query {
                            "Query"
                        } else {
                            "Mutation"
                        }
                        .into(),
                    )
                    .or_default()
                    .insert(op.name.clone(), op.name.clone());
            }
        }
        let mut names = std::collections::BTreeSet::new();
        for (group, methods) in groups {
            let group = ident(&group);
            ensure!(
                names.insert(group.clone())
                    && ![
                        "ExecuteAsync",
                        "Json",
                        "GraphqlClient",
                        "Equals",
                        "GetHashCode",
                        "ToString"
                    ]
                    .contains(&group.as_str()),
                "GraphQL group naming collision {group}"
            );
            writeln!(source,"public sealed partial class GraphqlClient {{ public {group}Group {group} => new(this); }}\npublic sealed class {group}Group(GraphqlClient client) {{").unwrap();
            let mut names = std::collections::BTreeSet::new();
            for (method, operation) in methods {
                let method = ident(&method);
                ensure!(
                    names.insert(method.clone()) && method != format!("{group}Group"),
                    "GraphQL method naming collision"
                );
                let name = symbols
                    .get(&operation)
                    .ok_or_else(|| anyhow::anyhow!("unknown GraphQL operation {operation}"))?;
                let op = contract
                    .operations
                    .iter()
                    .find(|op| op.name == operation)
                    .unwrap();
                let result = format!("{name}Result");
                let args = if op.variables.is_empty() {
                    String::new()
                } else {
                    format!("{name}Variables variables, ")
                };
                writeln!(source,"public Task<GraphqlResponse<{result}>> {method}Async({args}CancellationToken cancellationToken = default) => GraphqlOperations.{name}Async(client, {}cancellationToken);",if op.variables.is_empty(){""}else{"variables, "}).unwrap();
            }
            source.push_str("}\n");
        }
    } else {
        ensure!(
            groups.is_empty(),
            "GraphQL custom groups require idiomatic style"
        );
    }
    functions.push_str("}\n");
    source.push_str(&functions);
    source.push_str(&models.source);
    let mut declarations = std::collections::BTreeSet::new();
    for line in source.lines() {
        if line.starts_with("public sealed partial class GraphqlClient") {
            continue;
        }
        for prefix in [
            "public sealed record ",
            "public sealed class ",
            "public static class ",
        ] {
            if let Some(rest) = line.strip_prefix(prefix) {
                let name = rest.split([' ', '<', '(']).next().unwrap_or_default();
                ensure!(
                    declarations.insert(name.to_owned()),
                    "GraphQL generated declaration collision {name}"
                );
            }
        }
    }
    Ok((source, symbols))
}
