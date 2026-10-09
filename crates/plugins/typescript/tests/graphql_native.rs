use poolster_core::{
    engine::Packages,
    input::{InputOptions, InputProvider, InputRegistry},
    native::GraphqlOperations,
};
use poolster_plugin_typescript as ts;
use std::{process::Command, sync::Arc};
const SCHEMA: &str = r#"type User { id: ID! name: String! fragile: String } type Query { user(id: ID!): User! fatal: String! node: Node! } type Mutation { rename(name: String!): User! } interface Node { id: ID! } type Named implements Node { id: ID! label: String! } type QueryExtra { unused: String } type Subscription { changed: User! }"#;
const OPERATIONS: &str = r#"query Read($id: ID!) { person: user(id: $id) { id name } } query Partial($id: ID!) { user(id: $id) { name fragile } } query Fatal { fatal } mutation Rename($name: String!) { rename(name: $name) { name } } query Abstract($include: Boolean!) { node { __typename id ...NamedFields @include(if: $include) } } fragment NamedFields on Named { label }"#;
fn generate(
    directory: &std::path::Path,
    operations: &str,
    subscriptions: bool,
) -> anyhow::Result<poolster_core::GeneratedTree> {
    generate_schema(directory, SCHEMA, operations, subscriptions)
}
fn generate_schema(
    directory: &std::path::Path,
    schema: &str,
    operations: &str,
    subscriptions: bool,
) -> anyhow::Result<poolster_core::GeneratedTree> {
    std::fs::write(directory.join("schema.graphql"), schema)?;
    std::fs::write(directory.join("operations.graphql"), operations)?;
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_graphql::GraphqlInput)?;
    let input = InputProvider::<GraphqlOperations>::new(
        Arc::new(registry),
        "graphql",
        directory.join("schema.graphql"),
    )
    .with_options(InputOptions {
        operation_files: vec![directory.join("operations.graphql")],
        ..Default::default()
    });
    let output = ts::graphql(Some(input.handle()));
    let output = if subscriptions {
        output.subscriptions()
    } else {
        output
    };
    Packages::new()
        .package(ts::package("sdk").with(output).with(input))
        .generate_native()
}
#[test]
fn native_graphql_output_preserves_selection_and_regenerates() {
    let dir = tempfile::tempdir().unwrap();
    let tree = generate(dir.path(), OPERATIONS, false).unwrap();
    let source = graphql_source(&tree);
    assert!(
        source.contains("\"person\": { \"id\": string; \"name\": string; }"),
        "{source}"
    );
    assert!(source.contains("\"fragile\": (string) | null"), "{source}");
    assert!(!source.contains("export type User"));
    let again = generate(dir.path(), OPERATIONS, false).unwrap();
    assert_eq!(source, graphql_source(&again));
    let error = generate(
        dir.path(),
        "subscription Changed { changed { name } }",
        false,
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("subscription"));
    assert!(
        generate(
            dir.path(),
            "subscription Changed { changed { name } }",
            true
        )
        .unwrap()
        .get("sdk/graphql/operations/Changed.ts")
        .unwrap()
        .contains("AsyncIterable")
    );
    assert!(generate(dir.path(), "query Invalid { missing }", false).is_err());
}
#[test]
#[ignore = "requires Node, POOLSTER_TSC_JS and POOLSTER_GRAPHQL_JS (graphql 16.14.2)"]
fn native_graphql_package_compiles_and_executes_against_local_server() {
    let compiler = std::env::var("POOLSTER_TSC_JS").expect("POOLSTER_TSC_JS");
    let graphql = std::env::var("POOLSTER_GRAPHQL_JS").expect("POOLSTER_GRAPHQL_JS");
    let dir = tempfile::tempdir().unwrap();
    generate(
        dir.path(),
        &format!("{OPERATIONS} subscription Changed {{ changed {{ name }} }}"),
        true,
    )
    .unwrap()
    .write_to(dir.path())
    .unwrap();
    std::fs::write(dir.path().join("sdk/consumer.ts"),r#"import { Read, Partial, Rename, Fatal, Abstract, createGraphqlHttpTransport } from './index.js';
const transport = createGraphqlHttpTransport('http://localhost');
async function check() {
 const result = await Read(transport, {id:'1'});
 if(result.kind !== 'error') { const name: string = result.data.person.name; console.log(name);
 // @ts-expect-error unselected field absent
 result.data.person.fragile;
 }
 // @ts-expect-error required variables
 await Read(transport, {});
 // @ts-expect-error mutation variables are typed
 await Rename(transport, {name:3});
 const partial = await Partial(transport,{id:'1'}); if(partial.kind === 'partial') console.log(partial.errors);
 const abstract = await Abstract(transport,{include:true}); if(abstract.kind !== 'error' && abstract.data.node.__typename === 'Named') { const label: string | undefined = abstract.data.node.label; console.log(label); }
 await Fatal(transport,{});
}
void check;
"#).unwrap();
    let output = Command::new("node")
        .arg(&compiler)
        .args(["-p", "tsconfig.json"])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    generate(dir.path(), "{ user(id: \"anonymous\") { name } }", false)
        .unwrap()
        .write_to(dir.path().join("anonymous"))
        .unwrap();
    let anonymous_compile = Command::new("node")
        .arg(&compiler)
        .args(["-p", "tsconfig.json"])
        .current_dir(dir.path().join("anonymous/sdk"))
        .output()
        .unwrap();
    assert!(
        anonymous_compile.status.success(),
        "{}{}",
        String::from_utf8_lossy(&anonymous_compile.stdout),
        String::from_utf8_lossy(&anonymous_compile.stderr)
    );
    std::fs::write(
        dir.path().join("sdk/probe.cjs"),
        include_str!("fixtures/graphql_probe.cjs"),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.cjs")
        .env("POOLSTER_GRAPHQL_JS", graphql)
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn output_accepts_substituted_contract_provider() {
    struct Alternate;
    impl poolster_core::input::InputPlugin for Alternate {
        fn id(&self) -> &str {
            "graphql.alternate"
        }
        fn format(&self) -> &str {
            "graphql"
        }
        fn load(&self, _: &std::path::Path) -> anyhow::Result<poolster_core::input::InputContract> {
            let doc = poolster_input_graphql::parse(SCHEMA)?;
            let operations =
                poolster_input_graphql::lower_operations(&doc.schema, SCHEMA, OPERATIONS)?;
            let mut input = poolster_core::input::InputContract::new(doc.summary());
            input.publish(operations)?;
            Ok(input)
        }
    }
    let mut registry = InputRegistry::new();
    registry.register(Alternate).unwrap();
    let input = InputProvider::<GraphqlOperations>::new(Arc::new(registry), "graphql", "not-used")
        .using("graphql.alternate");
    let tree = Packages::new()
        .package(
            ts::package("sdk")
                .with(ts::graphql(Some(input.handle())))
                .with(input),
        )
        .generate_native()
        .unwrap();
    assert!(graphql_source(&tree).contains("export function Read"));
}

#[test]
#[ignore = "requires Node and POOLSTER_TSC_JS"]
fn pinned_github_schema_generates_compilable_selection_package() {
    let compiler = std::env::var("POOLSTER_TSC_JS").expect("POOLSTER_TSC_JS");
    let dir = tempfile::tempdir().unwrap();
    let tree = generate_schema(
        dir.path(),
        include_str!("../../../inputs/graphql/tests/fixtures/github/schema.graphql"),
        "query Viewer { viewer { id login } }",
        false,
    )
    .unwrap();
    tree.write_to(dir.path()).unwrap();
    assert!(graphql_source(&tree).contains("export type ViewerResult"));
    let output = Command::new("node")
        .arg(compiler)
        .args(["-p", "tsconfig.json"])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn post_plugin_consumes_actual_generated_graphql_symbols() {
    use poolster_core::engine::{Enforce, Handle, Meta, Plugin, PluginContext, Requirement};
    struct Manifest {
        meta: Meta,
        source: Handle<ts::GraphqlClient>,
    }
    impl Plugin<ts::TypeScript> for Manifest {
        fn kind(&self) -> &'static str {
            "graphql-operation-manifest"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn enforce(&self) -> Enforce {
            Enforce::Post
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn requires(&self) -> Vec<Requirement> {
            vec![Requirement::on(Some(self.source))]
        }
        fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> anyhow::Result<()> {
            let client = cx.inputs.get::<ts::GraphqlClient>()?;
            let op = &client.operations["Read"];
            let import = op.function.import_from("hooks/read.ts")?;
            let source = format!(
                "import {{ {} }} from '{}.js';\nimport type {{ {}, {} }} from '{}.js';\nexport {{ {} }};\nexport type {{ {}, {} }};\n",
                op.function.name,
                import,
                op.variables.name,
                op.result.name,
                import,
                op.function.name,
                op.variables.name,
                op.result.name
            );
            assert_eq!(
                client.runtime_module,
                std::path::Path::new("graphql-runtime")
            );
            cx.files
                .emit(poolster_core::GeneratedFile::new("hooks/read.ts", source)?)
        }
    }
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("schema.graphql"), SCHEMA).unwrap();
    std::fs::write(directory.path().join("operations.graphql"), OPERATIONS).unwrap();
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_graphql::GraphqlInput)
        .unwrap();
    let input = InputProvider::<GraphqlOperations>::new(
        Arc::new(registry),
        "graphql",
        directory.path().join("schema.graphql"),
    )
    .with_options(InputOptions {
        operation_files: vec![directory.path().join("operations.graphql")],
        ..Default::default()
    });
    let generator = ts::graphql(Some(input.handle()));
    let post = Manifest {
        meta: Meta::new(),
        source: generator.handle(),
    };
    let tree = Packages::new()
        .package(ts::package("sdk").with(post).with(generator).with(input))
        .generate_native()
        .unwrap();
    let hook = tree.get("sdk/hooks/read.ts").unwrap();
    assert!(hook.contains("import { Read } from '../graphql.js'"));
    assert!(hook.contains("import type { ReadVariables, ReadResult } from '../graphql.js'"));
    assert!(graphql_source(&tree).contains("export function Read"));
}

#[test]
fn generated_aliases_reject_builtin_and_runtime_export_collisions() {
    let directory = tempfile::tempdir().unwrap();
    for name in [
        "Array",
        "Promise",
        "AsyncIterable",
        "string",
        "GraphqlError",
        "createGraphqlHttpTransport",
    ] {
        let schema = format!(
            "input {name} {{ value: String }} type Query {{ check(input: {name}): String }}"
        );
        let operation = format!("query Check($input: {name}) {{ check(input: $input) }}");
        let error = generate_schema(directory.path(), &schema, &operation, false).unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("symbol collision") || message.contains("TypeScript identifier"),
            "{message}"
        );
    }
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

#[test]
fn graphql_layout_is_bounded_and_stable_for_large_operation_sets() {
    let root = tempfile::tempdir().unwrap();
    let documents: Vec<_> = (0..1000)
        .map(|i| format!("query Read{i:04} {{ fatal }}"))
        .collect();
    let first = generate(root.path(), &documents.join("\n"), false).unwrap();
    let reverse = documents
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(first, generate(root.path(), &reverse, false).unwrap());
    assert!(first.get("sdk/graphql.ts").unwrap().len() < 1024);
    assert!(first.get("sdk/graphql-client.ts").unwrap().len() < 8192);
    for (path, source) in first
        .iter()
        .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "ts"))
    {
        assert!(
            source.len() < 128 * 1024,
            "oversized {}: {} bytes",
            path.display(),
            source.len()
        );
    }
    assert!(first.get("sdk/graphql/models/Read0000.ts").is_some());
    assert!(first.get("sdk/graphql/operations/Read0000.ts").is_some());
}

#[test]
fn graphql_layout_uses_only_referenced_input_imports_and_portable_filenames() {
    let root = tempfile::tempdir().unwrap();
    let inputs = (0..300)
        .map(|i| {
            if i == 299 {
                format!("input Input{i} {{ value: String }}")
            } else {
                format!("input Input{i} {{ next: Input{} }}", i + 1)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let schema = format!("{inputs} type Query {{ echo(input:Input0): String }}");
    let long_name = format!("Read{}", "Long".repeat(100));
    let tree = generate_schema(
        root.path(),
        &schema,
        &format!("query {long_name}($input:Input0) {{ echo(input:$input) }}"),
        false,
    )
    .unwrap();
    tree.write_to(root.path()).unwrap();
    let input = tree.get("sdk/graphql/models/Input0.ts").unwrap();
    assert_eq!(input.matches("import type").count(), 1);
    assert!(input.contains("Input1"));
    for (path, source) in tree
        .iter()
        .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "ts"))
    {
        assert!(path.file_name().unwrap().len() < 128, "{}", path.display());
        assert!(source.len() < 128 * 1024);
    }
}
