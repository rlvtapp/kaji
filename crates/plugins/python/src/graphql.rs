//! Native selection-specific GraphQL Python clients.
use crate::{Python, python_identifier, python_module_name};
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
    const NAME: &'static str = "poolster.python.graphql-client.v1";
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
        if self.incremental {
            vec![Requirement::on(self.incremental_provider)]
        } else {
            vec![Requirement::on(self.provider)]
        }
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Python>) -> Result<()> {
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
                .unwrap_or("graphql_client"),
            self.style,
            &self.groups,
            self.subscriptions,
            self.incremental,
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
mod models;
use models::Models;
mod codec_layout;
mod layout;
fn base_import(root: Option<(String, String)>, group: bool) -> (String, String) {
    if let Some((path, class)) = root {
        let path = if group {
            if let Some(path) = path.strip_prefix("..groups.") {
                format!(".{path}")
            } else {
                format!(".._mixins{path}")
            }
        } else if let Some(path) = path.strip_prefix("..") {
            format!(".{path}")
        } else {
            format!("._mixins{path}")
        };
        (
            format!("from {path} import {class} as _Base\n"),
            "(_Base)".into(),
        )
    } else {
        (String::new(), String::new())
    }
}
#[cfg(test)]
fn render(
    contract: &GraphqlOperations,
    distribution: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    render_advanced(contract, distribution, style, groups, false, false)
}
fn render_advanced(
    contract: &GraphqlOperations,
    distribution: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
    subscriptions: bool,
    incremental: bool,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    ensure!(
        !contract.operations.is_empty(),
        "Python GraphQL requires operation documents"
    );
    ensure!(
        contract
            .operations
            .iter()
            .all(|op| subscriptions || op.kind != GraphqlOperationKind::Subscription),
        "Python GraphQL subscriptions require explicit subscriptions capability"
    );
    let module = python_module_name(distribution);
    ensure!(
        module == distribution.replace('-', "_") && !module.is_empty(),
        "invalid Python GraphQL package name"
    );
    ensure!(
        matches!(style, GraphqlStyle::Idiomatic) || groups.is_empty(),
        "custom GraphQL groups require idiomatic style"
    );
    let mut models = Models {
        files: BTreeMap::new(),
        exports: vec![],
        names: BTreeSet::new(),
        input_names: contract.input_objects.keys().cloned().collect(),
        paths: BTreeSet::new(),
    };
    for (name, fields) in &contract.input_objects {
        models.fields(name, fields)?;
    }
    let mut files = BTreeMap::new();
    let mut methods = BTreeMap::new();
    let mut signatures = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut operation_exports = vec![];
    let mut client_bases = vec![];
    let mut selected_operations = contract.operations.iter().collect::<Vec<_>>();
    selected_operations.sort_by_key(|op| &op.name);
    for (index, op) in selected_operations.iter().enumerate() {
        let name = python_identifier(&op.name);
        ensure!(
            names.insert(name.clone()) && name != "transport",
            "Python GraphQL operation naming collision {name}"
        );
        ensure!(
            matches!(op.result.kind, ModelKind::Object(_)),
            "Python GraphQL operation result must be an object"
        );
        let vars = models.fields(&format!("{}Variables", op.name), &op.variables)?;
        let result = models.ty(&op.result, &format!("{}Result", op.name))?;
        let mut aliases = vec![];
        let mut alias_code = String::new();
        for (alias, canonical) in [
            (format!("Operation{index}Variables"), vars.clone()),
            (
                format!("Operation{index}Result"),
                format!("{}Result", op.name),
            ),
        ] {
            if alias != canonical {
                ensure!(
                    models.names.insert(alias.clone()),
                    "Python GraphQL legacy alias collision {alias}"
                );
                writeln!(
                    alias_code,
                    "from .{} import {canonical}\n{alias} = {canonical}",
                    python_identifier(&canonical)
                )?;
                aliases.push(alias);
            }
        }
        if !aliases.is_empty() {
            let alias_file = format!("_aliases_{name}");
            models
                .files
                .insert(format!("models/{alias_file}.py"), alias_code);
            models.exports.push((alias_file, aliases));
        }
        let parameter = if op.variables.iter().all(|v| v.optional) {
            format!("variables: Optional[{vars}] = None")
        } else {
            format!("variables: {vars}")
        };
        let mode = if op.kind == GraphqlOperationKind::Subscription {
            "subscribe"
        } else if incremental {
            "incremental"
        } else {
            "execute"
        };
        let annotation = if op.kind == GraphqlOperationKind::Subscription {
            format!("Iterator[GraphqlResponse[{result}]]")
        } else if incremental {
            "Iterator[IncrementalFrame]".into()
        } else {
            format!("GraphqlResponse[{result}]")
        };
        let shapes = format!(
            "VARIABLE_SHAPE = json.loads({})\nRESULT_SHAPE = json.loads({})\n",
            serde_json::to_string(&serde_json::to_string(
                &poolster_core::native::graphql_scalar_fields(&op.variables)
            )?)?,
            serde_json::to_string(&serde_json::to_string(
                &poolster_core::native::graphql_scalar_shape(&op.result)
            )?)?
        );
        let mut code = format!(
            "from typing import Optional, Iterator\nimport json\nfrom ..models import *\nfrom ..runtime import GraphqlResponse, Transport, IncrementalFrame\nfrom ..codec_shapes import INPUT_SHAPES\n{shapes}\ndef {name}(transport: Transport, {parameter}) -> {annotation}:\n    return transport.{mode}({}, {}, {{}} if variables is None else variables, VARIABLE_SHAPE, RESULT_SHAPE, INPUT_SHAPES)\n",
            serde_json::to_string(&op.document)?,
            serde_json::to_string(&op.name)?
        );
        if !matches!(style, GraphqlStyle::Raw) {
            writeln!(
                code,
                "class _OperationMixin:\n    _transport: Transport\n    def {name}(self, {parameter}) -> {annotation}:\n        return {name}(self._transport, variables)\n"
            )?;
            client_bases.push((format!("..operations.{name}"), "_OperationMixin".into()));
        }
        files.insert(format!("operations/{name}.py"), code);
        operation_exports.push((name.clone(), vec![name.clone()]));
        methods.insert(op.name.clone(), name);
        signatures.insert(op.name.clone(), (vars, result, annotation));
    }
    if matches!(style, GraphqlStyle::Idiomatic) {
        let mut selected = groups.clone();
        if selected.is_empty() {
            for op in &selected_operations {
                selected
                    .entry(
                        match op.kind {
                            GraphqlOperationKind::Query => "query",
                            GraphqlOperationKind::Mutation => "mutation",
                            GraphqlOperationKind::Subscription => "subscription",
                        }
                        .into(),
                    )
                    .or_default()
                    .insert(methods[&op.name].clone(), op.name.clone());
            }
        }
        let mut group_names = BTreeSet::new();
        for (index, (group, entries)) in selected.iter().enumerate() {
            let group_name = python_identifier(group);
            ensure!(
                group_names.insert(group_name.clone())
                    && !names.contains(&group_name)
                    && !matches!(group_name.as_str(), "_transport" | "execute"),
                "Python GraphQL group collision {group_name}"
            );
            let mut group_bases = vec![];
            let mut normalized = BTreeSet::new();
            for (method, operation) in entries {
                let method = python_identifier(method);
                ensure!(
                    normalized.insert(method.clone()) && method != "_transport",
                    "Python GraphQL group method collision"
                );
                let (vars, _result, annotation) = signatures.get(operation).ok_or_else(|| {
                    anyhow::anyhow!("unknown GraphQL grouped operation {operation}")
                })?;
                let op = selected_operations
                    .iter()
                    .find(|op| &op.name == operation)
                    .unwrap();
                let parameter = if op.variables.iter().all(|v| v.optional) {
                    format!("variables: Optional[{vars}] = None")
                } else {
                    format!("variables: {vars}")
                };
                let file = format!("{group_name}_{method}");
                files.insert(format!("groups/{file}.py"),format!("from typing import Optional, Iterator\nfrom ..models import *\nfrom ..operations.{} import {}\nfrom ..runtime import GraphqlResponse, Transport, IncrementalFrame\nclass _MethodMixin:\n    _transport: Transport\n    def {method}(self, {parameter}) -> {annotation}:\n        return {}(self._transport, variables)\n",methods[operation],methods[operation],methods[operation]));
                group_bases.push((format!("..groups.{file}"), "_MethodMixin".into()));
            }
            let root = layout::mixins(&mut files, &format!("group_{group_name}"), group_bases)?;
            let (imports, base) = base_import(root, true);
            files.insert(format!("groups/{group_name}.py"),format!("{imports}class _Group{index}{base}:\n    def __init__(self, transport):\n        self._transport = transport\n\nclass _AccessorMixin:\n    @property\n    def {group_name}(self) -> _Group{index}:\n        return _Group{index}(self._transport)\n"));
            client_bases.push((format!("..groups.{group_name}"), "_AccessorMixin".into()));
        }
    }
    let root = layout::mixins(&mut files, "client", client_bases)?;
    let (imports, base) = base_import(root, false);
    files.insert("client.py".into(),format!("from .runtime import Transport\n{imports}class Client{base}:\n    def __init__(self, endpoint: str, *, headers=None, timeout: float=30, scalar_codecs=None, max_frame_bytes=1024*1024):\n        self._transport=Transport(endpoint, headers=headers, timeout=timeout, scalar_codecs=scalar_codecs, max_frame_bytes=max_frame_bytes)\n"));
    let model_exports = layout::facade(&mut models.files, "models", &models.exports);
    models.files.insert("models/__init__.py".into(),format!("{model_exports}\nimport sys as _sys\n_exports = {{name: globals()[name] for name in __all__}}\nfor _name, _module in list(_sys.modules.items()):\n    if _name.startswith(__name__ + '.'):\n        vars(_module).update({{name: _exports[name] for name in getattr(_module, '__poolster_refs__', ())}})\n"));
    models
        .files
        .insert("models/_parts/__init__.py".into(), String::new());
    files.extend(models.files);
    let operation_exports = layout::facade(&mut files, "operations", &operation_exports);
    files.insert("operations/__init__.py".into(), operation_exports);
    files.insert("groups/__init__.py".into(), String::new());
    files.insert("_mixins/__init__.py".into(), String::new());
    files.insert(
        "runtime.py".into(),
        include_str!("../templates/graphql_runtime.py").into(),
    );
    files.insert(
        "codecs.py".into(),
        include_str!("../templates/graphql_codecs.py").into(),
    );
    files.insert(
        "streaming.py".into(),
        include_str!("../templates/graphql_streaming.py").into(),
    );
    let inputs = contract
        .input_objects
        .iter()
        .map(|(name, fields)| {
            (
                name.clone(),
                poolster_core::native::graphql_scalar_fields(fields),
            )
        })
        .collect::<BTreeMap<_, _>>();
    files.insert("_codec_inputs/__init__.py".into(), String::new());
    codec_layout::emit(&serde_json::to_value(inputs)?, "all", &mut files)?;
    files.insert(
        "codec_shapes.py".into(),
        "from ._codec_inputs.all import VALUE as INPUT_SHAPES\n".into(),
    );
    files.insert("__init__.py".into(),"from .client import Client\nfrom .runtime import GraphqlResponse, GraphqlErrors, Transport\nfrom .models import *\n".into());
    files.insert("py.typed".into(), String::new());
    let mut tree = GeneratedTree::default();
    for (path, source) in files {
        tree.insert(GeneratedFile::new(format!("src/{module}/{path}"), source)?)?;
    }
    tree.insert(GeneratedFile::new("README.md", "# GraphQL Python client\n\nRequires Python 3.9+. Flat clients expose snake_case operation methods; idiomatic clients additionally expose query/mutation or configured groups. Raw exports use an explicit Transport. Variables and selection-specific results are TypedDicts: omitted keys differ from explicit None. GraphqlResponse preserves data, errors and extensions; inspect status or call require_data() to reject partial results. urllib transport exceptions remain distinct. Custom scalars retain Any application values; per-client scalar_codecs encode/decode callbacks walk selected fields and recursive input models. Opt-in subscriptions use graphql-sse distinct connections; incremental input uses multipart/mixed deferSpec=20220824 frames with final envelopes. No reconnect/replay, single-connection SSE or async transport.\n")?)?;
    tree.insert(GeneratedFile::new("pyproject.toml",format!("[build-system]\nrequires = [\"setuptools>=68\"]\nbuild-backend = \"setuptools.build_meta\"\n[project]\nname = {distribution:?}\nversion = \"0.0.0\"\nrequires-python = \">=3.9\"\n[tool.setuptools.packages.find]\nwhere = [\"src\"]\n[tool.setuptools.package-data]\n\"*\" = [\"py.typed\"]\n"))?)?;
    source_layout::diagnostics(&mut tree)?;
    Ok((tree, methods))
}
#[cfg(test)]
mod tests;

mod source_layout;
