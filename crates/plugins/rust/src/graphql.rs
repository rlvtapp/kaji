//! GraphQL operations render from owned contracts, independently of HTTP Api.
mod models;
mod scalars;
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
}
impl Contract for GraphqlClient {
    const NAME: &'static str = "poolster.rust.graphql-client.v1";
}
pub struct Graphql {
    meta: Meta,
    provider: Option<Handle<GraphqlOperations>>,
    scalars: BTreeMap<String, GraphqlScalarMapping>,
}
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        scalars: BTreeMap::new(),
    }
}
impl Graphql {
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
        let mappings = scalars::validate_mappings(&self.scalars, contract)?;
        let mut models = models::Models::with_mappings(contract, &mappings);
        models.input_objects()?;
        let mut source = String::new();
        let mut names = BTreeSet::new();
        let mut wire_names = BTreeSet::new();
        let mut operations = BTreeMap::new();
        for op in &contract.operations {
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
            writeln!(
                source,
                "pub async fn {function}(transport: &crate::graphql_runtime::GraphqlHttpTransport, variables: &{variables}) -> Result<crate::graphql_runtime::GraphqlResponse<{result}>, crate::graphql_runtime::GraphqlTransportError> {{\n    transport.execute({}, {}, variables).await\n}}\n",
                serde_json::to_string(&op.document)?,
                serde_json::to_string(&op.name)?
            )?;
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
        cx.files.emit(GeneratedFile::new(
            "src/graphql.rs",
            models.source + &source,
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "src/graphql_runtime.rs",
            include_str!("../templates/graphql_runtime.rs.tmpl"),
        )?)?;
        cx.files.emit(GeneratedFile::new("README.md","# Rust GraphQL client\n\nSelection-specific query/mutation functions use a caller-provided reqwest Client and endpoint. Results distinguish Success, Partial and Error; transport failures are separate. Presence::Absent, Null and Value preserve nullable input and conditional-result presence. Optional::Absent/Value preserves nonnullable optional fields. Unmapped custom scalars retain serde_json::Value. Configured input/output scalar mappings describe self-contained Rust JSON wire types; they do not install codecs. Subscription and incremental transports are unsupported. Abstract selections without __typename use structural untagged unions; structurally indistinguishable variants cannot identify a concrete GraphQL type. Native schema/operation documents remain in the input contract.\n")?)?;
        cx.workspace.graphql_package = Some(NativePackage { name, version });
        cx.publish(GraphqlClient { operations })
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
        cx.files.emit(GeneratedFile::new("src/lib.rs","pub mod graphql;\npub mod graphql_runtime;\npub use graphql::*;\npub use graphql_runtime::*;\npub use reqwest;\n")?)?;
        let manifest = format!(
            "[package]\nname = {:?}\nversion = {:?}\nedition = \"2024\"\n\n[dependencies]\nreqwest = {{ version = \"=0.12.28\", default-features = false, features = [\"json\", \"rustls-tls\"] }}\nserde = {{ version = \"=1.0.229\", features = [\"derive\"] }}\nserde_json = \"=1.0.151\"\n",
            self.name, self.version
        );
        cx.files.emit(GeneratedFile::new("Cargo.toml", manifest)?)
    }
}
