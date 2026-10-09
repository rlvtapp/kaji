use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedTree,
    customization::CodeCustomization,
    engine::Packages,
    input::{InputOptions, InputProvider, InputRegistry},
    native::GraphqlOperations,
};
use poolster_plugin_typescript::{self as ts, PackageExt};
use std::{
    path::Path,
    process::{Command, Output},
    sync::Arc,
};
const SCHEMA: &str = "input Filter { prefix: String! limit: Int = 3 } type User { id: ID! name: String! nickname: String fragile: String } type Query { user(id: ID!, filter: Filter): User! fatal: String! } type Mutation { rename(name: String!): User! }";
const OPERATIONS: &str = "query Read($id: ID!, $filter: Filter) { person: user(id: $id, filter: $filter) { id name nickname } } query Partial($id: ID!) { user(id: $id) { name fragile } } query Fatal { fatal } mutation Rename($name: String!) { rename(name: $name) { name } }";
fn generate(root: &Path, customized: bool) -> Result<GeneratedTree> {
    std::fs::write(root.join("schema.graphql"), SCHEMA)?;
    std::fs::write(root.join("operations.graphql"), OPERATIONS)?;
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_graphql::GraphqlInput)?;
    let input = InputProvider::<GraphqlOperations>::new(
        Arc::new(registry),
        "graphql",
        root.join("schema.graphql"),
    )
    .with_options(InputOptions {
        operation_files: vec![root.join("operations.graphql")],
        ..Default::default()
    });
    let generator = ts::graphql(Some(input.handle()));
    let mut package = ts::package("sdk")
        .name("@poolster-test/graphql-client")
        .with(generator)
        .with(input);
    if customized {
        package = package
            .customize(CodeCustomization::Add {
                path: "extension.ts".into(),
                contents: "export const releaseMarker = 'customized-release';\n".into(),
            })
            .customize(CodeCustomization::Patch {
                path: "index.ts".into(),
                find: "export * from \"./graphql.js\";".into(),
                replacement: "export * from \"./graphql.js\";\nexport * from './extension.js';"
                    .into(),
            });
    }
    Packages::new().package(package).generate_native()
}
fn successful(command: &mut Command) -> Result<Output> {
    let output = command.output()?;
    ensure!(
        output.status.success(),
        "{command:?}\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}
#[test]
fn graphql_release_customization_regeneration_preserves_owned_source() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let first = generate(dir.path(), true)?;
    first.write_to(dir.path())?;
    let second = generate(dir.path(), true)?;
    second.write_to(dir.path())?;
    ensure!(
        first.iter().collect::<Vec<_>>() == second.iter().collect::<Vec<_>>(),
        "regeneration changed customized sources"
    );
    ensure!(
        first.get("sdk/index.ts").unwrap().contains("extension.js"),
        "customized export missing"
    );
    ensure!(
        std::fs::read_to_string(dir.path().join("sdk/extension.ts"))?
            .contains("customized-release"),
        "customized extension missing"
    );
    Ok(())
}
#[test]
#[ignore = "requires Node/npm, POOLSTER_TSC_JS5.9.3 and POOLSTER_GRAPHQL_JS16.14.2; executes local HTTP server"]
fn packed_graphql_client_installs_compiles_and_executes_in_clean_consumer() -> Result<()> {
    let compiler = std::env::var("POOLSTER_TSC_JS")?;
    let graphql = std::env::var("POOLSTER_GRAPHQL_JS")?;
    let dir = tempfile::tempdir()?;
    let first = generate(dir.path(), true)?;
    first.write_to(dir.path())?;
    let second = generate(dir.path(), true)?;
    ensure!(
        first.iter().collect::<Vec<_>>() == second.iter().collect::<Vec<_>>(),
        "customized regeneration changed output"
    );
    second.write_to(dir.path())?;
    let sdk = dir.path().join("sdk");
    successful(
        Command::new("node")
            .args([&compiler, "-p", "tsconfig.json"])
            .current_dir(&sdk),
    )?;
    let packed = successful(
        Command::new("npm")
            .args(["pack", "--ignore-scripts", "--json"])
            .current_dir(&sdk),
    )?;
    let metadata: serde_json::Value = serde_json::from_slice(&packed.stdout)?;
    let tarball = sdk.join(metadata[0]["filename"].as_str().unwrap());
    let package_files = metadata[0]["files"].as_array().unwrap();
    ensure!(
        package_files
            .iter()
            .any(|file| file["path"] == "dist/index.js"),
        "npm archive missing executable entrypoint"
    );
    ensure!(
        package_files
            .iter()
            .any(|file| file["path"] == "dist/index.d.ts"),
        "npm archive missing type entrypoint"
    );
    ensure!(
        package_files.iter().all(|file| {
            let path = file["path"].as_str().unwrap();
            !path.starts_with(".poolster/") && !path.ends_with(".graphql")
        }),
        "npm archive includes bookkeeping or input documents"
    );
    let consumer = dir.path().join("consumer");
    std::fs::create_dir(&consumer)?;
    std::fs::write(
        consumer.join("package.json"),
        "{\"name\":\"clean-graphql-consumer\",\"private\":true,\"type\":\"module\"}\n",
    )?;
    successful(
        Command::new("npm")
            .args([
                "install",
                "--offline",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
                "--package-lock=false",
            ])
            .arg(&tarball)
            .current_dir(&consumer),
    )?;
    ensure!(
        !consumer.join("node_modules/typescript").exists(),
        "consumer unexpectedly installed package development dependencies"
    );
    std::fs::write(
        consumer.join("consumer.ts"),
        include_str!("fixtures/graphql_release_consumer.ts"),
    )?;
    std::fs::write(
        consumer.join("tsconfig.json"),
        "{\"compilerOptions\":{\"strict\":true,\"noEmit\":true,\"target\":\"ES2022\",\"module\":\"NodeNext\",\"moduleResolution\":\"NodeNext\",\"lib\":[\"ES2022\",\"DOM\"]},\"include\":[\"consumer.ts\"]}\n",
    )?;
    successful(
        Command::new("node")
            .args([&compiler, "-p", "tsconfig.json"])
            .current_dir(&consumer),
    )?;
    std::fs::write(
        consumer.join("probe.mjs"),
        include_str!("fixtures/graphql_release_probe.mjs"),
    )?;
    let runtime = successful(
        Command::new("node")
            .arg("probe.mjs")
            .env("POOLSTER_GRAPHQL_JS", graphql)
            .env("POOLSTER_RELEASE_SCHEMA", SCHEMA)
            .current_dir(&consumer),
    )?;
    println!("{}", String::from_utf8_lossy(&runtime.stdout));
    println!(
        "npm archive: {} files; clean consumer uses installed package only",
        package_files.len()
    );
    Ok(())
}
