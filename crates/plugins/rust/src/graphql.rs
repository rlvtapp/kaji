//! GraphQL operations render from owned contracts, independently of HTTP Api.
mod models;
mod scalars;
mod styles;
use crate::Rust;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{
        GraphqlIncrementalOperations, GraphqlOperationKind, GraphqlOperations,
        graphql_scalar_fields, graphql_scalar_shape,
    },
};
pub use scalars::GraphqlScalarMapping;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
pub use styles::GraphqlStyle;
#[derive(Clone, Debug)]
pub struct GraphqlOperationSymbols {
    pub function: String,
    pub variables: String,
    pub result: String,
    pub kind: GraphqlOperationKind,
    pub incremental: bool,
}
#[derive(Clone, Debug)]
pub struct GraphqlClient {
    pub operations: BTreeMap<String, GraphqlOperationSymbols>,
    pub style: GraphqlStyle,
    pub methods: BTreeMap<String, String>,
}
impl Contract for GraphqlClient {
    const NAME: &'static str = "poolster.rust.graphql-client.v1";
}
pub struct Graphql {
    meta: Meta,
    provider: Option<Handle<GraphqlOperations>>,
    incremental_provider: Option<Handle<GraphqlIncrementalOperations>>,
    subscriptions: bool,
    incremental_mode: bool,
    scalars: BTreeMap<String, GraphqlScalarMapping>,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        incremental_provider: None,
        subscriptions: false,
        incremental_mode: false,
        scalars: BTreeMap::new(),
        style: GraphqlStyle::Idiomatic,
        groups: BTreeMap::new(),
    }
}
/// Generate multipart incremental operations from the distinct capability contract.
pub fn graphql_incremental(provider: Option<Handle<GraphqlIncrementalOperations>>) -> Graphql {
    let mut generator = graphql(None);
    generator.incremental_provider = provider;
    generator.incremental_mode = true;
    generator
}
impl Graphql {
    /// Enable distinct-connection graphql-sse subscription operations.
    pub fn subscriptions(mut self) -> Self {
        self.subscriptions = true;
        self
    }
    /// Emit free operation functions taking an explicit HTTP transport.
    pub fn raw(mut self) -> Self {
        self.style = GraphqlStyle::Raw;
        self
    }
    /// Add bound snake_case methods such as `client.read_user(&variables).await`.
    /// Operations without variable declarations take no method arguments.
    pub fn flat(mut self) -> Self {
        self.style = GraphqlStyle::Flat;
        self
    }
    /// Group bound methods by operation kind: `client.query().read_user(&variables)`.
    /// Custom groups can override the operation-kind grouping.
    pub fn idiomatic(mut self) -> Self {
        self.style = GraphqlStyle::Idiomatic;
        self
    }
    pub fn namespaced(self) -> Self {
        self.idiomatic()
    }
    /// Assign an operation to a custom group and method, normalized to snake_case.
    /// This requires idiomatic style; unassigned operations keep their kind group.
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
    pub fn groups(mut self, groups: BTreeMap<String, BTreeMap<String, String>>) -> Self {
        self.groups = groups;
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

    pub fn handle(&self) -> Handle<GraphqlClient> {
        self.meta.handle()
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
}
impl Plugin<Rust> for Graphql {
    fn kind(&self) -> &'static str {
        "rust-graphql"
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
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        let mut oversized = Vec::new();
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
        ensure!(
            !contract.operations.is_empty(),
            "Rust GraphQL generation requires operations"
        );
        ensure!(
            contract
                .operations
                .iter()
                .all(|op| op.kind != GraphqlOperationKind::Subscription
                    || (self.subscriptions && !self.incremental_mode)),
            "Rust GraphQL subscriptions require .subscriptions(); incremental subscriptions are unsupported"
        );
        ensure!(
            cx.workspace.graphql_package.is_none(),
            "Rust package already contains a GraphQL generator"
        );
        ensure!(
            !cx.workspace.models && !cx.workspace.operations && cx.workspace.http_api.is_none(),
            "HTTP and GraphQL generators require separate Rust packages"
        );
        let name = cx
            .settings
            .package_name
            .clone()
            .unwrap_or_else(|| "poolster-graphql-client".into());
        ensure!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()),
            "invalid Rust GraphQL package name"
        );
        let version = cx
            .common
            .package_version
            .clone()
            .unwrap_or_else(|| "0.0.0".into());
        let layout = styles::layout(self.style, contract, &self.groups)?;
        let mappings = scalars::validate_mappings(&self.scalars, contract)?;
        let mut models = models::Models::with_reserved(contract, &mappings, &layout.reserved());
        models.input_objects()?;
        let mut operation_files = BTreeMap::new();
        let mut names = BTreeSet::new();
        let mut wire_names = BTreeSet::new();
        let mut operations = BTreeMap::new();
        let mut ordered_operations: Vec<_> = contract.operations.iter().collect();
        ordered_operations.sort_by_key(|op| &op.name);
        for op in ordered_operations {
            ensure!(
                wire_names.insert(&op.name),
                "duplicate GraphQL operation {}",
                op.name
            );
            let variables = models.variables(
                &format!("{}Variables", crate::render::type_name(&op.name)),
                &op.variables,
            )?;
            let result = models.result(
                &format!("{}Result", crate::render::type_name(&op.name)),
                &op.result,
            )?;
            let base = crate::render::rust_field_name(&op.name);
            let mut function = base.clone();
            let mut suffix = 2;
            while !names.insert(function.clone()) {
                function = format!("{base}{suffix}");
                suffix += 1;
            }
            let shape = serde_json::json!({"input":graphql_scalar_fields(&op.variables),"result":graphql_scalar_shape(&op.result),"selections":incremental.and_then(|value|value.selections.get(&op.name))});
            let response = if self.incremental_mode {
                format!("crate::graphql_incremental::GraphqlIncrementalStream<{result}>")
            } else if op.kind == GraphqlOperationKind::Subscription {
                format!("crate::graphql_sse::GraphqlSubscription<{result}>")
            } else {
                format!("crate::graphql_runtime::GraphqlResponse<{result}>")
            };
            let execute = if self.incremental_mode {
                "incremental_with_shape"
            } else if op.kind == GraphqlOperationKind::Subscription {
                "subscribe_with_shape"
            } else {
                "execute_with_shape"
            };
            let mut operation_source = String::new();
            writeln!(
                operation_source,
                "pub async fn {function}(transport: &crate::graphql_runtime::GraphqlHttpTransport, variables: &{variables}) -> Result<{response}, crate::graphql_runtime::GraphqlTransportError> {{\n    transport.{execute}({}, {}, variables, {}).await\n}}\n",
                serde_json::to_string(&op.document)?,
                serde_json::to_string(&op.name)?,
                serde_json::to_string(&serde_json::to_string(&shape)?)?
            )?;
            operation_files.insert(function.clone(), operation_source);
            operations.insert(
                op.name.clone(),
                GraphqlOperationSymbols {
                    function,
                    variables,
                    result,
                    kind: op.kind,
                    incremental: self.incremental_mode,
                },
            );
        }
        let client_files = layout.render_files(self.style, &operations)?;
        let mut entry =
            String::from("// Generated by Poolster.\nuse serde::{Deserialize, Serialize};\n");
        for wrapper in ["Presence", "Optional"] {
            if models
                .files
                .values()
                .any(|source| source.contains(&format!("{wrapper}<")))
            {
                writeln!(entry, "use crate::graphql_runtime::{wrapper};")?;
            }
        }
        for (category, files) in [
            ("models", models.files),
            ("operations", operation_files),
            ("client", client_files),
        ] {
            let files: Vec<_> = files.into_iter().collect();
            for (index, chunk) in files.chunks(64).enumerate() {
                let mut index_source = String::from("// Generated by Poolster.\n");
                for (name, content) in chunk {
                    let name = name
                        .split('/')
                        .map(poolster_core::files::source_file_stem)
                        .collect::<Vec<_>>()
                        .join("/");
                    writeln!(index_source, "include!(\"../{name}.rs\");")?;
                    emit_source(
                        &mut cx.files,
                        GeneratedFile::new(
                            format!("src/graphql/{category}/{name}.rs"),
                            content.clone(),
                        )?,
                        &mut oversized,
                    )?;
                }
                writeln!(
                    entry,
                    "include!(\"graphql/{category}/_exports/part_{index}.rs\");"
                )?;
                emit_source(
                    &mut cx.files,
                    GeneratedFile::new(
                        format!("src/graphql/{category}/_exports/part_{index}.rs"),
                        index_source,
                    )?,
                    &mut oversized,
                )?;
            }
        }
        emit_source(
            &mut cx.files,
            GeneratedFile::new("src/graphql.rs", entry)?,
            &mut oversized,
        )?;
        emit_source(
            &mut cx.files,
            GeneratedFile::new(
                "src/graphql_runtime.rs",
                include_str!("../templates/graphql_runtime.rs.tmpl"),
            )?,
            &mut oversized,
        )?;
        for (path, source) in [
            (
                "src/graphql_codecs.rs",
                include_str!("../templates/graphql_codecs.rs.tmpl"),
            ),
            (
                "src/graphql_sse.rs",
                include_str!("../templates/graphql_sse.rs.tmpl"),
            ),
            (
                "src/graphql_incremental.rs",
                include_str!("../templates/graphql_incremental.rs.tmpl"),
            ),
        ] {
            emit_source(
                &mut cx.files,
                GeneratedFile::new(path, source)?,
                &mut oversized,
            )?;
        }
        let input_shapes = contract
            .input_objects
            .iter()
            .map(|(name, fields)| (name.clone(), graphql_scalar_fields(fields)))
            .collect::<BTreeMap<_, _>>();
        emit_source(
            &mut cx.files,
            GeneratedFile::new(
                "src/graphql_input_shapes.json",
                serde_json::to_string(&input_shapes)?,
            )?,
            &mut oversized,
        )?;
        emit_source(
            &mut cx.files,
            GeneratedFile::new(
                "README.md",
                "# Rust GraphQL client\n\nSelection-specific query/mutation functions use a caller-provided reqwest Client and endpoint. Results distinguish Success, Partial and Error; transport failures are separate. Presence::Absent, Null and Value preserve nullable input and conditional-result presence. Optional::Absent/Value preserves nonnullable optional fields. Unmapped custom scalars retain serde_json::Value. Configured input/output mappings define Rust model types; GraphqlScalarCodecs registers typed runtime encode/decode callbacks without changing null or omission semantics. Enabled subscriptions use distinct-connection graphql-sse next/complete streams; dropping a stream closes its connection, with no automatic reconnect/replay. Incremental packages negotiate multipart/mixed deferSpec=20220824; snapshots are recursively partial JSON and Complete validates the final selected model. Other incremental dialects are rejected. Abstract selections without __typename use structural untagged unions; structurally indistinguishable variants cannot identify a concrete GraphQL type. Native schema/operation documents remain in the input contract.\n",
            )?,
            &mut oversized,
        )?;
        cx.workspace.graphql_package = Some(NativePackage { name, version });
        if !oversized.is_empty() {
            cx.files.emit(GeneratedFile::new(
                ".poolster/source-layout-diagnostics.json",
                serde_json::to_string_pretty(&oversized)?,
            )?)?;
        }
        cx.publish(GraphqlClient {
            operations,
            style: self.style,
            methods: layout.methods,
        })
    }
}
pub(crate) struct NativePackage {
    name: String,
    version: String,
}
impl NativePackage {
    pub(crate) fn finalize(
        &self,
        cx: &mut poolster_core::engine::FinalizeContext<'_, Rust>,
    ) -> Result<()> {
        ensure!(
            !cx.workspace.models && !cx.workspace.operations && cx.workspace.http_api.is_none(),
            "HTTP and GraphQL generators require separate Rust packages"
        );
        cx.files.emit(GeneratedFile::new("src/lib.rs", "pub mod graphql;\npub mod graphql_runtime;\npub mod graphql_codecs;\npub mod graphql_sse;\npub mod graphql_incremental;\npub use graphql_codecs::*;\npub use graphql_sse::*;\npub use graphql_incremental::*;\npub use graphql::*;\npub use graphql_runtime::*;\npub use reqwest;\n")?)?;
        let manifest = format!(
            "[package]\nname = {:?}\nversion = {:?}\nedition = \"2024\"\n\n[dependencies]\nreqwest = {{ version = \"=0.12.28\", default-features = false, features = [\"json\", \"rustls-tls\"] }}\nserde = {{ version = \"=1.0.229\", features = [\"derive\"] }}\nserde_json = \"=1.0.151\"\n",
            self.name, self.version
        );
        cx.files.emit(GeneratedFile::new("Cargo.toml", manifest)?)
    }
}

fn emit_source(
    files: &mut poolster_core::engine::Emitter<'_>,
    file: GeneratedFile,
    oversized: &mut Vec<serde_json::Value>,
) -> Result<()> {
    if file.contents.len() > 128 * 1024 {
        oversized.push(serde_json::json!({"path": file.path, "bytes": file.contents.len(), "max_file_bytes": 128 * 1024, "reason": "GraphQL source exceeds the grouping budget; retained intact. Review selection size or layout."}));
    }
    files.emit(file)
}
