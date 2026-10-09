use poolster_core::{
    engine::Packages,
    input::{InputOptions, InputProvider, InputRegistry},
    native::GraphqlOperations,
};
use poolster_plugin_typescript as ts;
use std::{collections::BTreeMap, path::Path, process::Command, sync::Arc};
const SCHEMA: &str = r#"scalar DateTime scalar JSON scalar Opaque input StampInput {at:DateTime! maybe:DateTime history:[DateTime!]! payload:JSON} type StampValue {at:DateTime! maybe:DateTime history:[DateTime!]! payload:JSON opaque:Opaque} type Query {noop:Boolean!} type Mutation {stamp(at:DateTime!,maybe:DateTime,input:StampInput!):StampValue!}"#;
const OPERATIONS: &str = r#"mutation Stamp($at:DateTime!,$maybe:DateTime,$input:StampInput!,$include:Boolean!){stamp(at:$at,maybe:$maybe,input:$input){at maybe @include(if:$include) history payload opaque}}"#;
fn generate(
    path: &Path,
    mappings: BTreeMap<String, ts::GraphqlScalarMapping>,
) -> anyhow::Result<poolster_core::GeneratedTree> {
    std::fs::write(path.join("schema.graphql"), SCHEMA)?;
    std::fs::write(path.join("operations.graphql"), OPERATIONS)?;
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_graphql::GraphqlInput)?;
    let input = InputProvider::<GraphqlOperations>::new(
        Arc::new(registry),
        "graphql",
        path.join("schema.graphql"),
    )
    .with_options(InputOptions {
        operation_files: vec![path.join("operations.graphql")],
        ..Default::default()
    });
    let output = ts::graphql(Some(input.handle())).scalars(mappings);
    Packages::new()
        .package(ts::package("sdk").with(output).with(input))
        .generate_native()
}
fn mappings() -> BTreeMap<String, ts::GraphqlScalarMapping> {
    [
        (
            "DateTime".into(),
            ts::GraphqlScalarMapping::new("string", "number"),
        ),
        (
            "JSON".into(),
            ts::GraphqlScalarMapping::new("unknown", "{ readonly marker: string }"),
        ),
    ]
    .into()
}
#[test]
fn custom_scalars_have_independent_directions_preserving_wrappers_and_defaults() {
    let path = tempfile::tempdir().unwrap();
    let tree = generate(path.path(), mappings()).unwrap();
    let source = graphql_source(&tree);
    assert!(source.contains("\"at\": (string)"), "{source}");
    assert!(source.contains("\"at\": (number)"), "{source}");
    assert!(source.contains("\"maybe\"?: ((number)) | null"), "{source}");
    assert!(source.contains("Array<(string)>"), "{source}");
    assert!(source.contains("Array<(number)>"), "{source}");
    assert!(source.contains("\"opaque\": (unknown) | null"), "{source}");
    assert_eq!(tree, generate(path.path(), mappings()).unwrap());
    let default = generate(path.path(), BTreeMap::new()).unwrap();
    assert!(!graphql_source(&default).contains("(number)"));
    for mapping in [
        ts::GraphqlScalarMapping::new("string; export const injected=1", "number"),
        ts::GraphqlScalarMapping::new("", "number"),
    ] {
        assert!(generate(path.path(), [("DateTime".into(), mapping)].into()).is_err());
    }
    assert!(
        generate(
            path.path(),
            [(
                "Int".into(),
                ts::GraphqlScalarMapping::new("string", "number")
            )]
            .into()
        )
        .is_err()
    );
}
#[test]
#[ignore = "requires Node, POOLSTER_TSC_JS and POOLSTER_GRAPHQL_JS (pinned GraphQL server)"]
fn mapped_scalars_compile_and_execute_against_real_custom_scalar_server() {
    let compiler = std::env::var("POOLSTER_TSC_JS").expect("POOLSTER_TSC_JS");
    let graphql = std::env::var("POOLSTER_GRAPHQL_JS").expect("POOLSTER_GRAPHQL_JS");
    let path = tempfile::tempdir().unwrap();
    generate(path.path(), mappings())
        .unwrap()
        .write_to(path.path())
        .unwrap();
    let sdk = path.path().join("sdk");
    std::fs::write(
        sdk.join("consumer.ts"),
        include_str!("fixtures/graphql_scalars/consumer.ts"),
    )
    .unwrap();
    std::fs::write(
        sdk.join("runtime.mjs"),
        include_str!("fixtures/graphql_scalars/runtime.mjs"),
    )
    .unwrap();
    for mut command in [
        {
            let mut c = Command::new("node");
            c.arg(compiler).args(["-p", "tsconfig.json"]);
            c
        },
        {
            let mut c = Command::new("node");
            c.arg("runtime.mjs")
                .env("POOLSTER_GRAPHQL_JS", graphql)
                .env("POOLSTER_SCALAR_SCHEMA", SCHEMA);
            c
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
    generate(path.path(), mappings())
        .unwrap()
        .write_to(path.path())
        .unwrap();
}

fn graphql_source(tree: &poolster_core::GeneratedTree) -> String {
    tree.iter()
        .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "ts"))
        .map(|(_, content)| content)
        .collect::<Vec<_>>()
        .join("\n")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
