//! Native GraphQL clients consume selection contracts, never the HTTP AST.
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
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        style: GraphqlStyle::default(),
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
        let (mut tree, methods) = render(
            cx.inputs.get::<GraphqlOperations>()?,
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("io.poolster.graphql"),
            self.style,
            &self.groups,
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
fn render(
    c: &GraphqlOperations,
    package: &str,
    style: GraphqlStyle,
    custom: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    ensure!(
        !c.operations.is_empty(),
        "Java GraphQL requires operation documents"
    );
    ensure!(
        package.len() <= 512
            && package
                .split('.')
                .all(|segment| models::identifier(segment) && segment.len() <= 128),
        "Invalid Java package name"
    );
    fn validate_names(ty: &poolster_core::native::ModelType, c: &GraphqlOperations) -> Result<()> {
        use poolster_core::native::ModelKind;
        match &ty.kind {
            ModelKind::Named(name) => ensure!(
                c.input_objects.contains_key(name),
                "Unknown Java GraphQL input object {name}"
            ),
            ModelKind::List(inner) => validate_names(inner, c)?,
            ModelKind::Object(fields) => {
                for field in fields {
                    validate_names(&field.ty, c)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    for fields in c
        .input_objects
        .values()
        .chain(c.operations.iter().map(|o| &o.variables))
    {
        for field in fields {
            validate_names(&field.ty, c)?;
        }
    }
    let mut models = models::Models::default();
    for (name, fields) in &c.input_objects {
        models.input(name, fields)?;
    }
    let mut methods = BTreeMap::new();
    let mut calls = String::new();
    let mut operation_files = Vec::new();
    let mut groups: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for op in &c.operations {
        ensure!(
            op.kind != GraphqlOperationKind::Subscription,
            "Java GraphQL subscriptions require a separately supported transport"
        );
        ensure!(
            models::identifier(&op.name),
            "Invalid Java operation identifier: {}",
            op.name
        );
        let method = crate::method_name(&op.name);
        ensure!(
            ![
                "execute",
                "query",
                "mutation",
                "transport",
                "getClass",
                "wait",
                "notify",
                "notifyAll",
                "toString",
                "hashCode",
                "equals"
            ]
            .contains(&method.as_str()),
            "Reserved Java operation name"
        );
        ensure!(
            !methods.values().any(|v| v == &method),
            "Java GraphQL operation name collision"
        );
        methods.insert(op.name.clone(), method.clone());
        models.input(&format!("{}Variables", op.name), &op.variables)?;
        ensure!(
            matches!(op.result.kind, poolster_core::native::ModelKind::Object(_)),
            "GraphQL operation result must be an object selection"
        );
        let result = models.ty(&op.result, &format!("{}Result", op.name))?;
        let vars = format!("{}Variables", op.name);
        let chunks = op
            .document
            .chars()
            .collect::<Vec<_>>()
            .chunks(4000)
            .map(|chars| serde_json::to_string(&chars.iter().collect::<String>()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let document = if chunks.len() <= 1 {
            chunks.first().cloned().unwrap_or("\"\"".into())
        } else {
            format!("String.join(\"\",{})", chunks.join(","))
        };
        let name = serde_json::to_string(&op.name)?;
        let (receiver, param) = if style == GraphqlStyle::Raw {
            ("client.transport", "Client client, ")
        } else {
            ("transport", "")
        };
        writeln!(
            calls,
            "public {} Envelope<{result}> {method}({param}{vars} variables) throws java.io.IOException, InterruptedException {{ return {}Operation.execute({receiver}, variables); }}",
            if style == GraphqlStyle::Raw {
                "static"
            } else {
                ""
            },
            op.name
        )?;
        operation_files.push((format!("{}Operation",op.name),format!("package {package}.operations;\nimport {package}.models.*;\nimport static {package}.GraphqlRuntime.*;\npublic final class {}Operation {{ private {}Operation() {{}} public static Envelope<{result}> execute(Transport transport,{vars} variables) throws java.io.IOException,InterruptedException {{ return transport.execute({document},{name},variables.toJson(),{result}::fromJson); }} }}\n",op.name,op.name)));
        if op.variables.iter().all(|v| v.optional) {
            writeln!(
                calls,
                "public {} Envelope<{result}> {method}({}) throws java.io.IOException, InterruptedException {{ return {method}({}new {vars}({})); }}",
                if style == GraphqlStyle::Raw {
                    "static"
                } else {
                    ""
                },
                if style == GraphqlStyle::Raw {
                    "Client client"
                } else {
                    ""
                },
                if style == GraphqlStyle::Raw {
                    "client, "
                } else {
                    ""
                },
                op.variables
                    .iter()
                    .map(|_| "Field.absent()")
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        if style == GraphqlStyle::Idiomatic {
            groups
                .entry(
                    match op.kind {
                        GraphqlOperationKind::Query => "query",
                        _ => "mutation",
                    }
                    .into(),
                )
                .or_default()
                .insert(method, op.name.clone());
        }
    }
    if !custom.is_empty() {
        ensure!(
            style == GraphqlStyle::Idiomatic,
            "Custom groups require idiomatic style"
        );
        groups = custom.clone();
    }
    let mut group_code = String::new();
    let mut group_files = Vec::new();
    for (group, members) in groups {
        ensure!(
            models::identifier(&group)
                && !methods.values().any(|m| m == &group)
                && ![
                    "transport",
                    "execute",
                    "getClass",
                    "wait",
                    "notify",
                    "notifyAll",
                    "toString",
                    "hashCode",
                    "equals"
                ]
                .contains(&group.as_str()),
            "Invalid or colliding Java group: {group}"
        );
        let class = format!("{}Group", crate::type_name(&group));
        models.reserve(&class)?;
        writeln!(
            group_code,
            "public {package}.groups.{class} {group}() {{ return new {package}.groups.{class}(this); }}"
        )?;
        let mut group_source = format!(
            "package {package}.groups;\nimport {package}.Client;\nimport {package}.models.*;\nimport static {package}.GraphqlRuntime.*;\npublic final class {class} {{ private final Client client; public {class}(Client client) {{this.client=client;}}\n"
        );
        for (method, op) in members {
            ensure!(
                models::identifier(&method)
                    && ![
                        "getClass",
                        "wait",
                        "notify",
                        "notifyAll",
                        "toString",
                        "hashCode",
                        "equals"
                    ]
                    .contains(&method.as_str()),
                "Invalid Java grouped method"
            );
            let target = methods
                .get(&op)
                .ok_or_else(|| anyhow::anyhow!("Unknown GraphQL operation {op}"))?;
            writeln!(
                group_source,
                "public Envelope<{op}Result> {method}({op}Variables variables) throws java.io.IOException, InterruptedException {{ return client.{target}(variables); }}"
            )?;
            if c.operations
                .iter()
                .find(|v| v.name == op)
                .unwrap()
                .variables
                .iter()
                .all(|v| v.optional)
            {
                writeln!(
                    group_source,
                    "public Envelope<{op}Result> {method}() throws java.io.IOException, InterruptedException {{ return client.{target}(); }}"
                )?;
            }
        }
        group_source.push_str("}\n");
        group_files.push((class, group_source));
    }
    let source = format!(
        "package {package};\nimport {package}.models.*;\nimport {package}.operations.*;\nimport java.util.Objects;\npublic final class Client extends GraphqlRuntime {{ public Client(Transport transport){{super(transport);}} public Client(String endpoint){{this(new HttpTransport(endpoint));}}\n{calls}\n{group_code}\n}}\n"
    );
    let root = format!("src/main/java/{}", package.replace('.', "/"));
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new(format!("{root}/Client.java"), source)?)?;
    let runtime = include_str!("graphql/runtime.java.tmpl")
        .replace("private static ", "public static ")
        .replace(
            "public static final ObjectMapper JSON",
            "private static final ObjectMapper JSON",
        );
    tree.insert(GeneratedFile::new(format!("{root}/GraphqlRuntime.java"),format!("package {package};\nimport com.fasterxml.jackson.databind.*;\nimport com.fasterxml.jackson.databind.node.*;\nimport java.util.*;\npublic class GraphqlRuntime {{ public final Transport transport; protected GraphqlRuntime(Transport transport){{this.transport=Objects.requireNonNull(transport);}} public static ObjectNode object(){{return JSON.createObjectNode();}}\n{runtime}\n}}\n"))?)?;
    for (name, source) in layout::declarations(&models.source)? {
        let source = source.replace("JSON.createObjectNode()", "object()");
        tree.insert(GeneratedFile::new(format!("{root}/models/{name}.java"),format!("package {package}.models;\nimport com.fasterxml.jackson.databind.*;\nimport com.fasterxml.jackson.databind.node.*;\nimport static {package}.GraphqlRuntime.*;\npublic {}\n",source.strip_prefix("public ").unwrap()))?)?;
    }
    for (name, source) in operation_files {
        tree.insert(GeneratedFile::new(
            format!("{root}/operations/{name}.java"),
            source,
        )?)?;
    }
    for (name, source) in group_files {
        tree.insert(GeneratedFile::new(
            format!("{root}/groups/{name}.java"),
            source,
        )?)?;
    }
    tree.insert(GeneratedFile::new(
        "pom.xml",
        crate::docs::pom_xml(package, "graphql-client", "0.0.0"),
    )?)?;
    tree.insert(GeneratedFile::new(
        "build.gradle",
        crate::docs::build_gradle(package, "graphql-client", "0.0.0"),
    )?)?;
    tree.insert(GeneratedFile::new(
        "settings.gradle",
        crate::docs::settings_gradle("graphql-client"),
    )?)?;
    tree.insert(GeneratedFile::new("README.md",format!("# GraphQL Java client\n\nJava 17+; Maven or Gradle; pinned Jackson 2.18.3. Import `{package}.Client`.\n\nSelected style: `{style:?}`. Raw exposes static operation functions taking a client, flat exposes methods on the client, and idiomatic exposes query/mutation or explicitly configured resource groups.\n\nVariables and selected results are individual records in `{package}.models`; import `{package}.models.ReadUserVariables` instead of the previous `Client.ReadUserVariables`. Client call paths and inherited Field/Envelope/Transport helper types remain compatible. Each operation document and execution body lives in a separate operations file; Client is a thin facade. Atomic declarations and exceptionally large public facades exceeding128KiB are reported in .poolster/source-layout-diagnostics.json rather than silently split across incompatible APIs. Optional fields use `Field.absent()` or `Field.of(value)`; explicit null is distinct from absence. HTTP uses JDK HttpClient. Supply `HttpTransport(endpoint, http, headers)` or implement `Transport`.\n\nCalls return `Envelope<T>` with data presence, typed partial data, errors, extensions and HTTP status. `requireData()` rejects GraphQL errors and missing/null data. Transport/protocol failures throw IOException; interruption remains explicit. Abstract selections require selected __typename. Custom scalars remain JsonNode. Subscriptions require a separately supported transport and are rejected.\n"))?)?;
    tree.insert(GeneratedFile::new(
        "graphql/schema.graphql",
        c.schema_source.clone(),
    )?)?;
    tree.insert(GeneratedFile::new(
        "graphql/operations.graphql",
        c.operation_source.clone(),
    )?)?;
    let oversized=tree.iter().filter(|(path,source)|path.extension().is_some_and(|ext|ext=="java")&&source.len()>128*1024).map(|(path,source)|serde_json::json!({"path":path,"bytes":source.len(),"max_file_bytes":128*1024,"reason":"Atomic declaration or thin public facade exceeds the source budget; retained intact to preserve public API."})).collect::<Vec<_>>();
    if !oversized.is_empty() {
        tree.insert(GeneratedFile::new(
            ".poolster/source-layout-diagnostics.json",
            serde_json::to_string_pretty(&oversized)?,
        )?)?;
    }
    Ok((tree, methods))
}
