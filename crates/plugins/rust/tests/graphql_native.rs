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
const SCHEMA: &str = "scalar Timestamp scalar Json input ScalarInput { required:Timestamp! values:[Timestamp]! optional:Timestamp } type ScalarResult { timestamp:Timestamp! values:[Timestamp]! nullable:Timestamp optional:Timestamp raw:Json! } input Options { note: String = \"input-default\" } type User { id: ID! name: String! nickname: String fragile: String } interface Node { id:ID! } type Person implements Node { id:ID! name:String! } type Robot implements Node { id:ID! code:String! } type Query { scalars(value:Timestamp!,input:ScalarInput!,optional:Timestamp):ScalarResult! node:Node! user(id: ID!): User! fatal: String! echo(input:String = \"argument-default\"):String inputEcho(options:Options):String } type Mutation { rename(name:String!):User! }";
const OPS: &str = "query Scalars($value:Timestamp!,$input:ScalarInput!,$optional:Timestamp,$include:Boolean!){scalars(value:$value,input:$input,optional:$optional){timestamp values nullable optional @include(if:$include) raw}} query AbstractNode { node { id ... on Person { name } ... on Robot { code } } } query Read($id:ID!){person:user(id:$id){id name nickname}} query Partial($id:ID!){user(id:$id){name fragile}} query Fatal{fatal} mutation Rename($name:String!){rename(name:$name){name}} query PresenceQuery($value:String){value:echo(input:$value)} query InputPresence($options:Options){value:inputEcho(options:$options)} query Conditional($include:Boolean!){user(id:\"7\"){name @include(if:$include) nickname @include(if:$include)}} query Defaulted($include:Boolean! = false){user(id:\"7\"){name @include(if:$include) nickname @include(if:$include)}}";
fn generate(root: &Path) -> Result<GeneratedTree> {
    generate_style(root, "idiomatic")
}
fn generate_style(root: &Path, style: &str) -> Result<GeneratedTree> {
    std::fs::write(root.join("schema.graphql"), SCHEMA)?;
    std::fs::write(
        root.join("operations.graphql"),
        if style == "collision" {
            "query FooBar { fatal } query foo_bar { fatal }"
        } else {
            OPS
        },
    )?;
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
    let generator = rust::graphql(Some(input.handle())).scalar(
        "Timestamp",
        rust::GraphqlScalarMapping::new("String", "i64"),
    );
    let generator = match style {
        "raw" => generator.raw(),
        "flat" | "collision" => generator.flat(),
        "reserved-group" => generator.idiomatic().group("new", "read", "Read"),
        "group-collision" => generator
            .idiomatic()
            .group("userProfile", "read", "Read")
            .group("user_profile", "rename", "Rename"),
        "method-collision" => generator
            .idiomatic()
            .group("user", "readUser", "Read")
            .group("user", "read_user", "Rename"),
        "unknown-group" => generator
            .idiomatic()
            .group("user", "read", "MissingOperation"),
        "grouped" => generator
            .idiomatic()
            .group("user", "read", "Read")
            .group("user", "rename", "Rename"),
        _ => generator.idiomatic(),
    };
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
    let target = tempfile::tempdir()?;
    for style in ["raw", "flat", "idiomatic", "grouped"] {
        packaged_style(style, target.path())?;
    }
    Ok(())
}
fn packaged_style(style: &str, target: &Path) -> Result<()> {
    use std::io::{BufRead, BufReader};
    let dir = tempfile::tempdir()?;
    generate_style(dir.path(), style)?.write_to(dir.path())?;
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
            "[workspace]\n[package]\nname='graphql-consumer'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nclient={{package='poolster-graphql-test-sdk',path={:?}}}\ntokio={{version='1',features=['macros','rt-multi-thread','time']}}\nserde_json='=1.0.151'\n",
            installed.to_string_lossy()
        ),
    )?;
    std::fs::write(
        consumer.join("src/main.rs"),
        format!(
            "{}\n{}",
            include_str!("fixtures/graphql_native/consumer.rs"),
            style_consumer(style)
        ),
    )?;
    successful(
        Command::new("cargo")
            .args(["run", "--offline", "--quiet"])
            .current_dir(&consumer)
            .env("GRAPHQL_ENDPOINT", endpoint.trim())
            .env("CARGO_TARGET_DIR", target),
    )?;
    std::fs::create_dir(consumer.join("src/bin"))?;
    std::fs::write(
        consumer.join("src/bin/invalid.rs"),
        "use client::*; fn main() { let _ = ReadVariables { id: 7 }; let _ = ScalarInput { required: 123, values: vec![], optional: Presence::Absent }; } fn invalid(result: ReadResult) { let _ = result.person.fragile; }",
    )?;
    let invalid = Command::new("cargo")
        .args(["check", "--offline", "--bin", "invalid"])
        .current_dir(&consumer)
        .env("CARGO_TARGET_DIR", target)
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

