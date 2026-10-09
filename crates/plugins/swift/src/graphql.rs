//! Fixed-operation GraphQL clients using Foundation URLSession.
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
        let (source, symbols) = render(contract, self.style, &self.groups)?;
        cx.files.emit(GeneratedFile::new(
            format!("Sources/{module}/Graphql.swift"),
            source,
        )?)?;
        cx.files.emit(GeneratedFile::new("Package.swift", format!("// swift-tools-version: 5.9\nimport PackageDescription\nlet package = Package(name: \"{module}\", platforms: [.macOS(.v12), .iOS(.v15)], products: [.library(name: \"{module}\", targets: [\"{module}\"])], targets: [.target(name: \"{module}\")])\n"))?)?;
        cx.files.emit(GeneratedFile::new("README.md", "# Swift GraphQL client\n\nSwift 5.9+, Foundation URLSession async transport. GraphqlClient takes endpoint, session and headers. Raw calls are free functions; flat style exposes client.readUser(variables:); grouped style exposes client.query.readUser(variables:) or custom groups. No-variable calls omit variables. Results are selection-specific Codable classes with explicit GraphqlResponse data/errors/extensions; ensureSuccess() throws GraphqlFailure for GraphQL errors while preserving partial data on the envelope. HTTP failures throw GraphqlHTTPError. GraphqlField<T> distinguishes omitted, null, and value for optional input/result fields. Custom scalars use GraphqlJSON and enums use strings. Subscription, incremental and abstract union selections are unsupported. Fixed operation documents are retained and sent unchanged. Required presence is validated while decoding; nonnull reference properties remain subject to Codable decoding.\n")?)?;
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

fn render(
    contract: &GraphqlOperations,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(String, BTreeMap<String, String>)> {
    ensure!(
        !contract.operations.is_empty(),
        "Swift GraphQL generation requires operations"
    );
    let mut source = String::from("// Generated by Poolster.\n");
    source.push_str(include_str!("graphql/runtime.swift.tmpl"));
    let mut models = models::Models::default();
    for (name, fields) in &contract.input_objects {
        models.object(&ident(name), fields)?;
    }
    let mut symbols = BTreeMap::new();
    for op in &contract.operations {
        ensure!(
            op.kind != GraphqlOperationKind::Subscription,
            "Swift GraphQL subscriptions require a separate unsupported transport"
        );
        let name = ident(&op.name);
        ensure!(
            !symbols.values().any(|v| v == &name),
            "GraphQL operation naming collision {name}"
        );
        ensure!(
            !["Execute", "GraphqlClient", "Query", "Mutation"].contains(&name.as_str()),
            "GraphQL operation conflicts with runtime {name}"
        );
        models.object(&format!("{name}Variables"), &op.variables)?;
        let result = models.ty(&format!("{name}Result"), &op.result)?;
        let args = if op.variables.is_empty() {
            String::new()
        } else {
            format!(", variables: {name}Variables")
        };
        let vars = if op.variables.is_empty() {
            "GraphqlEmptyVariables()"
        } else {
            "variables"
        };
        let method = member(&op.name);
        writeln!(source,"public func {method}(client: GraphqlClient{args}) async throws -> GraphqlResponse<{result}> {{ try await client.execute(operationName:{},document:{},variables:{vars},resultType:{result}.self) }}",literal(&op.name),literal(&op.document)).unwrap();
        if style == GraphqlStyle::Flat {
            let args = args.trim_start_matches(", ");
            writeln!(source,"extension GraphqlClient {{ public func {method}({args}) async throws -> GraphqlResponse<{result}> {{ try await self.execute(operationName:{},document:{},variables:{vars},resultType:{result}.self) }} }}",literal(&op.name),literal(&op.document)).unwrap();
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
                            "query"
                        } else {
                            "mutation"
                        }
                        .into(),
                    )
                    .or_default()
                    .insert(op.name.clone(), op.name.clone());
            }
        }
        let mut names = std::collections::BTreeSet::new();
        for (group, methods) in groups {
            let property = member(&group);
            let group = ident(&group);
            ensure!(
                names.insert(group.clone()) && group != "Execute",
                "GraphQL group naming collision"
            );
            writeln!(source,"extension GraphqlClient {{ public var {property}: {group}Group {{ {group}Group(client:self) }} }}\npublic struct {group}Group {{ fileprivate let client: GraphqlClient").unwrap();
            let mut names = std::collections::BTreeSet::new();
            for (method, operation) in methods {
                let method = member(&method);
                ensure!(
                    names.insert(method.clone()) && method != "`client`",
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
                let args = if op.variables.is_empty() {
                    String::new()
                } else {
                    format!("variables: {name}Variables")
                };
                let vars = if op.variables.is_empty() {
                    "GraphqlEmptyVariables()"
                } else {
                    "variables"
                };
                writeln!(source,"public func {method}({args}) async throws -> GraphqlResponse<{name}Result> {{ try await client.execute(operationName:{},document:{},variables:{vars},resultType:{name}Result.self) }}",literal(&op.name),literal(&op.document)).unwrap();
            }
            source.push_str("}\n");
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
        for prefix in ["public final class ", "public struct ", "public enum "] {
            if let Some(rest) = line.strip_prefix(prefix) {
                let name = rest.split([' ', '<', ':', '{']).next().unwrap_or_default();
                ensure!(
                    declarations.insert(name.to_owned()),
                    "GraphQL generated declaration collision {name}"
                );
            }
        }
    }
    Ok((source, symbols))
}
