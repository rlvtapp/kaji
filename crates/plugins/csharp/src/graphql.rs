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
#[cfg(test)]
fn render(
    contract: &GraphqlOperations,
    namespace: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(BTreeMap<String, String>, BTreeMap<String, String>)> {
    render_advanced(contract, namespace, style, groups, false, false)
}
fn render_advanced(
    contract: &GraphqlOperations,
    namespace: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
    subscriptions: bool,
    incremental: bool,
) -> Result<(BTreeMap<String, String>, BTreeMap<String, String>)> {
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
    let header = format!(
        "// Generated by Poolster.\n#nullable enable\n{}namespace {namespace};\n",
        &runtime[..split]
    );
    let mut files = BTreeMap::new();
    let client_start = runtime
        .find("public sealed partial class GraphqlClient")
        .unwrap();
    files.insert(
        "Runtime/GraphqlRuntime.cs".into(),
        header.clone() + &runtime[split..client_start],
    );
    files.insert(
        "Client/GraphqlClient.cs".into(),
        header.clone() + &runtime[client_start..],
    );
    files.insert(
        "Runtime/GraphqlAdvanced.cs".into(),
        header.clone() + include_str!("graphql/advanced.cs.tmpl"),
    );
    let mut models = models::Models::default();
    for (name, fields) in &contract.input_objects {
        models.object(&ident(name), fields)?;
    }
    let mut symbols = BTreeMap::new();
    let mut functions = String::from("public static class GraphqlOperations {\n");
    for op in &contract.operations {
        ensure!(
            op.kind != GraphqlOperationKind::Subscription || subscriptions,
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
        let return_type = if incremental {
            format!("IAsyncEnumerable<GraphqlIncrementalSnapshot<{result}>>")
        } else if op.kind == GraphqlOperationKind::Subscription {
            format!("IAsyncEnumerable<GraphqlResponse<{result}>>")
        } else {
            format!("Task<GraphqlResponse<{result}>>")
        };
        let execution = if incremental {
            "IncrementalAsync"
        } else if op.kind == GraphqlOperationKind::Subscription {
            "SubscribeAsync"
        } else {
            "ExecuteWithScalarsAsync"
        };
        let variable_shape = serde_json::to_string(&serde_json::to_string(
            &poolster_core::native::graphql_scalar_fields(&op.variables),
        )?)?;
        let result_shape = serde_json::to_string(&serde_json::to_string(
            &poolster_core::native::graphql_scalar_shape(&op.result),
        )?)?;
        let inputs: BTreeMap<_, _> = contract
            .input_objects
            .iter()
            .map(|(name, fields)| (name, poolster_core::native::graphql_scalar_fields(fields)))
            .collect();
        let inputs = serde_json::to_string(&serde_json::to_string(&inputs)?)?;
        let functions_start = functions.len();
        let source_start = source.len();
        writeln!(functions,"public static {return_type} {name}Async(GraphqlClient client, {args}CancellationToken cancellationToken = default) => client.{execution}<{result}>({}, {}, {variables}, cancellationToken,{variable_shape},{result_shape},{inputs});",serde_json::to_string(&op.name)?,serde_json::to_string(&op.document)?).unwrap();
        if style == GraphqlStyle::Flat {
            writeln!(source,"public sealed partial class GraphqlClient {{ public {return_type} {name}Async({args}CancellationToken cancellationToken = default) => GraphqlOperations.{name}Async(this, {}cancellationToken); }}",if op.variables.is_empty(){""}else{"variables, "}).unwrap();
        }
        let operation = header.clone()
            + "public static partial class GraphqlOperations {\n"
            + &functions[functions_start..]
            + "}\n"
            + &source[source_start..];
        files.insert(
            format!("Operations/{}", crate::bounded_filename(&name, 0, "cs")),
            operation,
        );
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
                        } else if op.kind == GraphqlOperationKind::Subscription {
                            "Subscription"
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
            files.insert(format!("Groups/{}/Client.cs",crate::bounded_filename(&group,0,"cs").trim_end_matches(".cs")),header.clone()+&format!("public sealed partial class GraphqlClient {{ public {group}Group {group} => new(this); }}\npublic sealed partial class {group}Group {{ private readonly GraphqlClient client; public {group}Group(GraphqlClient client) {{ this.client = client; }} }}\n"));
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
                let return_type = if incremental {
                    format!("IAsyncEnumerable<GraphqlIncrementalSnapshot<{result}>>")
                } else if op.kind == GraphqlOperationKind::Subscription {
                    format!("IAsyncEnumerable<GraphqlResponse<{result}>>")
                } else {
                    format!("Task<GraphqlResponse<{result}>>")
                };
                let method_start = source.len();
                writeln!(source,"public {return_type} {method}Async({args}CancellationToken cancellationToken = default) => GraphqlOperations.{name}Async(client, {}cancellationToken);",if op.variables.is_empty(){""}else{"variables, "}).unwrap();
                files.insert(
                    format!(
                        "Groups/{}/{}",
                        crate::bounded_filename(&group, 0, "cs").trim_end_matches(".cs"),
                        crate::bounded_filename(&method, 0, "cs")
                    ),
                    header.clone()
                        + &format!("public sealed partial class {group}Group {{\n")
                        + &source[method_start..]
                        + "}\n",
                );
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
    for (path, body) in models.files {
        files.insert(path, header.clone() + &body);
    }
    let oversized = files.iter().filter(|(_,body)|body.len()>128*1024).map(|(path,body)|serde_json::json!({"path":path,"bytes":body.len(),"max_file_bytes":128*1024,"reason":"Atomic GraphQL declaration exceeds the source grouping budget; source retained intact."})).collect::<Vec<_>>();
    if !oversized.is_empty() {
        files.insert(
            ".poolster/source-layout-diagnostics.json".into(),
            serde_json::to_string_pretty(&oversized)?,
        );
    }
    Ok((files, symbols))
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
incremental_plugin!(crate::DotNet, "dotnet-graphql-incremental");
