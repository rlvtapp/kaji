use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedTree,
    blocks::{Blocks, CollectionState, ContractReference},
    engine::*,
    input::{InputOptions, InputPlugin, InputProvider, InputRegistry},
    native::{GraphqlOperation, GraphqlOperations},
};
use poolster_plugin_rust_cli::{self as output, PackageExt};
use std::{
    path::Path,
    process::{Command, Output},
    sync::Arc,
};
const SCHEMA: &str = "type User {id:ID! name:String! fragile:String} type Query {user(id:ID!):User! fatal:String! presence(value:String):String!} type Mutation{rename(name:String!):User!} type Subscription{changed:User!}";
const OPERATIONS: &str = "query ReadUser($id:ID!){user(id:$id){id name}} mutation Rename($name:String!){rename(name:$name){name}} query Partial($id:ID!){user(id:$id){name fragile}} query Fatal{fatal} query Presence($value:String){presence(value:$value)}";
fn generate(root: &Path, operations: &str) -> Result<GeneratedTree> {
    std::fs::write(root.join("schema.graphql"), SCHEMA)?;
    std::fs::write(root.join("operations.graphql"), operations)?;
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
    let cli = output::graphql()
        .input(input.handle())
        .command_name("users");
    Packages::new()
        .package(
            output::package("cli")
                .name("graphql-cli-test")
                .with(input)
                .with(cli),
        )
        .generate_native()
}
#[test]
fn native_graphql_cli_is_stable_modular_and_rejects_unsupported_inputs() -> Result<()> {
    let root = tempfile::tempdir()?;
    let first = generate(root.path(), OPERATIONS)?;
    first.write_to(root.path())?;
    let second = generate(root.path(), OPERATIONS)?;
    ensure!(first.iter().collect::<Vec<_>>() == second.iter().collect::<Vec<_>>());
    ensure!(
        root.path()
            .join("cli/src/operations/operation-read-user.rs")
            .exists()
    );
    for (source, expected) in [
        ("", "required contract"),
        ("subscription Changed{changed{id}}", "subscriptions"),
        ("query ReadUser{fatal} query read_user{fatal}", "colliding"),
        ("query Bad{unknown}", "unknown"),
        ("query Help{fatal}", "reserved"),
    ] {
        let error = generate(root.path(), source).expect_err("invalid input accepted");
        ensure!(
            format!("{error:#}").to_lowercase().contains(expected),
            "unexpected error: {error:#}"
        );
    }
    Ok(())
}
struct Selected {
    meta: Meta,
    partial: bool,
    wrong_parent: bool,
}
impl Plugin<output::RustCli> for Selected {
    fn kind(&self) -> &'static str {
        "substituted-graphql"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![
            Provision::of::<GraphqlOperations>(),
            Provision::of::<Blocks<GraphqlOperation>>(),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, output::RustCli>) -> Result<()> {
        let source = tempfile::tempdir()?;
        std::fs::write(source.path().join("schema.graphql"), SCHEMA)?;
        std::fs::write(source.path().join("operations.graphql"), OPERATIONS)?;
        let loaded = poolster_input_graphql::GraphqlInput.load_with_options(
            &source.path().join("schema.graphql"),
            &InputOptions {
                operation_files: vec![source.path().join("operations.graphql")],
                ..Default::default()
            },
        )?;
        let whole = loaded.get::<GraphqlOperations>()?.clone();
        let reference =
            ContractReference::from_bytes(GraphqlOperations::NAME, "source", b"revision");
        let mut blocks = whole
            .operation_blocks("source")
            .with_parent(reference.clone());
        blocks.items.retain(|op| op.value.name == "ReadUser");
        if self.partial {
            blocks.state = CollectionState::Partial {
                diagnostics: vec!["partial".into()],
            };
        }
        if self.wrong_parent {
            blocks = blocks.with_parent(ContractReference::from_bytes(
                GraphqlOperations::NAME,
                "other",
                b"revision",
            ));
        }
        cx.publish_with_reference(whole, reference)?;
        cx.publish(blocks)
    }
}
#[test]
fn selected_operation_blocks_enforce_completeness_and_revision() -> Result<()> {
    for (partial, wrong_parent) in [(false, false), (true, false), (false, true)] {
        let source = Selected {
            meta: Meta::new(),
            partial,
            wrong_parent,
        };
        let cli = output::graphql()
            .input(source.meta.handle())
            .input_operations(source.meta.handle());
        let result = Packages::new()
            .package(output::package("cli").with(source).with(cli))
            .generate_native();
        if partial || wrong_parent {
            let error = result.expect_err("invalid block selection accepted");
            ensure!(format!("{error:#}").contains(if partial { "complete" } else { "revision" }));
        } else {
            let root = tempfile::tempdir()?;
            result?.write_to(root.path())?;
            ensure!(
                !root
                    .path()
                    .join("cli/src/operations/operation-rename.rs")
                    .exists()
            );
        }
    }
    Ok(())
}
fn successful(command: &mut Command) -> Result<Output> {
    let result = command.output()?;
    ensure!(
        result.status.success(),
        "{command:?}: {}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(result)
}
#[test]
#[ignore = "requires cached generated Cargo dependencies and pinned POOLSTER_GRAPHQL_JS; starts local HTTP server"]
fn generated_cli_compiles_and_executes_graphql() -> Result<()> {
    use std::io::{BufRead, BufReader};
    let root = tempfile::tempdir()?;
    let large_operations = format!(
        "{OPERATIONS}\n{}",
        (0..300)
            .map(|i| format!("query Indexed{i:03} {{ presence }}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    generate(root.path(), &large_operations)?.write_to(root.path())?;
    successful(
        Command::new("cargo")
            .args(["build", "--offline", "--quiet"])
            .current_dir(root.path().join("cli"))
            .env("CARGO_TARGET_DIR", root.path().join("target")),
    )?;
    let mut process = Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/graphql_server.cjs"
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
    BufReader::new(process.stdout.take().unwrap()).read_line(&mut endpoint)?;
    let _server = Server(process);
    let binary = root.path().join("target/debug/users");
    let run = |args: &[&str]| -> Result<Output> {
        Ok(Command::new(&binary)
            .env("GRAPHQL_ENDPOINT", endpoint.trim())
            .env("GRAPHQL_TOKEN", "secret")
            .args(["--header", "x-test: configured"])
            .args(args)
            .output()?)
    };
    let help = run(&["--help"])?;
    ensure!(help.status.success() && String::from_utf8_lossy(&help.stdout).contains("indexed299"));
    let read = run(&["read-user", "--variables", r#"{"id":"7"}"#])?;
    ensure!(read.status.success());
    ensure!(
        serde_json::from_slice::<serde_json::Value>(&read.stdout)?["data"]["user"]["name"] == "Ada"
    );
    let mutation = run(&["rename", "--variables", r#"{"name":"New"}"#])?;
    ensure!(mutation.status.success());
    ensure!(
        serde_json::from_slice::<serde_json::Value>(&mutation.stdout)?["data"]["rename"]["name"]
            == "New"
    );
    for (args, code) in [
        (vec!["partial", "--variables", r#"{"id":"7"}"#], 3),
        (vec!["fatal"], 4),
        (vec!["read-user", "--variables", "[1]"], 2),
        (vec!["read-user", "--variables", "{"], 2),
        (vec!["read-user", "--variables", r#"{"wrong":1}"#], 2),
        (vec!["read-user", "--variables", r#"{"id":null}"#], 4),
        (vec!["read-user", "--variables", r#"{"id":{}}"#], 4),
        (
            vec![
                "fatal",
                "--endpoint",
                &endpoint.trim().replace("/graphql", "/bad"),
            ],
            2,
        ),
    ] {
        let result = run(&args)?;
        ensure!(
            result.status.code() == Some(code),
            "{args:?}: {}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let absent = run(&["presence"])?;
    let null = run(&["presence", "--variables", r#"{"value":null}"#])?;
    ensure!(
        serde_json::from_slice::<serde_json::Value>(&absent.stdout)?["data"]["presence"]
            == "absent"
    );
    ensure!(
        serde_json::from_slice::<serde_json::Value>(&null.stdout)?["data"]["presence"] == "null"
    );
    let file = root.path().join("vars.json");
    std::fs::write(&file, r#"{"id":"8"}"#)?;
    let from_file = run(&["read-user", "--variables-file", file.to_str().unwrap()])?;
    ensure!(from_file.status.success());
    let unauth = Command::new(&binary)
        .args(["--endpoint", endpoint.trim(), "fatal"])
        .env_remove("GRAPHQL_TOKEN")
        .output()?;
    ensure!(unauth.status.code() == Some(2));
    Ok(())
}

#[test]
fn many_graphql_operations_are_bounded_and_order_independent() -> Result<()> {
    let root = tempfile::tempdir()?;
    let operations = (0..300)
        .map(|i| format!("query Read{i:03} {{ presence }}"))
        .collect::<Vec<_>>();
    let first = generate(root.path(), &operations.join("\n"))?;
    let mut reversed = operations;
    reversed.reverse();
    let second = generate(root.path(), &reversed.join("\n"))?;
    ensure!(
        first.iter().collect::<Vec<_>>() == second.iter().collect::<Vec<_>>(),
        "reordering changed generated files"
    );
    for (path, source) in first.iter() {
        if path.extension().is_some_and(|extension| extension == "rs") {
            ensure!(
                source.len() < 32_768 && source.lines().count() < 200,
                "unbounded module {}",
                path.display()
            );
        }
    }
    Ok(())
}