fn style_consumer(style: &str) -> String {
    if style == "raw" {
        return "async fn style_probe(_: &str) {}".into();
    }
    let query = if style == "flat" {
        "client"
    } else {
        "client.query()"
    };
    let read = if style == "grouped" {
        "client.user()"
    } else {
        query
    };
    let mutation = if style == "flat" {
        "client"
    } else if style == "grouped" {
        "client.user()"
    } else {
        "client.mutation()"
    };
    format!(
        r#"async fn style_probe(endpoint:&str) {{
 let mut headers=reqwest::header::HeaderMap::new();headers.insert("x-style",reqwest::header::HeaderValue::from_static("configured-once"));
 let http=reqwest::Client::builder().default_headers(headers).timeout(std::time::Duration::from_secs(3)).build().unwrap();
 let client=Client::new(format!("{{endpoint}}/style"),http);
 assert_eq!(success({read}.read(&ReadVariables{{id:"7".into()}}).await.unwrap()).person.name,"Ada");
 assert_eq!(success({mutation}.rename(&RenameVariables{{name:"Style".into()}}).await.unwrap()).rename.name,"Style");
 assert!(matches!({query}.partial(&PartialVariables{{id:"7".into()}}).await.unwrap(),GraphqlResponse::Partial{{..}}));
 assert!(matches!({query}.fatal().await.unwrap(),GraphqlResponse::Error{{..}}));
 assert!(success({query}.presence_query(&PresenceQueryVariables{{value:Presence::Null}}).await.unwrap()).value.is_none());
 assert!(matches!(success({query}.conditional(&ConditionalVariables{{include:false}}).await.unwrap()).user.name,Optional::Absent));
 let variables=ScalarsVariables{{value:"wire-time".into(),input:ScalarInput{{required:"nested-time".into(),values:vec![Some("list-time".into()),None],optional:Presence::Null}},optional:Presence::Absent,include:false}};
 assert_eq!(success({query}.scalars(&variables).await.unwrap()).scalars.timestamp,1700000000);
 let client=Client::new(format!("{{endpoint}}/bad-scalar"),reqwest::Client::new());
 assert!(matches!({query}.scalars(&variables).await.unwrap_err(),GraphqlTransportError::Decode(_)));
 let client=Client::new(format!("{{endpoint}}/slow"),reqwest::Client::builder().timeout(std::time::Duration::from_millis(30)).build().unwrap());
 assert!(matches!({read}.read(&ReadVariables{{id:"7".into()}}).await.unwrap_err(),GraphqlTransportError::Network(error) if error.is_timeout()));
 let client=Client::new(format!("{{endpoint}}/slow"),reqwest::Client::new());
 let task=tokio::spawn(async move {{ {read}.read(&ReadVariables{{id:"7".into()}}).await }});
 tokio::time::sleep(std::time::Duration::from_millis(10)).await;task.abort();assert!(task.await.unwrap_err().is_cancelled());
}}
"#
    )
}

#[test]
fn graphql_style_collision_and_unknown_operation_are_diagnostics() -> Result<()> {
    for (style, expected) in [
        ("collision", "collision"),
        ("unknown-group", "MissingOperation"),
        ("reserved-group", "reserved"),
        ("group-collision", "group name collision"),
        ("method-collision", "method collision"),
    ] {
        let root = tempfile::tempdir()?;
        let error =
            generate_style(root.path(), style).expect_err("invalid style should fail generation");
        ensure!(
            format!("{error:#}").contains(expected),
            "unexpected diagnostic: {error:#}"
        );
    }
    Ok(())
}

#[test]
fn graphql_bound_styles_have_snake_case_methods_and_zero_argument_queries() -> Result<()> {
    for (style, expected) in [
        ("flat", "pub async fn presence_query"),
        ("idiomatic", "pub fn query"),
        ("grouped", "pub fn user"),
    ] {
        let root = tempfile::tempdir()?;
        let tree = generate_style(root.path(), style)?;
        tree.write_to(root.path())?;
        let source = std::fs::read_to_string(root.path().join("sdk/src/graphql.rs"))?;
        ensure!(source.contains(expected), "missing {expected} for {style}");
        ensure!(
            source.contains("pub async fn fatal(&self)"),
            "empty variables must not require a dummy argument"
        );
        ensure!(source.contains("pub async fn read(&self, variables: &ReadVariables)"));
        ensure!(
            source.contains("pub async fn fatal(transport:"),
            "raw transport functions remain available"
        );
    }
    Ok(())
}
