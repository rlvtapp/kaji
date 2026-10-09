use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedTree,
    engine::Packages,
    input::{InputOptions, InputProvider, InputRegistry},
    native::GraphqlOperations,
};
use poolster_plugin_typescript::{self as ts, PackageExt};
use std::{path::Path, process::Command, sync::Arc};
const SCHEMA: &str = "scalar Timestamp scalar Json enum Role { USER ADMIN } input DefaultInput { role:Role! = USER } interface Node { id:ID! } type Person implements Node { id:ID! name:String! } type Robot implements Node { id:ID! code:String! } input Nested { required:ID! optional:String child:Nested values:[Int!]! } type User { id:ID! name:String! nickname:String fragile:String } type Query { node:Node! role:Role! extra:Json! defaulted(input:DefaultInput!):Role! user(id:ID!,nested:Nested):User! timestamp(value:Timestamp!):Timestamp! } type Mutation { rename(name:String!):User! }";
const OPS: &str = r#"query Models { role extra node { __typename id ... on Person { name } ... on Robot { code } } } query DefaultedInput($input:DefaultInput!){defaulted(input:$input)} query Conditional($include:Boolean!){user(id:"n"){name @include(if:$include) nickname @include(if:$include)}} query NestedInput($nested:Nested!){user(id:"n",nested:$nested){id name nickname}} query ReadUser($id:ID!,$nested:Nested){user(id:$id,nested:$nested){id name nickname}} query Partial($id:ID!){user(id:$id){id fragile}} query Timestamp($value:Timestamp!){timestamp(value:$value)} mutation Rename($name:String!){rename(name:$name){id name nickname}}"#;
fn generate(root: &Path) -> Result<GeneratedTree> {
    generate_config(
        root,
        ts::FixtureOptions {
            seed: Some(7),
            ..Default::default()
        },
        ts::CypressOptions {
            include_mutations: true,
            ..Default::default()
        },
    )
}
fn generate_config(
    root: &Path,
    fixtures: ts::FixtureOptions,
    cypress: ts::CypressOptions,
) -> Result<GeneratedTree> {
    std::fs::write(root.join("schema.graphql"), SCHEMA)?;
    std::fs::write(root.join("operations.graphql"), OPS)?;
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_graphql::GraphqlInput)?;
    let provider = InputProvider::<GraphqlOperations>::new(
        Arc::new(registry),
        "graphql",
        root.join("schema.graphql"),
    )
    .with_options(InputOptions {
        operation_files: vec![root.join("operations.graphql")],
        ..Default::default()
    });
    let client = ts::graphql(Some(provider.handle())).flat().scalar(
        "Timestamp",
        ts::GraphqlScalarMapping::new("string", "number"),
    );
    let handle = client.handle();
    let mut package = ts::package("sdk").name("graphql-helpers-test");
    // Reverse registration proves dependencies, rather than insertion order, govern helpers.
    package = package
        .with(
            ts::cypress()
                .using_graphql(Some(handle))
                .cypress_options(cypress),
        )
        .with(ts::msw().using_graphql(Some(handle)))
        .with(
            ts::faker()
                .using_graphql(Some(handle))
                .fixture_options(fixtures),
        )
        .with(ts::zod().using_graphql(Some(handle)))
        .with(client)
        .with(provider);
    Packages::new().package(package).generate_native()
}
#[test]
fn helpers_regenerate_and_consume_actual_selected_client() -> Result<()> {
    let root = tempfile::tempdir()?;
    let first = generate(root.path())?;
    let second = generate(root.path())?;
    ensure!(first.iter().collect::<Vec<_>>() == second.iter().collect::<Vec<_>>());
    ensure!(
        first
            .get("sdk/msw.ts")
            .unwrap()
            .contains("graphql.query(\"ReadUser\"")
    );
    ensure!(
        first
            .get("sdk/cypress.ts")
            .unwrap()
            .contains("request.alias=\"ReadUser\"")
    );
    Ok(())
}
#[test]
#[ignore = "requires pinned Node fixture modules, TypeScript compiler; actual MSW/local server execution"]
fn generated_helpers_compile_and_execute() -> Result<()> {
    let modules = std::env::var("POOLSTER_GRAPHQL_HELPER_NODE_MODULES")?;
    let compiler = std::env::var("POOLSTER_TSC_JS")?;
    let graphql = std::env::var("POOLSTER_GRAPHQL_JS")?;
    let root = tempfile::tempdir()?;
    generate(root.path())?.write_to(root.path())?;
    let sdk = root.path().join("sdk");
    #[cfg(unix)]
    std::os::unix::fs::symlink(modules, sdk.join("node_modules"))?;
    let output = Command::new("node")
        .args([&compiler, "-p", "tsconfig.json"])
        .current_dir(&sdk)
        .output()?;
    ensure!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(
        sdk.join("probe.mjs"),
        include_str!("fixtures/graphql_helpers/probe.mjs"),
    )?;
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&sdk)
        .env("POOLSTER_GRAPHQL_JS", graphql)
        .env("POOLSTER_GRAPHQL_SCHEMA", SCHEMA)
        .output()?;
    ensure!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
    if let Ok(destination) = std::env::var("POOLSTER_GRAPHQL_HELPER_FIXTURE_OUTPUT") {
        let destination = Path::new(&destination);
        std::fs::create_dir_all(destination)?;
        generate(root.path())?.write_to(destination)?;
        std::fs::write(destination.join("schema.graphql"), SCHEMA)?;
        std::fs::write(destination.join("operations.graphql"), OPS)?;
    }
    Ok(())
}

#[test]
fn unsupported_fixture_controls_and_depth_are_diagnostics() -> Result<()> {
    for fixtures in [
        ts::FixtureOptions {
            max_depth: 0,
            ..Default::default()
        },
        ts::FixtureOptions {
            max_attempts: 1,
            ..Default::default()
        },
        ts::FixtureOptions {
            overrides: std::collections::BTreeMap::from([(
                "UnknownDomain".into(),
                serde_json::json!(1),
            )]),
            ..Default::default()
        },
    ] {
        let root = tempfile::tempdir()?;
        ensure!(generate_config(root.path(), fixtures, ts::CypressOptions::default()).is_err());
    }
    let root = tempfile::tempdir()?;
    let error = generate_config(
        root.path(),
        ts::FixtureOptions::default(),
        ts::CypressOptions {
            operation_overrides: std::collections::BTreeMap::from([(
                "ReadUser".into(),
                ts::CypressOperationOptions::default(),
            )]),
            ..Default::default()
        },
    )
    .expect_err("HTTP overrides must be rejected");
    ensure!(format!("{error:#}").contains("operation_overrides"));
    Ok(())
}
