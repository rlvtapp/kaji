//! Native selection-specific GraphQL Python clients.
use crate::{Python, python_identifier, python_module_name};
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
    const NAME: &'static str = "poolster.python.graphql-client.v1";
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
impl Plugin<Python> for Graphql {
    fn kind(&self) -> &'static str {
        "python-graphql"
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
    fn generate(&self, cx: &mut PluginContext<'_, Python>) -> Result<()> {
        let contract = cx.inputs.get::<GraphqlOperations>()?;
        let (mut tree, methods) = render(
            contract,
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("graphql_client"),
            self.style,
            &self.groups,
        )?;
        if let Some(version) = &cx.common.package_version {
            let manifest = tree.get("pyproject.toml").unwrap().replace(
                "version = \"0.0.0\"",
                &format!("version = {:?}", crate::python_package_version(version)),
            );
            tree.replace(GeneratedFile::new("pyproject.toml", manifest)?)?;
        }
        cx.files.append(tree)?;
        cx.publish(GraphqlClient { methods })
    }
}
struct Models {
    source: String,
    names: BTreeSet<String>,
}
impl Models {
    fn fields(&mut self, name: &str, fields: &[ModelField]) -> Result<String> {
        ensure!(
            !matches!(
                name,
                "None"
                    | "True"
                    | "False"
                    | "class"
                    | "def"
                    | "return"
                    | "and"
                    | "or"
                    | "not"
                    | "if"
                    | "else"
                    | "elif"
                    | "for"
                    | "while"
                    | "in"
                    | "is"
                    | "import"
                    | "from"
                    | "with"
                    | "as"
                    | "try"
                    | "except"
                    | "finally"
                    | "raise"
                    | "pass"
                    | "break"
                    | "continue"
                    | "lambda"
                    | "yield"
                    | "global"
                    | "nonlocal"
                    | "assert"
                    | "del"
                    | "async"
                    | "await"
                    | "Any"
                    | "TypedDict"
                    | "Optional"
                    | "List"
                    | "Union"
                    | "Literal"
                    | "Client"
                    | "Transport"
                    | "GraphqlResponse"
            ),
            "GraphQL Python type name conflicts with language/runtime name: {name}"
        );
        ensure!(
            name.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "invalid GraphQL Python type name {name}"
        );
        ensure!(
            self.names.insert(name.into()),
            "GraphQL Python type collision: {name}"
        );
        let mut required = Vec::new();
        let mut optional = Vec::new();
        for (i, field) in fields.iter().enumerate() {
            let ty = self.ty(&field.ty, &format!("{name}Field{i}"))?;
            let entry = format!("{}: {ty}", serde_json::to_string(&field.name)?);
            if field.optional {
                optional.push(entry);
            } else {
                required.push(entry);
            }
        }
        writeln!(
            self.source,
            "_{name}Required = TypedDict({name:?}, {{{}}})\n_{name}Optional = TypedDict({name:?}, {{{}}}, total=False)\nclass {name}(_{name}Required, _{name}Optional):\n    pass\n",
            required.join(", "),
            optional.join(", ")
        )?;
        Ok(name.into())
    }
    fn ty(&mut self, ty: &ModelType, name: &str) -> Result<String> {
        let mut value = match &ty.kind {
            ModelKind::Scalar(s) => match s.as_str() {
                "String" | "ID" => "str",
                "Int" => "int",
                "Float" => "float",
                "Boolean" => "bool",
                _ => "Any",
            }
            .into(),
            ModelKind::Named(s) => format!("{s:?}"),
            ModelKind::Enum(values) => format!(
                "Literal[{}]",
                values
                    .iter()
                    .map(|v| format!("{v:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ModelKind::Literal(v) => format!("Literal[{v:?}]"),
            ModelKind::List(inner) => format!("List[{}]", self.ty(inner, &format!("{name}Item"))?),
            ModelKind::Object(fields) => self.fields(name, fields)?,
            ModelKind::Union(members) => format!(
                "Union[{}]",
                members
                    .iter()
                    .enumerate()
                    .map(|(i, t)| self.ty(t, &format!("{name}Variant{i}")))
                    .collect::<Result<Vec<_>>>()?
                    .join(", ")
            ),
        };
        if ty.nullable {
            value = format!("Optional[{value}]");
        }
        Ok(value)
    }
}
fn render(
    contract: &GraphqlOperations,
    distribution: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    ensure!(
        !contract.operations.is_empty(),
        "Python GraphQL requires operation documents"
    );
    ensure!(
        contract
            .operations
            .iter()
            .all(|op| op.kind != GraphqlOperationKind::Subscription),
        "Python GraphQL subscriptions require a separate unsupported transport"
    );
    let module = python_module_name(distribution);
    ensure!(
        module == distribution.replace('-', "_") && !module.is_empty(),
        "invalid Python GraphQL package name"
    );
    let mut models = Models {
        source: "from typing import Any, TypedDict, Optional, List, Union, Literal\n".into(),
        names: BTreeSet::new(),
    };
    for (name, fields) in &contract.input_objects {
        models.fields(name, fields)?;
    }
    let mut operations =
        String::from("from .models import *\nfrom .runtime import GraphqlResponse, Transport\n\n");
    let mut methods = BTreeMap::new();
    let mut signatures = BTreeMap::new();
    let mut names = BTreeSet::new();
    for (i, op) in contract.operations.iter().enumerate() {
        let name = python_identifier(&op.name);
        ensure!(
            names.insert(name.clone()),
            "Python GraphQL operation name collision: {name}"
        );
        ensure!(
            name != "transport",
            "Python GraphQL operation conflicts with runtime symbol"
        );
        let vars = models.fields(&format!("Operation{i}Variables"), &op.variables)?;
        let result = models.ty(&op.result, &format!("Operation{i}Result"))?;
        let parameter = if op.variables.iter().all(|v| v.optional) {
            format!("variables: Optional[{vars}] = None")
        } else {
            format!("variables: {vars}")
        };
        writeln!(
            operations,
            "def {name}(transport: Transport, {parameter}) -> GraphqlResponse[{result}]:\n    return transport.execute({}, {}, {{}} if variables is None else variables)\n",
            serde_json::to_string(&op.document)?,
            serde_json::to_string(&op.name)?
        )?;
        methods.insert(op.name.clone(), name.clone());
        signatures.insert(op.name.clone(), (vars, result));
    }
    let mut client = String::from(
        "from . import operations\nfrom .models import *\nfrom .runtime import Transport, GraphqlResponse\n\nclass Client:\n    def __init__(self, endpoint: str, *, headers=None, timeout: float = 30):\n        self._transport = Transport(endpoint, headers=headers, timeout=timeout)\n",
    );
    if !matches!(style, GraphqlStyle::Raw) {
        for op in &contract.operations {
            let (vars, result) = &signatures[&op.name];
            let name = &methods[&op.name];
            let parameter = if op.variables.iter().all(|v| v.optional) {
                format!("variables: Optional[{vars}] = None")
            } else {
                format!("variables: {vars}")
            };
            writeln!(
                client,
                "    def {name}(self, {parameter}) -> GraphqlResponse[{result}]:\n        return operations.{name}(self._transport, variables)\n"
            )?;
        }
    }
    let mut group_source = String::new();
    if matches!(style, GraphqlStyle::Idiomatic) {
        let mut group_names = BTreeSet::new();
        let mut selected = groups.clone();
        if selected.is_empty() {
            for op in &contract.operations {
                selected
                    .entry(
                        if op.kind == GraphqlOperationKind::Query {
                            "query"
                        } else {
                            "mutation"
                        }
                        .into(),
                    )
                    .or_default()
                    .insert(methods[&op.name].clone(), op.name.clone());
            }
        }
        for (index, (group, entries)) in selected.iter().enumerate() {
            let group_name = python_identifier(group);
            ensure!(
                group_names.insert(group_name.clone())
                    && !names.contains(&group_name)
                    && !matches!(group_name.as_str(), "_transport" | "execute"),
                "Python GraphQL group collision: {group_name}"
            );
            writeln!(
                client,
                "    @property\n    def {group_name}(self):\n        return _Group{index}(self._transport)\n"
            )?;
            let mut normalized = BTreeSet::new();
            writeln!(
                group_source,
                "class _Group{index}:\n    def __init__(self, transport):\n        self._transport = transport"
            )?;
            for (method, operation) in entries {
                let method = python_identifier(method);
                ensure!(
                    normalized.insert(method.clone()) && method != "_transport",
                    "Python GraphQL grouped method collision"
                );
                let Some((vars, result)) = signatures.get(operation) else {
                    anyhow::bail!("unknown GraphQL grouped operation {operation}");
                };
                let op = contract
                    .operations
                    .iter()
                    .find(|op| &op.name == operation)
                    .unwrap();
                let parameter = if op.variables.iter().all(|v| v.optional) {
                    format!("variables: Optional[{vars}] = None")
                } else {
                    format!("variables: {vars}")
                };
                writeln!(
                    group_source,
                    "    def {method}(self, {parameter}) -> GraphqlResponse[{result}]:\n        return operations.{}(self._transport, variables)\n",
                    methods[operation]
                )?;
            }
        }
    }
    client.push_str(&group_source);
    let mut tree = GeneratedTree::default();
    for (path,contents) in [("models.py",models.source),("operations.py",operations),("client.py",client),("runtime.py",include_str!("../templates/graphql_runtime.py").into()),("__init__.py","from .client import Client\nfrom .runtime import GraphqlResponse, GraphqlErrors, Transport\nfrom .models import *\n".into()),("py.typed",String::new())] {tree.insert(GeneratedFile::new(format!("src/{module}/{path}"),contents)?)?;}
    tree.insert(GeneratedFile::new("README.md", "# GraphQL Python client\n\nRequires Python 3.9+. Flat clients expose snake_case operation methods; idiomatic clients additionally expose query/mutation or configured groups. Raw exports use an explicit Transport. Variables and selection-specific results are TypedDicts: omitted keys differ from explicit None. GraphqlResponse preserves data, errors and extensions; inspect status or call require_data() to reject partial results. urllib transport exceptions remain distinct. Custom scalars retain Any JSON wire values. Subscriptions, incremental delivery and async transports are not supported.\n")?)?;
    tree.insert(GeneratedFile::new("pyproject.toml",format!("[build-system]\nrequires = [\"setuptools>=68\"]\nbuild-backend = \"setuptools.build_meta\"\n[project]\nname = {distribution:?}\nversion = \"0.0.0\"\nrequires-python = \">=3.9\"\n[tool.setuptools.packages.find]\nwhere = [\"src\"]\n[tool.setuptools.package-data]\n\"*\" = [\"py.typed\"]\n"))?)?;
    Ok((tree, methods))
}
#[cfg(test)]
mod tests;
