use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedTree,
    engine::Packages,
    input::{InputOptions, InputProvider, InputRegistry},
    native::GraphqlOperations,
};
use poolster_plugin_rust::{self as rust, PackageExt};
use std::{
    path::Path,
    process::{Command, Output},
    sync::Arc,
};
const SCHEMA: &str = "input Options { note: String = \"input-default\" } type User { id: ID! name: String! nickname: String fragile: String } interface Node { id:ID! } type Person implements Node { id:ID! name:String! } type Robot implements Node { id:ID! code:String! } type Query { node:Node! user(id: ID!): User! fatal: String! echo(input:String = \"argument-default\"):String inputEcho(options:Options):String } type Mutation { rename(name:String!):User! }";
const OPS: &str = "query AbstractNode { node { id ... on Person { name } ... on Robot { code } } } query Read($id:ID!){person:user(id:$id){id name nickname}} query Partial($id:ID!){user(id:$id){name fragile}} query Fatal{fatal} mutation Rename($name:String!){rename(name:$name){name}} query PresenceQuery($value:String){value:echo(input:$value)} query InputPresence($options:Options){value:inputEcho(options:$options)} query Conditional($include:Boolean!){user(id:\"7\"){name @include(if:$include) nickname @include(if:$include)}} query Defaulted($include:Boolean! = false){user(id:\"7\"){name @include(if:$include) nickname @include(if:$include)}}";
fn generate(root: &Path) -> Result<GeneratedTree> {
    std::fs::write(root.join("schema.graphql"), SCHEMA)?;
    std::fs::write(root.join("operations.graphql"), OPS)?;
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
    let generator = rust::graphql(Some(input.handle()));
    Packages::new()
        .package(
            rust::package("sdk")
                .name("poolster-graphql-test-sdk")
                .with(generator)
                .with(input),
        )
        .generate_native()
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
fn graphql_regeneration_is_stable() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let first = generate(dir.path())?;
    first.write_to(dir.path())?;
    let second = generate(dir.path())?;
    ensure!(first.iter().collect::<Vec<_>>() == second.iter().collect::<Vec<_>>());
    second.write_to(dir.path())?;
    Ok(())
}
#[test]
#[ignore = "requires cached Cargo dependencies and POOLSTER_GRAPHQL_JS16.14.2; executes local HTTP server"]
fn packaged_graphql_client_compiles_and_executes() -> Result<()> {
    use std::io::{BufRead, BufReader};
    let dir = tempfile::tempdir()?;
    generate(dir.path())?.write_to(dir.path())?;
    let mut child = Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/graphql_native/server.cjs"
        ))
        .arg(dir.path().join("schema.graphql"))
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    struct Server(std::process::Child);
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut endpoint = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut endpoint)?;
    let _server = Server(child);
    successful(
        Command::new("cargo")
            .args(["package", "--allow-dirty", "--no-verify", "--offline"])
            .current_dir(dir.path().join("sdk"))
            .env("CARGO_TARGET_DIR", dir.path().join("sdk/target")),
    )?;
    let archive = dir
        .path()
        .join("sdk/target/package/poolster-graphql-test-sdk-0.1.0.crate");
    let archives = std::fs::read_dir(dir.path().join("sdk/target/package"))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| path.extension().is_some_and(|ext| ext == "crate"))
        .unwrap_or(archive);
    std::fs::create_dir(dir.path().join("installed"))?;
    successful(
        Command::new("tar")
            .arg("xf")
            .arg(archives)
            .arg("-C")
            .arg(dir.path().join("installed")),
    )?;
    let installed = std::fs::read_dir(dir.path().join("installed"))?
        .next()
        .unwrap()?
        .path();
    let consumer = dir.path().join("consumer");
    std::fs::create_dir_all(consumer.join("src"))?;
    std::fs::write(
        consumer.join("Cargo.toml"),
        format!(
            "[workspace]\n[package]\nname='graphql-consumer'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nclient={{package='poolster-graphql-test-sdk',path={:?}}}\ntokio={{version='1',features=['macros','rt-multi-thread']}}\nserde_json='=1.0.151'\n",
            installed.to_string_lossy()
        ),
    )?;
    std::fs::write(
        consumer.join("src/main.rs"),
        include_str!("fixtures/graphql_native/consumer.rs"),
    )?;
    successful(
        Command::new("cargo")
            .args(["run", "--offline", "--quiet"])
            .current_dir(&consumer)
            .env("GRAPHQL_ENDPOINT", endpoint.trim())
            .env("CARGO_TARGET_DIR", dir.path().join("consumer-target")),
    )?;
    std::fs::create_dir(consumer.join("src/bin"))?;
    std::fs::write(
        consumer.join("src/bin/invalid.rs"),
        "use client::*; fn main() { let _ = ReadVariables { id: 7 }; } fn invalid(result: ReadResult) { let _ = result.person.fragile; }",
    )?;
    let invalid = Command::new("cargo")
        .args(["check", "--offline", "--bin", "invalid"])
        .current_dir(&consumer)
        .env("CARGO_TARGET_DIR", dir.path().join("consumer-target"))
        .output()?;
    ensure!(
        !invalid.status.success(),
        "invalid selections and variables compiled"
    );
    let errors = String::from_utf8_lossy(&invalid.stderr);
    ensure!(
        errors.contains("E0308") && errors.contains("E0609"),
        "unexpected compile failure: {errors}"
    );
    Ok(())
}
