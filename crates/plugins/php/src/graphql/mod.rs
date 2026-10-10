//! Selection-specific native GraphQL PHP 8.2 clients.
use crate::Php;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile, GeneratedTree,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{
        GraphqlIncrementalOperations, GraphqlOperationKind, GraphqlOperations, ModelField,
        ModelKind, ModelType,
    },
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
    incremental_provider: Option<Handle<GraphqlIncrementalOperations>>,
    subscriptions: bool,
    incremental: bool,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        incremental_provider: None,
        subscriptions: false,
        incremental: false,
        style: GraphqlStyle::default(),
        groups: BTreeMap::new(),
    }
}
pub fn graphql_incremental(provider: Option<Handle<GraphqlIncrementalOperations>>) -> Graphql {
    let mut plugin = graphql(None);
    plugin.incremental_provider = provider;
    plugin.incremental = true;
    plugin
}
impl Graphql {
    pub fn subscriptions(mut self) -> Self {
        self.subscriptions = true;
        self
    }
    pub fn incremental_input(mut self, input: Handle<GraphqlIncrementalOperations>) -> Self {
        self.incremental_provider = Some(input);
        self.incremental = true;
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
        if self.incremental {
            vec![Requirement::on(self.incremental_provider)]
        } else {
            vec![Requirement::on(self.provider)]
        }
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Php>) -> Result<()> {
        let contract = if self.incremental {
            &cx.inputs.get::<GraphqlIncrementalOperations>()?.definition
        } else {
            cx.inputs.get::<GraphqlOperations>()?
        };
        let (mut tree, methods) = render_advanced(
            contract,
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("poolster/graphql-client"),
            self.style,
            &self.groups,
            self.subscriptions,
            self.incremental,
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
            format!("trait {class}\n{{\n{method}\n}}"),
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
                format!(
                    "{requires}\ntrait {class}\n{{\n    use {};\n}}\n",
                    children.join(", ")
                ),
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
/// Namespace used by the portable PHP package.
pub fn graphql_package_namespace(package: &str) -> String {
    crate::namespace_for_package(package)
}

/// Render a portable query/mutation package for framework integrations.
/// Frameworks add their transport and container wiring without duplicating models.
pub fn render_graphql_package(
    contract: &GraphqlOperations,
    package: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<GeneratedTree> {
    Ok(render_advanced(contract, package, style, groups, false, false)?.0)
}

#[cfg(test)]
pub(crate) fn render(
    contract: &GraphqlOperations,
    package: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    render_advanced(contract, package, style, groups, false, false)
}
mod render;
use render::render_advanced;

#[cfg(test)]
mod tests;

mod source_layout;
