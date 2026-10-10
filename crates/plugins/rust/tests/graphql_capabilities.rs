use anyhow::{Result, ensure};
use poolster_core::{
    engine::Packages,
    input::{InputOptions, InputProvider, InputRegistry},
    native::{GraphqlIncrementalOperations, GraphqlOperations},
};
use poolster_plugin_rust::{self as rust, PackageExt};
use std::{path::Path, process::Command, sync::Arc};
const SCHEMA: &str = "directive @defer(if:Boolean! = true,label:String) on FRAGMENT_SPREAD | INLINE_FRAGMENT directive @stream(if:Boolean! = true,label:String,initialCount:Int! = 0) on FIELD scalar DateTime input Filter {when:DateTime! list:[DateTime] next:Filter} type Stamp {when:DateTime! maybe:DateTime list:[DateTime] name:String!} type User{id:ID! when:DateTime!} type Query{stamp(input:Filter!):Stamp! user:User! values:[Int!]!} type Subscription{ticks(start:DateTime!):Stamp!}";
const REGULAR: &str = "query Read($input:Filter!,$include:Boolean!){stamp(input:$input){when maybe @include(if:$include) list name}} subscription Ticks($start:DateTime!){ticks(start:$start){when maybe list name}}";
const INCREMENTAL: &str = "query Feed($count:Int!){user{id ... @defer(label:\"details\"){when}} values @stream(label:\"values\",initialCount:$count)}";
fn generate(root: &Path) -> Result<poolster_core::GeneratedTree> {
    std::fs::write(root.join("schema.graphql"), SCHEMA)?;
    std::fs::write(root.join("regular.graphql"), REGULAR)?;
    std::fs::write(root.join("incremental.graphql"), INCREMENTAL)?;
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_graphql::GraphqlInput)?;
    let registry = Arc::new(registry);
    let input = InputProvider::<GraphqlOperations>::new(
        registry.clone(),
        "graphql",
        root.join("schema.graphql"),
    )
    .with_options(InputOptions {
        operation_files: vec![root.join("regular.graphql")],
        ..Default::default()
    });
    let incremental = InputProvider::<GraphqlIncrementalOperations>::new(
        registry,
        "graphql",
        root.join("schema.graphql"),
    )
    .with_options(InputOptions {
        operation_files: vec![root.join("incremental.graphql")],
        graphql_incremental: true,
        ..Default::default()
    });
    let sdk = rust::graphql(Some(input.handle()))
        .subscriptions()
        .flat()
        .scalar("DateTime", rust::GraphqlScalarMapping::new("String", "i64"));
    let stream = rust::graphql_incremental(Some(incremental.handle()))
        .flat()
        .scalar("DateTime", rust::GraphqlScalarMapping::new("String", "i64"));
    Packages::new()
        .package(
            rust::package("regular")
                .name("regular-client")
                .with(sdk)
                .with(input),
        )
        .package(
            rust::package("incremental")
                .name("incremental-client")
                .with(stream)
                .with(incremental),
        )
        .generate_native()
}
#[test]
fn distinct_stream_contracts_and_codecs_regenerate() -> Result<()> {
    let root = tempfile::tempdir()?;
    let first = generate(root.path())?;
    ensure!(first == generate(root.path())?);
    first.write_to(root.path())?;
    let source =
        std::fs::read_to_string(root.path().join("regular/src/graphql/operations/ticks.rs"))?;
    ensure!(source.contains("GraphqlSubscription<TicksResult>"));
    let source = std::fs::read_to_string(
        root.path()
            .join("incremental/src/graphql/operations/feed.rs"),
    )?;
    ensure!(source.contains("GraphqlIncrementalStream<FeedResult>"));
    Ok(())
}
#[test]
#[ignore = "requires cached Cargo dependencies, official pinned GraphQL.js16.14.2/graphql-sse2.6.0 and local sockets"]
fn generated_streams_codecs_compile_and_run() -> Result<()> {
    use std::io::{BufRead, BufReader};
    let root = tempfile::tempdir()?;
    generate(root.path())?.write_to(root.path())?;
    let mut server = Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/graphql_capabilities/server.cjs"
        ))
        .arg(root.path().join("schema.graphql"))
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
    BufReader::new(server.stdout.take().unwrap()).read_line(&mut endpoint)?;
    let _server = Server(server);
    let consumer = root.path().join("consumer");
    std::fs::create_dir_all(consumer.join("src"))?;
    std::fs::write(
        consumer.join("Cargo.toml"),
        "[workspace]\n[package]\nname='capability-consumer'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nregular={package='regular-client',path='../regular'}\nincremental={package='incremental-client',path='../incremental'}\ntokio={version='1',features=['macros','rt-multi-thread','time']}\nserde_json='=1.0.151'\n",
    )?;
    std::fs::write(
        consumer.join("src/main.rs"),
        include_str!("fixtures/graphql_capabilities/consumer.rs"),
    )?;
    let result = Command::new("cargo")
        .args(["run", "--offline", "--quiet"])
        .current_dir(consumer)
        .env("GRAPHQL_ENDPOINT", endpoint.trim())
        .env("CARGO_TARGET_DIR", root.path().join("target"))
        .output()?;
    ensure!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}
