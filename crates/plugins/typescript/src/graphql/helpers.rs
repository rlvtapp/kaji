//! Selection-specific GraphQL companions, independent of HTTP API adaptation.
mod mocks;
mod models;
use crate::{CypressOptions, FixtureOptions, GraphqlClient, TypeScript};
use anyhow::{Result, ensure};
use poolster_core::{GeneratedFile, engine::PluginContext, native::GraphqlOperationKind};
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Zod,
    Faker,
    Msw,
    Cypress,
}
pub(crate) fn generate(
    cx: &mut PluginContext<'_, TypeScript>,
    client: &GraphqlClient,
    kind: Kind,
    output: &str,
    fixtures: &FixtureOptions,
    cypress: &CypressOptions,
    max_file_bytes: usize,
) -> Result<()> {
    if !matches!(kind, Kind::Faker) {
        ensure!(
            *fixtures == FixtureOptions::default(),
            "GraphQL fixture_options only apply to Faker"
        );
    }
    if !matches!(kind, Kind::Cypress) {
        ensure!(
            *cypress == CypressOptions::default(),
            "GraphQL cypress_options only apply to Cypress"
        );
    }
    if matches!(kind, Kind::Faker) {
        ensure!(
            fixtures.max_attempts == FixtureOptions::default().max_attempts,
            "GraphQL Faker does not support max_attempts; fixtures are structural samples"
        );
    }
    let target = GeneratedFile::new(format!("{output}.ts"), "")?.path;
    ensure!(
        client.definition.operations.iter().all(|op| client
            .operations
            .get(&op.name)
            .is_some_and(|symbols| symbols.kind == op.kind)),
        "GraphQL client omitted operation symbols"
    );
    if matches!(kind, Kind::Msw | Kind::Cypress) {
        ensure!(
            client
                .definition
                .operations
                .iter()
                .all(|op| op.kind != GraphqlOperationKind::Subscription),
            "GraphQL MSW/Cypress helpers support query/mutation HTTP operations; subscriptions need a separate transport"
        );
    }
    let source = match kind {
        Kind::Zod => models::zod(client, &target)?,
        Kind::Faker => models::faker(client, &target, fixtures)?,
        Kind::Msw => mocks::msw(client, &target)?,
        Kind::Cypress => mocks::cypress(client, &target, cypress)?,
    };
    ensure!(
        source.len() <= max_file_bytes,
        "GraphQL helper exceeds max_file_bytes; source chunking is not yet supported"
    );
    cx.files.emit(GeneratedFile::new(&target, source)?)?;
    match kind {
        Kind::Zod => cx.workspace.dependency("zod", "^4.0.0")?,
        Kind::Faker => cx.workspace.dependency("@faker-js/faker", "^9.0.0")?,
        Kind::Msw => cx.workspace.dependency("msw", "^2.0.0")?,
        Kind::Cypress => cx.workspace.dev_dependency("cypress", "^15.0.0")?,
    };
    if !matches!(kind, Kind::Cypress) {
        cx.workspace
            .export_namespace(output, &crate::sdk::lower_camel_identifier(output))?;
    }
    Ok(())
}
fn imports(client: &GraphqlClient, target: &std::path::Path) -> Result<String> {
    let mut source = String::new();
    for symbols in client.operations.values() {
        for symbol in [&symbols.variables, &symbols.result] {
            source.push_str(&format!(
                "import type {{ {} }} from {};\n",
                symbol.name,
                serde_json::to_string(&symbol.import_from(target)?)?
            ));
        }
    }
    Ok(source)
}
