//! Fixed-operation native GraphQL command-line packages.
mod render;
use crate::RustCli;
use anyhow::{Context, Result, ensure};
use poolster_core::{
    GeneratedFile,
    blocks::Blocks,
    engine::{Handle, Meta, Plugin, PluginContext, Requirement},
    native::{GraphqlOperation, GraphqlOperationKind, GraphqlOperations},
};
use std::collections::BTreeSet;

pub struct Graphql {
    meta: Meta,
    input: Option<Handle<GraphqlOperations>>,
    operations: Option<Handle<Blocks<GraphqlOperation>>>,
    command_name: String,
    endpoint: Option<String>,
}
/// Generate named query/mutation commands from validated operation documents.
pub fn graphql() -> Graphql {
    Graphql {
        meta: Meta::new(),
        input: None,
        operations: None,
        command_name: "graphql-client".into(),
        endpoint: None,
    }
}
impl Graphql {
    pub fn input(mut self, input: Handle<GraphqlOperations>) -> Self {
        self.input = Some(input);
        self
    }
    pub fn input_operations(mut self, operations: Handle<Blocks<GraphqlOperation>>) -> Self {
        self.operations = Some(operations);
        self
    }
    pub fn command_name(mut self, name: impl Into<String>) -> Self {
        self.command_name = name.into();
        self
    }
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }
}
impl Plugin<RustCli> for Graphql {
    fn kind(&self) -> &'static str {
        "rust-cli-graphql"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut requirements = vec![Requirement::on(self.input)];
        if let Some(operations) = self.operations {
            requirements.push(Requirement::on(Some(operations)));
        }
        requirements
    }
    fn generate(&self, cx: &mut PluginContext<'_, RustCli>) -> Result<()> {
        let contract = cx.inputs.get::<GraphqlOperations>()?;
        let mut operations = if self.operations.is_some() {
            let blocks = cx.inputs.get::<Blocks<GraphqlOperation>>()?;
            blocks.validate()?;
            blocks.require_complete()?;
            blocks.require_parent(cx.inputs.reference::<GraphqlOperations>()?.context(
                "selected GraphQL blocks require authoritative whole-contract provenance",
            )?)?;
            blocks
                .items
                .iter()
                .map(|item| item.value.clone())
                .collect::<Vec<_>>()
        } else {
            contract.operations.clone()
        };
        operations.sort_by(|a, b| a.name.cmp(&b.name));
        ensure!(
            !operations.is_empty(),
            "Rust GraphQL CLI requires operation documents"
        );
        ensure!(
            operations
                .iter()
                .all(|op| op.kind != GraphqlOperationKind::Subscription),
            "Rust GraphQL CLI supports query/mutation HTTP operations; subscriptions require a separately supported transport"
        );
        let package = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or(&self.command_name);
        for name in [package, &self.command_name] {
            ensure!(
                !name.is_empty()
                    && name.starts_with(|c: char| c.is_ascii_alphabetic())
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')),
                "invalid Rust GraphQL CLI package/command name {name:?}"
            );
        }
        let mut commands = BTreeSet::new();
        for op in &operations {
            let command = crate::kebab_case(&op.name);
            ensure!(
                !command.is_empty()
                    && !matches!(command.as_str(), "help" | "version")
                    && commands.insert(command.clone()),
                "reserved or colliding GraphQL command {command:?}"
            );
        }
        for (path, source) in render::files(
            package,
            &self.command_name,
            self.endpoint.as_deref(),
            &operations,
        )? {
            cx.files.emit(GeneratedFile::new(path, source)?)?;
        }
        Ok(())
    }
}
