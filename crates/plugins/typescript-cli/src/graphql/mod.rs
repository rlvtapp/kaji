//! Native operation-document GraphQL CLI generation, separate from HTTP commands.
use crate::TypeScriptCli;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile, GeneratedTree,
    blocks::{Blocks, ContractReference},
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Requirement},
    native::{GraphqlOperation, GraphqlOperationKind, GraphqlOperations},
};
use std::{collections::BTreeSet, fmt::Write};

pub struct Graphql {
    meta: Meta,
    input: Option<Handle<GraphqlOperations>>,
    blocks: Option<Handle<Blocks<GraphqlOperation>>>,
    command_name: Option<String>,
    endpoint: Option<String>,
}
pub fn graphql() -> Graphql {
    Graphql {
        meta: Meta::new(),
        input: None,
        blocks: None,
        command_name: None,
        endpoint: None,
    }
}
impl Graphql {
    pub fn input(mut self, input: Handle<GraphqlOperations>) -> Self {
        self.input = Some(input);
        self
    }
    pub fn blocks(mut self, blocks: Handle<Blocks<GraphqlOperation>>) -> Self {
        self.blocks = Some(blocks);
        self
    }
    pub fn input_operations(self, blocks: Handle<Blocks<GraphqlOperation>>) -> Self {
        self.blocks(blocks)
    }
    pub fn command_name(mut self, name: impl Into<String>) -> Self {
        self.command_name = Some(name.into());
        self
    }
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }
}
impl Plugin<TypeScriptCli> for Graphql {
    fn kind(&self) -> &'static str {
        "typescript-cli-graphql"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![
            Requirement::on(self.input),
            if self.blocks.is_some() {
                Requirement::on(self.blocks)
            } else {
                Requirement::on(self.blocks).optional()
            },
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScriptCli>) -> Result<()> {
        let contract = cx.inputs.get::<GraphqlOperations>()?;
        let reference = cx.inputs.reference::<GraphqlOperations>()?;
        let derived;
        let blocks = if let Some(blocks) = cx.inputs.optional::<Blocks<GraphqlOperation>>()? {
            blocks
        } else {
            derived = contract.operation_blocks(
                reference
                    .map(|r| r.instance.as_str())
                    .unwrap_or("selected-graphql"),
            );
            &derived
        };
        let blocks = if cx.inputs.optional::<Blocks<GraphqlOperation>>()?.is_none() {
            blocks.clone().with_parent(
                reference
                    .cloned()
                    .unwrap_or_else(|| blocks.parent.clone().unwrap()),
            )
        } else {
            blocks.clone()
        };
        validate(contract, &blocks, reference)?;
        let command = self.command_name.as_deref().unwrap_or("graphql");
        let package = cx.settings.package_name.as_deref().unwrap_or("graphql-cli");
        let version = cx.common.package_version.as_deref().unwrap_or("0.0.0");
        cx.files.append(render(
            contract,
            &blocks,
            command,
            package,
            version,
            self.endpoint.as_deref(),
        )?)
    }
}
fn validate(
    contract: &GraphqlOperations,
    blocks: &Blocks<GraphqlOperation>,
    reference: Option<&ContractReference>,
) -> Result<()> {
    blocks.validate()?;
    blocks.require_complete()?;
    blocks.require_nonempty()?;
    let parent = blocks
        .parent
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("GraphQL operation blocks require provenance"))?;
    let expected = ContractReference::from_bytes(
        GraphqlOperations::NAME,
        parent.instance.clone(),
        &serde_json::to_vec(contract)?,
    );
    blocks.require_parent(reference.unwrap_or(&expected))?;
    ensure!(
        blocks
            .items
            .iter()
            .map(|b| &b.value)
            .eq(contract.operations.iter()),
        "GraphQL CLI operation blocks differ from selected contract"
    );
    ensure!(
        blocks
            .items
            .iter()
            .all(|b| b.value.kind != GraphqlOperationKind::Subscription),
        "GraphQL CLI subscriptions require an unsupported separate transport"
    );
    Ok(())
}
fn render(
    contract: &GraphqlOperations,
    blocks: &Blocks<GraphqlOperation>,
    command: &str,
    package: &str,
    version: &str,
    endpoint: Option<&str>,
) -> Result<GeneratedTree> {
    validate(contract, blocks, None)?;
    ensure!(
        !command.is_empty()
            && command
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')),
        "invalid GraphQL CLI command name"
    );
    let mut tree = GeneratedTree::default();
    let mut index = String::from("#!/usr/bin/env node\nimport { Command } from 'commander';\n");
    let mut sorted = blocks.items.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.value.name.cmp(&b.value.name));
    let mut used = BTreeSet::new();
    for block in &sorted {
        let op = &block.value;
        let name = crate::kebab_case(&op.name);
        ensure!(
            !name.is_empty() && used.insert(name.clone()),
            "GraphQL CLI operation command collision: {name}"
        );
        let required = op
            .variables
            .iter()
            .filter(|v| !v.optional)
            .map(|v| v.name.clone())
            .collect::<Vec<_>>();
        let nonnull = op
            .variables
            .iter()
            .filter(|v| !v.ty.nullable)
            .map(|v| v.name.clone())
            .collect::<Vec<_>>();
        let data = serde_json::json!({"name":op.name,"command":name,"kind":if op.kind==GraphqlOperationKind::Query {"query"}else{"mutation"},"document":op.document,"required":required,"nonnull":nonnull});
        let file = poolster_core::files::source_file_stem(&op.name);
        tree.insert(GeneratedFile::new(
            format!("src/operations/{file}.ts"),
            format!(
                "export default {} as const;\n",
                serde_json::to_string_pretty(&data)?
            ),
        )?)?;
    }
    for (chunk, operations) in sorted.chunks(50).enumerate() {
        let mut source = String::from(
            "import type { Command } from 'commander';\nimport { register } from '../runtime.js';\n",
        );
        for (i, block) in operations.iter().enumerate() {
            writeln!(
                source,
                "import operation{i} from '../operations/{}.js';",
                poolster_core::files::source_file_stem(&block.value.name)
            )?;
        }
        source.push_str(
            "export function configure(program: Command, endpoint: string | null): void {\n",
        );
        for i in 0..operations.len() {
            writeln!(source, "  register(program, operation{i}, endpoint);")?;
        }
        source.push_str("}\n");
        tree.insert(GeneratedFile::new(
            format!("src/commands/commands_{chunk:03}.ts"),
            source,
        )?)?;
        writeln!(
            index,
            "import {{ configure as configure{chunk} }} from './commands/commands_{chunk:03}.js';"
        )?;
    }
    writeln!(
        index,
        "const program = new Command().name({}).description('Generated GraphQL operation client');",
        serde_json::to_string(command)?
    )?;
    for chunk in 0..sorted.len().div_ceil(50) {
        writeln!(
            index,
            "configure{chunk}(program, {});",
            serde_json::to_string(&endpoint)?
        )?;
    }
    index.push_str("program.exitOverride();\nawait program.parseAsync(process.argv).catch(error => { if (error && error.exitCode === 0) return; console.error(error instanceof Error ? error.message : String(error)); process.exitCode = 2; });\n");
    tree.insert(GeneratedFile::new("src/index.ts", index)?)?;
    tree.insert(GeneratedFile::new(
        "src/runtime.ts",
        include_str!("runtime.ts"),
    )?)?;
    tree.insert(GeneratedFile::new(
        "tsconfig.json",
        crate::render_tsconfig(),
    )?)?;
    tree.insert(GeneratedFile::new(
        "package.json",
        crate::render_package_json(package, command, version),
    )?)?;
    tree.insert(GeneratedFile::new("README.md",format!("# GraphQL CLI\n\nInstall dependencies and run `npm run build`.\n\n```sh\n{command} <operation-name> --endpoint https://example.test/graphql --variables '{{\"id\":\"123\"}}'\n{command} <operation-name> --variables-file variables.json --header 'Authorization: Bearer token'\n```\n\nConfigure the endpoint with `--endpoint` or `GRAPHQL_ENDPOINT`; `GRAPHQL_TOKEN` adds bearer authentication unless an Authorization header is supplied. Variables are complete JSON objects; omission differs from explicit null. Output is the full GraphQL envelope. Exit codes: 0 success, 3 partial data with errors, 4 GraphQL errors without data, 2 invalid options/transport/HTTP/malformed response. Query and mutation operations only; subscriptions and incremental delivery are unsupported. No OpenAPI OAuth login commands are implied.\n"))?)?;
    Ok(tree)
}
#[cfg(test)]
mod tests;
