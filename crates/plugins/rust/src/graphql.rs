//! GraphQL operations render from owned contracts, independently of HTTP Api.
mod models;
mod scalars;
mod styles;
use crate::Rust;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{GraphqlOperationKind, GraphqlOperations},
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
    scalars: BTreeMap<String, GraphqlScalarMapping>,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        scalars: BTreeMap::new(),
        style: GraphqlStyle::Idiomatic,
        groups: BTreeMap::new(),
    }
}
impl Graphql {
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
        vec![Requirement::on(self.provider)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        let mut oversized = Vec::new();
        let contract = cx.inputs.get::<GraphqlOperations>()?;
        ensure!(
            !contract.operations.is_empty(),
            "Rust GraphQL generation requires operations"
        );
        ensure!(
            contract.operations.iter().all(|op| matches!(
                op.kind,
                GraphqlOperationKind::Query | GraphqlOperationKind::Mutation
            )),
            "Rust GraphQL supports query/mutation HTTP operations; subscriptions require an unsupported separate transport"
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
            let mut operation_source = String::new();
            writeln!(
                operation_source,
                "pub async fn {function}(transport: &crate::graphql_runtime::GraphqlHttpTransport, variables: &{variables}) -> Result<crate::graphql_runtime::GraphqlResponse<{result}>, crate::graphql_runtime::GraphqlTransportError> {{\n    transport.execute({}, {}, variables).await\n}}\n",
                serde_json::to_string(&op.document)?,
                serde_json::to_string(&op.name)?
            )?;
            operation_files.insert(function.clone(), operation_source);
            operations.insert(
                op.name.clone(),
                GraphqlOperationSymbols {
                    function,
                    variables,
                    result,
                    kind: op.kind,
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
        emit_source(
            &mut cx.files,
            GeneratedFile::new(
                "README.md",
                "# Rust GraphQL client\n\nSelection-specific query/mutation functions use a caller-provided reqwest Client and endpoint. Results distinguish Success, Partial and Error; transport failures are separate. Presence::Absent, Null and Value preserve nullable input and conditional-result presence. Optional::Absent/Value preserves nonnullable optional fields. Unmapped custom scalars retain serde_json::Value. Configured input/output scalar mappings describe self-contained Rust JSON wire types; they do not install codecs. Subscription and incremental transports are unsupported. Abstract selections without __typename use structural untagged unions; structurally indistinguishable variants cannot identify a concrete GraphQL type. Native schema/operation documents remain in the input contract.\n",
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
        cx.files.emit(GeneratedFile::new("src/lib.rs", "pub mod graphql;\npub mod graphql_runtime;\npub use graphql::*;\npub use graphql_runtime::*;\npub use reqwest;\n")?)?;
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
