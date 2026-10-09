//! Native GraphQL clients consume selection contracts, never the HTTP AST.
mod models;
#[cfg(test)]
mod tests;
use crate::Elixir;
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
    const NAME: &'static str = "poolster.elixir.graphql-client.v1";
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
impl Plugin<Elixir> for Graphql {
    fn kind(&self) -> &'static str {
        "elixir-graphql"
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
    fn generate(&self, cx: &mut PluginContext<'_, Elixir>) -> Result<()> {
        let (mut tree, methods) = render(
            cx.inputs.get::<GraphqlOperations>()?,
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("graphql_client"),
            self.style,
            &self.groups,
        )?;
        if let Some(version) = &cx.common.package_version {
            let path = "mix.exs";
            let content = tree
                .get(path)
                .unwrap()
                .replace("0.0.0", &crate::package_version(version));
            tree.replace(GeneratedFile::new(path, content)?)?;
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
        "Elixir GraphQL requires operation documents"
    );
    let app = crate::elixir_identifier(package);
    let module = crate::pascal_case(package);
    ensure!(
        !app.is_empty() && !module.is_empty(),
        "Invalid Elixir GraphQL package name"
    );
    fn validate_names(t: &poolster_core::native::ModelType, c: &GraphqlOperations) -> Result<()> {
        use poolster_core::native::ModelKind;
        match &t.kind {
            ModelKind::Named(n) => ensure!(
                c.input_objects.contains_key(n),
                "Unknown GraphQL input object {n}"
            ),
            ModelKind::List(inner) => validate_names(inner, c)?,
            ModelKind::Object(fields) => {
                for f in fields {
                    validate_names(&f.ty, c)?;
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
        for f in fields {
            validate_names(&f.ty, c)?;
        }
    }
    let mut models = models::Models::new(&module);
    let mut methods = BTreeMap::new();
    let mut calls = String::new();
    let mut groups: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (name, fields) in &c.input_objects {
        models.input(name, fields)?;
    }
    for op in &c.operations {
        ensure!(
            op.kind != GraphqlOperationKind::Subscription,
            "Elixir GraphQL subscriptions need a separately supported transport"
        );
        let name = crate::pascal_case(&op.name);
        ensure!(!name.is_empty(), "Invalid Elixir GraphQL operation name");
        let method = crate::elixir_identifier(&op.name);
        ensure!(
            models::identifier(&method)
                && ![
                    "new",
                    "client",
                    "execute",
                    "require_data",
                    "__info__",
                    "module_info"
                ]
                .contains(&method.as_str()),
            "Invalid Elixir GraphQL operation method"
        );
        ensure!(
            !methods.values().any(|m| m == &method),
            "Elixir GraphQL operation method collision"
        );
        methods.insert(op.name.clone(), method.clone());
        ensure!(
            matches!(op.result.kind, poolster_core::native::ModelKind::Object(_)),
            "GraphQL result must be an object selection"
        );
        let vars = models.input(&format!("{name}Variables"), &op.variables)?;
        let result = models.ty(&op.result, &format!("{name}Result"))?;
        let optional = op.variables.iter().all(|v| v.optional);
        let default = if optional {
            format!(" \\\\ %{vars}{{}}")
        } else {
            String::new()
        };
        writeln!(
            calls,
            "  @spec {method}({module}.Client.t(), {vars}.t()) :: {{:ok, {module}.Envelope.t({result})}} | {{:error, term()}}\n  def {method}(client, variables{default}) do\n    try do\n      {module}.Runtime.execute(client, {}, {}, {vars}.to_wire(variables), &{module}.Models.{name}Result.from_wire/1)\n    rescue e in ArgumentError -> {{:error, {{:variables, Exception.message(e)}}}}\n    end\n  end",
            elixir_string(&op.document),
            elixir_string(&op.name)
        )?;
        if style == GraphqlStyle::Idiomatic {
            groups
                .entry(
                    match op.kind {
                        GraphqlOperationKind::Query => "Query",
                        _ => "Mutation",
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
    let surface = if style == GraphqlStyle::Raw {
        format!("{module}.Operations")
    } else {
        module.clone()
    };
    let mut source = format!("defmodule {surface} do\n{calls}\nend\n");
    let mut group_names = std::collections::BTreeSet::new();
    for (group, members) in groups {
        let group_name = crate::pascal_case(&group);
        ensure!(
            !group_name.is_empty()
                && group_names.insert(group_name.clone())
                && ![
                    "Client",
                    "Models",
                    "Runtime",
                    "Envelope",
                    "Application",
                    "MixProject"
                ]
                .contains(&group_name.as_str()),
            "Invalid Elixir group"
        );
        writeln!(source, "defmodule {module}.{group_name} do")?;
        let mut member_names = std::collections::BTreeSet::new();
        for (method, op) in members {
            let method = crate::elixir_identifier(&method);
            ensure!(
                models::identifier(&method)
                    && member_names.insert(method.clone())
                    && !["__info__", "module_info"].contains(&method.as_str()),
                "Invalid Elixir group method"
            );
            let target = methods
                .get(&op)
                .ok_or_else(|| anyhow::anyhow!("Unknown GraphQL operation {op}"))?;
            let name = crate::pascal_case(&op);
            let optional = c
                .operations
                .iter()
                .find(|v| v.name == op)
                .unwrap()
                .variables
                .iter()
                .all(|v| v.optional);
            let default = if optional {
                format!(" \\\\ %{module}.Models.{name}Variables{{}}")
            } else {
                String::new()
            };
            writeln!(
                source,
                "  def {method}(client, variables{default}), do: {surface}.{target}(client, variables)"
            )?;
        }
        source.push_str("end\n");
    }
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new(
        format!("lib/{app}/models.ex"),
        models.source,
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("lib/{app}/runtime.ex"),
        include_str!("graphql/runtime.ex.tmpl").replace("__POOLSTER__", &module),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("lib/{app}/operations.ex"),
        source,
    )?)?;
    tree.insert(GeneratedFile::new("mix.exs",format!("defmodule {module}.MixProject do\n use Mix.Project\n def project, do: [app: :{app},version: \"0.0.0\",elixir: \"~> 1.15\",deps: [{{:finch, \"== 0.24.0\"}},{{:jason, \"== 1.4.5\"}}]]\n def application, do: [extra_applications: [:logger,:inets,:crypto],mod: {{{module}.Application,[]}}]\nend\n"))?)?;
    tree.insert(GeneratedFile::new("README.md",format!("# GraphQL Elixir client\n\nFixed operation clients in `{style:?}` style. Construct `{module}.Client.new(endpoint)`. Raw functions live in `{module}.Operations`, flat functions in `{module}`, grouped functions in Query/Mutation or your custom modules. Variables and selected results are typed structs below `{module}.Models`. Optional variables default to `:poolster_absent`; nil means explicit null.\n\nReturns `{{:ok, Envelope}}` even for GraphQL errors/partial results; inspect data_present, data, errors, extensions and status. `{module}.Runtime.require_data/1` returns error for GraphQL errors or absent/null data. HTTP/protocol/variable failures return `{{:error, reason}}`. Finch0.24.0 and Jason1.4.5 are pinned. Custom transport receives a Finch request. Subscriptions rejected; abstract selections require selected __typename; custom scalars remain term().\n"))?)?;
    tree.insert(GeneratedFile::new(
        "graphql/schema.graphql",
        c.schema_source.clone(),
    )?)?;
    tree.insert(GeneratedFile::new(
        "graphql/operations.graphql",
        c.operation_source.clone(),
    )?)?;
    Ok((tree, methods))
}
fn elixir_string(value: &str) -> String {
    format!(
        "\"{}\"",
        crate::escape_elixir_string(value).replace("#{", "\\#{")
    )
}
