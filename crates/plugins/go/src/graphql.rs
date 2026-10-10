//! Fixed-operation GraphQL clients using the Go standard HTTP library.
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
fn render_capabilities(
    contract: &GraphqlOperations,
    module: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
    subscriptions: bool,
    mappings: &BTreeMap<String, GraphqlScalarMapping>,
    incremental: Option<&GraphqlIncrementalOperations>,
) -> Result<(BTreeMap<String, String>, BTreeMap<String, String>)> {
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
    let header = format!("// Code generated by Poolster. DO NOT EDIT.\npackage {package}\n");
    let runtime = include_str!("graphql/runtime.go.tmpl");
    let declarations_start = runtime.find("// Optional").unwrap();
    let client_start = runtime.find("type Client struct").unwrap();
    let transport_start = runtime.find("func execute[").unwrap();
    let mut files = BTreeMap::new();
    files.insert(
        "graphql_runtime.go".into(),
        header.clone()
            + "import (\"encoding/json\";\"fmt\")\n"
            + &runtime[declarations_start..client_start],
    );
    files.insert(
        "graphql_client.go".into(),
        header.clone() + "import \"net/http\"\n" + &runtime[client_start..transport_start],
    );
    files.insert(
        "graphql_transport.go".into(),
        header.clone() + &runtime[..declarations_start] + &runtime[transport_start..],
    );
    files.insert(
        "graphql_codecs.go".into(),
        header.clone() + include_str!("graphql/codecs.go.tmpl"),
    );
    files.insert(
        "graphql_sse.go".into(),
        header.clone() + include_str!("graphql/sse.go.tmpl"),
    );
    files.insert(
        "graphql_incremental.go".into(),
        header.clone() + include_str!("graphql/incremental.go.tmpl"),
    );
    let input_shapes = contract
        .input_objects
        .iter()
        .map(|(name, fields)| (name.clone(), graphql_scalar_fields(fields)))
        .collect::<BTreeMap<_, _>>();
    let shapes = serde_json::to_string(&input_shapes)?;
    files.insert("graphql_shapes.go".into(),header.clone()+"import \"encoding/json\"\n"+&format!("var graphqlInputShapes = func()map[string]*graphqlScalarShape{{ var shapes map[string]*graphqlScalarShape;if err:=json.Unmarshal([]byte({:?}),&shapes);err!=nil{{panic(err)}};return shapes}}()\n",shapes));
    let mut models = models::Models::default();
    models.mappings = mappings.clone();
    for (name, fields) in &contract.input_objects {
        models.object(&ident(name), fields, true)?;
    }
    let mut symbols = BTreeMap::new();
    for op in &contract.operations {
        ensure!(
            op.kind != GraphqlOperationKind::Subscription
                || (subscriptions && incremental.is_none()),
            "Go GraphQL subscriptions require .subscriptions(); incremental subscriptions are unsupported"
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
        let operation_start = source.len();
        let input_shape = serde_json::to_string(&graphql_scalar_fields(&op.variables))?;
        let result_shape = serde_json::to_string(&graphql_scalar_shape(&op.result))?;
        let response = if incremental.is_some() {
            format!("IncrementalStream[{name}Result]")
        } else if op.kind == GraphqlOperationKind::Subscription {
            format!("Subscription[{name}Result]")
        } else {
            format!("GraphQLResponse[{name}Result]")
        };
        let execute = if incremental.is_some() {
            "incrementalExecute"
        } else if op.kind == GraphqlOperationKind::Subscription {
            "subscribe"
        } else {
            "executeShaped"
        };
        let selections = incremental.and_then(|value| value.selections.get(&op.name));
        let extra = if incremental.is_some() {
            format!(
                ", {}",
                serde_json::to_string(&serde_json::to_string(&selections)?)?
            )
        } else {
            String::new()
        };
        writeln!(source,"const {name}Document = {document}\nvar {name}InputShape=parseScalarShape({input_shape:?})\nvar {name}ResultShape=parseScalarShape({result_shape:?})\nfunc {name}(ctx context.Context, client *Client, variables {name}Variables) (*{response},error){{return {execute}[{name}Result](ctx,client,{operation},{name}Document,variables,{name}InputShape,{name}ResultShape{extra})}}\n").unwrap();
        if style == GraphqlStyle::Flat {
            writeln!(source,"func(c *Client){name}(ctx context.Context,variables {name}Variables)(*{response},error){{return {name}(ctx,c,variables)}}\n").unwrap();
        }
        files.insert(
            filename("operation", &name),
            header.clone() + "import \"context\"\n" + &source[operation_start..],
        );
    }
    if style == GraphqlStyle::Idiomatic {
        let mut groups = groups.clone();
        if groups.is_empty() {
            for op in &contract.operations {
                let group = match op.kind {
                    GraphqlOperationKind::Query => "Query",
                    GraphqlOperationKind::Mutation => "Mutation",
                    GraphqlOperationKind::Subscription => "Subscription",
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
            let group_start = source.len();
            writeln!(source,"type {group}Client struct {{ client *Client }}\nfunc (c *Client) {group}() *{group}Client {{ return &{group}Client{{client:c}} }}").unwrap();
            files.insert(
                filename("group", &group),
                header.clone() + &source[group_start..],
            );
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
                let method_start = source.len();
                let kind = contract
                    .operations
                    .iter()
                    .find(|op| op.name == operation)
                    .unwrap()
                    .kind;
                let response = if incremental.is_some() {
                    format!("IncrementalStream[{name}Result]")
                } else if kind == GraphqlOperationKind::Subscription {
                    format!("Subscription[{name}Result]")
                } else {
                    format!("GraphQLResponse[{name}Result]")
                };
                writeln!(source,"func(g *{group}Client){method}(ctx context.Context,variables {name}Variables)(*{response},error){{return {name}(ctx,g.client,variables)}}\n").unwrap();
                files.insert(
                    filename("group_method", &serde_json::to_string(&(&group, &method))?),
                    header.clone() + "import \"context\"\n" + &source[method_start..],
                );
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
    for (path, body) in models.files {
        let imports = [
            ("encoding/json", "json."),
            ("fmt", "fmt."),
            ("time", "time."),
        ]
        .into_iter()
        .filter(|(_, needle)| body.contains(needle))
        .map(|(package, _)| format!("{package:?}"))
        .collect::<Vec<_>>();
        let imports = if imports.is_empty() {
            String::new()
        } else {
            format!("import ({})\n", imports.join(";"))
        };
        files.insert(path, header.clone() + &imports + &body);
    }
    let oversized=files.iter().filter(|(_,body)|body.len()>128*1024).map(|(path,body)|serde_json::json!({"path":path,"bytes":body.len(),"max_file_bytes":128*1024,"reason":"Atomic Go GraphQL declaration exceeds the source grouping budget; source retained intact."})).collect::<Vec<_>>();
    if !oversized.is_empty() {
        files.insert(
            ".poolster/source-layout-diagnostics.json".into(),
            serde_json::to_string_pretty(&oversized)?,
        );
    }
    Ok((files, symbols))
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
