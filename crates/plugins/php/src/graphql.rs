//! Selection-specific native GraphQL PHP 8.2 clients.
use crate::Php;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile, GeneratedTree,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{GraphqlOperationKind, GraphqlOperations, ModelField, ModelKind, ModelType},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
#[derive(Clone, Copy, Debug, Default)]
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
    const NAME: &'static str = "poolster.php.graphql-client.v1";
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
impl Plugin<Php> for Graphql {
    fn kind(&self) -> &'static str {
        "php-graphql"
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
    fn generate(&self, cx: &mut PluginContext<'_, Php>) -> Result<()> {
        let (mut tree, methods) = render(
            cx.inputs.get::<GraphqlOperations>()?,
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("poolster/graphql-client"),
            self.style,
            &self.groups,
        )?;
        if let Some(version) = &cx.common.package_version {
            let manifest = tree.get("composer.json").unwrap().replace("0.0.0", version);
            tree.replace(GeneratedFile::new("composer.json", manifest)?)?;
        }
        cx.files.append(tree)?;
        cx.publish(GraphqlClient { methods })
    }
}
fn ident(value: &str) -> Result<String> {
    ensure!(
        !value.is_empty()
            && value.bytes().enumerate().all(|(i, c)| c == b'_'
                || c.is_ascii_alphabetic()
                || (i > 0 && c.is_ascii_digit())),
        "invalid PHP GraphQL identifier {value}"
    );
    ensure!(
        ![
            "class",
            "function",
            "trait",
            "interface",
            "enum",
            "match",
            "readonly",
            "new",
            "clone",
            "extends",
            "implements",
            "static",
            "self",
            "parent",
            "public",
            "private",
            "protected",
            "return",
            "throw",
            "try",
            "catch",
            "finally",
            "echo",
            "print",
            "true",
            "false",
            "null",
            "int",
            "float",
            "string",
            "bool",
            "mixed",
            "object",
            "array",
            "void",
            "never",
            "callable"
        ]
        .contains(&value.to_ascii_lowercase().as_str()),
        "reserved PHP identifier {value}"
    );
    Ok(value.into())
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}
mod models;
use models::Models;
fn upper(value: &str) -> String {
    let mut v = value.to_string();
    v[..1].make_ascii_uppercase();
    v
}
fn method_traits(
    owner: &str,
    methods: &BTreeMap<String, String>,
    files: &mut BTreeMap<String, String>,
    names: &mut BTreeSet<String>,
) -> Result<String> {
    let mut level = vec![];
    for (name, method) in methods {
        let class = format!("{owner}{name}Methods");
        ensure!(
            names.insert(class.to_ascii_lowercase()),
            "GraphQL trait naming collision {class}"
        );
        files.insert(
            format!("Methods/{class}.php"),
            format!("trait {class} {{\n{method}\n}}"),
        );
        level.push(class);
    }
    let mut depth = 0;
    while level.len() > 1 {
        let mut next = vec![];
        for (index, children) in level.chunks(2).enumerate() {
            if children.len() == 1 {
                next.push(children[0].clone());
                continue;
            }
            let class = format!("{owner}Branch{depth}Node{index}Methods");
            ensure!(
                names.insert(class.to_ascii_lowercase()),
                "GraphQL trait naming collision {class}"
            );
            let requires = children
                .iter()
                .map(|child| format!("require_once __DIR__.'/{child}.php';\n"))
                .collect::<String>();
            files.insert(
                format!("Methods/{class}.php"),
                format!("{requires}trait {class} {{use {};}}", children.join(",")),
            );
            next.push(class);
        }
        level = next;
        depth += 1;
    }
    Ok(level
        .first()
        .map(|root| format!("use {root};"))
        .unwrap_or_default())
}
pub(crate) fn render(
    contract: &GraphqlOperations,
    package: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    ensure!(
        package.split('/').count() == 2
            && package
                .split('/')
                .all(|part| part.as_bytes().first().is_some_and(u8::is_ascii_lowercase))
            && package
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "/-_.".contains(c)),
        "invalid Composer package name"
    );
    ensure!(
        matches!(style, GraphqlStyle::Idiomatic) || groups.is_empty(),
        "custom GraphQL groups require idiomatic style"
    );
    let namespace = crate::namespace_for_package(package);
    for segment in namespace.split('\\') {
        ident(segment)?;
    }
    let mut models = Models {
        namespace: namespace.clone(),
        files: BTreeMap::new(),
        names: ["client", "presence", "graphqlresponse", "graphqlexception"]
            .into_iter()
            .map(str::to_string)
            .collect(),
    };
    for (name, fields) in &contract.input_objects {
        models.object(name, fields, true)?;
    }
    let mut symbols = BTreeMap::new();
    let mut files = BTreeMap::new();
    let mut methods = BTreeMap::new();
    let mut used = BTreeSet::new();
    for op in &contract.operations {
        ensure!(
            op.kind != GraphqlOperationKind::Subscription,
            "PHP GraphQL subscriptions require a separately supported transport"
        );
        let name = ident(&op.name)?;
        let method = {
            let mut v = name.clone();
            v[..1].make_ascii_lowercase();
            v
        };
        ensure!(
            used.insert(method.to_ascii_lowercase())
                && !matches!(
                    method.to_ascii_lowercase().as_str(),
                    "execute"
                        | "__construct"
                        | "query"
                        | "mutation"
                        | "decodevalue"
                        | "encodevalue"
                ),
            "GraphQL operation naming collision {method}"
        );
        models.object(&format!("{name}Variables"), &op.variables, true)?;
        let ModelKind::Object(fields) = &op.result.kind else {
            anyhow::bail!("GraphQL operation result must be an object")
        };
        models.object(&format!("{name}Result"), fields, false)?;
        let variables = if op.variables.iter().all(|v| v.optional) {
            format!("?{name}Variables $variables=null")
        } else {
            format!("{name}Variables $variables")
        };
        let mut functions = String::new();
        writeln!(functions,"/** @return GraphqlResponse<{name}Result> */\nfunction {method}(Client $client,{variables}):GraphqlResponse{{return $client->execute({},{},$variables??new {name}Variables(),{name}Result::class);}}",quote(&name),quote(&op.document)).unwrap();
        if matches!(style, GraphqlStyle::Flat) {
            methods.insert(name.clone(),format!("/** @return GraphqlResponse<{name}Result> */ public function {method}({variables}):GraphqlResponse{{return {method}($this,$variables);}}"));
        }
        files.insert(format!("Operations/{name}.php"), functions);
        symbols.insert(name, method);
    }
    if matches!(style, GraphqlStyle::Idiomatic) {
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
                    .insert(symbols[&op.name].clone(), op.name.clone());
            }
        }
        let mut group_names = BTreeSet::new();
        for (group, entries) in groups {
            let group = ident(&group)?;
            ensure!(
                group_names.insert(group.to_ascii_lowercase())
                    && !matches!(
                        group.to_ascii_lowercase().as_str(),
                        "execute" | "__construct"
                    ),
                "GraphQL group naming collision"
            );
            let class = format!("{}Group", upper(&group));
            ensure!(
                models.names.insert(class.to_ascii_lowercase()),
                "GraphQL group type collision"
            );
            methods.insert(
                class.clone(),
                format!("public function {group}():{class}{{return new {class}($this);}}"),
            );
            let mut group_methods = BTreeMap::new();
            let mut names = BTreeSet::new();
            for (method, operation) in entries {
                let method = ident(&method)?;
                ensure!(
                    names.insert(method.to_ascii_lowercase()) && method != "__construct",
                    "GraphQL grouped method collision"
                );
                let function = symbols
                    .get(&operation)
                    .ok_or_else(|| anyhow::anyhow!("unknown GraphQL operation {operation}"))?;
                let op = contract
                    .operations
                    .iter()
                    .find(|o| o.name == operation)
                    .unwrap();
                let vars = if op.variables.iter().all(|f| f.optional) {
                    format!("?{operation}Variables $variables=null")
                } else {
                    format!("{operation}Variables $variables")
                };
                group_methods.insert(upper(&method),format!("/** @return GraphqlResponse<{operation}Result> */ public function {method}({vars}):GraphqlResponse{{return {function}($this->client,$variables);}}"));
            }
            let inherited = method_traits(&class, &group_methods, &mut files, &mut models.names)?;
            files.insert(format!("Groups/{class}.php"),format!("final readonly class {class} {{ public function __construct(private Client $client){{}} {inherited} }}"));
        }
    }
    let mut tree = GeneratedTree::default();
    let inherited = method_traits("Client", &methods, &mut files, &mut models.names)?;
    let runtime =
        include_str!("../templates/graphql.php.tmpl").replace("__NAMESPACE__", &namespace);
    let (runtime, client) = runtime.split_once("class Client {").unwrap();
    tree.insert(GeneratedFile::new("src/Runtime.php", runtime)?)?;
    files.insert(
        "Client.php".into(),
        format!(
            "class Client {{{}",
            client.replace("__CLIENT_METHODS__", &inherited)
        ),
    );
    for (name, source) in models.files {
        files.insert(format!("Models/{name}.php"), source);
    }
    for (file, source) in files {
        tree.insert(GeneratedFile::new(
            format!("src/{file}"),
            format!("<?php\ndeclare(strict_types=1);\nnamespace {namespace};\n{source}"),
        )?)?;
    }
    tree.insert(GeneratedFile::new("src/Graphql.php","<?php\ndeclare(strict_types=1);\nrequire_once __DIR__.'/Runtime.php';\nforeach(['Models','Operations','Methods','Groups'] as $dir) foreach(glob(__DIR__.'/'. $dir .'/*.php') ?: [] as $file) require_once $file;\nrequire_once __DIR__.'/Client.php';\n")?)?;
    tree.insert(GeneratedFile::new("composer.json",serde_json::to_string_pretty(&serde_json::json!({"name":package,"version":"0.0.0","require":{"php":">=8.2"},"autoload":{"files":["src/Graphql.php"]}}))?)?)?;
    tree.insert(GeneratedFile::new("README.md","# GraphQL client\n\nPHP 8.2+, Composer autoload. Query/mutation fixed-operation clients use selection-specific immutable models. Optional fields use Presence::missing() versus Presence::of(null). Responses preserve partial data and errors; requireData() rejects errors. Raw functions, flat Client methods or grouped accessors. HTTP streams by default, injectable callable transport for PSR-18/Symfony. Subscriptions unsupported. Custom scalars retain JSON values.\n")?)?;
    source_layout::diagnostics(&mut tree)?;
    Ok((tree, symbols))
}
#[cfg(test)]
mod tests;

mod source_layout;
