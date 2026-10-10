use poolster_core::{
    engine::Packages,
    input::{InputOptions, InputProvider, InputRegistry},
    native::GraphqlOperations,
};
use poolster_plugin_typescript as ts;
use std::{process::Command, sync::Arc};
const SCHEMA: &str = "scalar DateTime input Filter {when:DateTime! list:[DateTime] next:Filter} type Stamp {when:DateTime! maybe:DateTime list:[DateTime]} type Query {stamp(input:Filter!):Stamp!} type Subscription {ticks(start:DateTime!):Stamp!}";
const OPERATIONS: &str = "query Read($input:Filter!,$include:Boolean!){stamp(input:$input){when maybe @include(if:$include) list}} subscription Ticks($start:DateTime!){ticks(start:$start){when maybe list}}";
fn generate(path: &std::path::Path) -> poolster_core::GeneratedTree {
    std::fs::write(path.join("schema.graphql"), SCHEMA).unwrap();
    std::fs::write(path.join("operations.graphql"), OPERATIONS).unwrap();
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_graphql::GraphqlInput)
        .unwrap();
    let input = InputProvider::<GraphqlOperations>::new(
        Arc::new(registry),
        "graphql",
        path.join("schema.graphql"),
    )
    .with_options(InputOptions {
        operation_files: vec![path.join("operations.graphql")],
        ..Default::default()
    });
    let output = ts::graphql(Some(input.handle()))
        .subscriptions()
        .flat()
        .scalar("DateTime", ts::GraphqlScalarMapping::new("Date", "Date"));
    Packages::new()
        .package(ts::package("sdk").with(output).with(input))
        .generate_native()
        .unwrap()
}
#[test]
fn subscription_helpers_and_typed_scalar_callbacks_regenerate() {
    let directory = tempfile::tempdir().unwrap();
    let first = generate(directory.path());
    let second = generate(directory.path());
    assert_eq!(first, second);
    assert!(
        first
            .get("sdk/graphql-sse.ts")
            .unwrap()
            .contains("createGraphqlSseTransport")
    );
    assert!(
        first
            .get("sdk/graphql-codecs.ts")
            .unwrap()
            .contains("encode?: (value: Date)")
    );
    assert!(
        first
            .get("sdk/graphql-codecs.ts")
            .unwrap()
            .contains("decode?: (value: unknown) => Date")
    );
}
#[test]
#[ignore = "requires pinned TypeScript/GraphQL/graphql-sse server and loopback sockets"]
fn compiled_scalar_codecs_and_sse_execute_against_official_server() {
    let compiler = std::env::var("POOLSTER_TSC_JS").expect("POOLSTER_TSC_JS");
    let root = std::env::var("POOLSTER_GRAPHQL_SSE_ROOT").expect("POOLSTER_GRAPHQL_SSE_ROOT");
    let directory = tempfile::tempdir().unwrap();
    generate(directory.path())
        .write_to(directory.path())
        .unwrap();
    let sdk = directory.path().join("sdk");
    std::fs::write(
        sdk.join("consumer.ts"),
        include_str!("fixtures/graphql_runtime_capabilities/consumer.ts"),
    )
    .unwrap();
    std::fs::write(
        sdk.join("runtime.mjs"),
        include_str!("fixtures/graphql_runtime_capabilities/runtime.mjs"),
    )
    .unwrap();
    for mut command in [
        {
            let mut command = Command::new("node");
            command.arg(compiler).args(["-p", "tsconfig.json"]);
            command
        },
        {
            let mut command = Command::new("node");
            command
                .arg("runtime.mjs")
                .env("POOLSTER_GRAPHQL_SSE_ROOT", root)
                .env("POOLSTER_SCHEMA", SCHEMA);
            command
        },
    ] {
        let result = command.current_dir(&sdk).output().unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
