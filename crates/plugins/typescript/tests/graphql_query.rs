use poolster_core::{
    engine::Packages,
    input::{InputOptions, InputProvider, InputRegistry},
    native::GraphqlOperations,
};
use poolster_plugin_typescript as ts;
use std::{path::Path, process::Command, sync::Arc};
const SCHEMA: &str =
    "type Query { read(id:ID!):String! } type Mutation { rename(name:String!):String! }";
const OPS: &str =
    "query Read($id:ID!){read(id:$id)} mutation Rename($name:String!){rename(name:$name)}";
fn generate(root: &Path, style: &str) -> anyhow::Result<poolster_core::GeneratedTree> {
    generate_with(root, style, ts::composition::react_query())
}
fn generate_with(
    root: &Path,
    style: &str,
    query: ts::composition::Query,
) -> anyhow::Result<poolster_core::GeneratedTree> {
    std::fs::write(root.join("schema.graphql"), SCHEMA)?;
    std::fs::write(root.join("ops.graphql"), OPS)?;
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_graphql::GraphqlInput)?;
    let input = InputProvider::<GraphqlOperations>::new(
        Arc::new(registry),
        "graphql",
        root.join("schema.graphql"),
    )
    .with_options(InputOptions {
        operation_files: vec![root.join("ops.graphql")],
        ..Default::default()
    });
    let sdk = ts::graphql(Some(input.handle()));
    let sdk = match style {
        "raw" => sdk.raw(),
        "flat" => sdk.flat(),
        _ => sdk.group("user", "read", "Read"),
    };
    let handle = sdk.handle();
    Packages::new()
        .package(
            ts::package("sdk")
                .with(query.using_graphql(Some(handle)))
                .with(ts::graphql_vue_query(Some(handle)))
                .with(ts::graphql_swr(Some(handle)))
                .with(sdk)
                .with(input),
        )
        .generate_native()
}
#[test]
fn graph_helpers_consume_selected_client_for_all_styles_and_regenerate() {
    let root = tempfile::tempdir().unwrap();
    for style in ["raw", "flat", "idiomatic"] {
        let tree = generate(root.path(), style).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_str(tree.get("sdk/package.json").unwrap()).unwrap();
        assert_eq!(manifest["devDependencies"]["@types/react"], "19.3.0");
        assert_eq!(tree, generate(root.path(), style).unwrap());
        for module in ["react-query", "vue-query", "swr"] {
            let source = tree.get(format!("sdk/{module}.ts")).unwrap();
            assert!(source.contains("GraphqlResult"));
            assert!(source.contains("cacheScope"));
            assert!(source.contains("read as readOperation"));
        }
        assert!(
            tree.get("sdk/react-query.ts")
                .unwrap()
                .contains("retry: false")
        );
    }
}
#[test]
fn unsupported_native_query_options_fail_before_generation() {
    let root = tempfile::tempdir().unwrap();
    for query in [
        ts::composition::react_query().operation_kind("Read", ts::composition::QueryKind::Mutation),
        ts::composition::react_query().include_operations(["Missing"]),
        ts::composition::react_query().max_operations_per_file(1),
        ts::composition::react_query().layout(poolster_core::SourceLayout::PerOperation),
        ts::composition::react_query()
            .operation_name("Read", "same")
            .operation_name("Rename", "same"),
    ] {
        assert!(generate_with(root.path(), "idiomatic", query).is_err());
    }
}

#[test]
#[ignore = "requires pinned node_modules, TS compiler, renderer, and local GraphQL server"]
fn generated_framework_consumers_compile_and_execute() {
    let modules =
        std::env::var("POOLSTER_GRAPHQL_QUERY_MODULES").expect("POOLSTER_GRAPHQL_QUERY_MODULES");
    let compiler = std::env::var("POOLSTER_TSC_JS").expect("POOLSTER_TSC_JS");
    std::env::var("POOLSTER_REACT_TEST_RENDERER")
        .expect("POOLSTER_REACT_TEST_RENDERER (mounted React/SWR validation)");
    for style in ["raw", "flat", "idiomatic"] {
        let root = tempfile::tempdir().unwrap();
        generate(root.path(), style)
            .unwrap()
            .write_to(root.path())
            .unwrap();
        let sdk = root.path().join("sdk");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&modules, sdk.join("node_modules")).unwrap();
        std::fs::write(
            sdk.join("consumer.ts"),
            include_str!("fixtures/graphql_query/consumer.ts"),
        )
        .unwrap();
        std::fs::write(
            sdk.join("runtime.mjs"),
            include_str!("fixtures/graphql_query/runtime.mjs"),
        )
        .unwrap();
        for (executable, args) in [
            ("node", vec![compiler.as_str(), "-p", "tsconfig.json"]),
            ("node", vec!["runtime.mjs"]),
        ] {
            let result = Command::new(executable)
                .args(args)
                .env(
                    "POOLSTER_GRAPHQL_JS",
                    Path::new(&modules).join("graphql/index.js"),
                )
                .current_dir(&sdk)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}
