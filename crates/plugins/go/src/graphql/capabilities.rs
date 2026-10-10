use super::*;
use std::{
    fs,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};
const SCHEMA: &str = "directive @defer(if:Boolean! = true,label:String) on FRAGMENT_SPREAD | INLINE_FRAGMENT directive @stream(if:Boolean! = true,label:String,initialCount:Int! = 0) on FIELD scalar DateTime input Filter {when:DateTime! list:[DateTime] next:Filter} type Stamp {when:DateTime! maybe:DateTime list:[DateTime] name:String!} type User{id:ID! when:DateTime!} type Query{stamp(input:Filter!):Stamp! user:User! values:[Int!]!} type Subscription{ticks(start:DateTime!):Stamp!}";
const REGULAR: &str = "query Read($input:Filter!,$include:Boolean!){stamp(input:$input){when maybe @include(if:$include) list name}} subscription Ticks($start:DateTime!){ticks(start:$start){when maybe list name}}";
const INCREMENTAL: &str = "query Feed($count:Int!){user{id ... @defer(label:\"details\"){when}} values @stream(label:\"values\",initialCount:$count)}";
fn fixtures() -> (GraphqlOperations, GraphqlIncrementalOperations) {
    let schema = poolster_input_graphql::parse(SCHEMA).unwrap();
    (
        poolster_input_graphql::lower_operations(&schema.schema, SCHEMA, REGULAR).unwrap(),
        poolster_input_graphql::lower_incremental_operations(&schema.schema, SCHEMA, INCREMENTAL)
            .unwrap(),
    )
}
#[test]
fn streaming_and_scalar_files_are_stable_and_opt_in() {
    let (regular, incremental) = fixtures();
    let mappings = BTreeMap::from([(
        "DateTime".into(),
        GraphqlScalarMapping::new("time.Time", "time.Time"),
    )]);
    assert!(render(&regular, "client", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    let first = render_capabilities(
        &regular,
        "client",
        GraphqlStyle::Flat,
        &BTreeMap::new(),
        true,
        &mappings,
        None,
    )
    .unwrap()
    .0;
    assert_eq!(
        first,
        render_capabilities(
            &regular,
            "client",
            GraphqlStyle::Flat,
            &BTreeMap::new(),
            true,
            &mappings,
            None
        )
        .unwrap()
        .0
    );
    assert!(
        first
            .values()
            .any(|source| source.contains("Subscription[TicksResult]"))
    );
    let files = render_capabilities(
        &incremental.definition,
        "client",
        GraphqlStyle::Flat,
        &BTreeMap::new(),
        false,
        &mappings,
        Some(&incremental),
    )
    .unwrap()
    .0;
    assert!(
        files
            .values()
            .any(|source| source.contains("IncrementalStream[FeedResult]"))
    );
    assert!(
        scalars::validate(&BTreeMap::from([(
            "DateTime".into(),
            GraphqlScalarMapping::new("time.Time;panic()", "time.Time")
        )]))
        .is_err()
    );
}
#[test]
#[ignore = "requires official pinned GraphQL.js16.14.2/graphql-sse2.6.0, Go and loopback sockets"]
fn generated_go_streams_and_typed_codecs_execute() {
    let root =
        std::env::var("POOLSTER_GRAPHQL_SSE_ROOT").expect("pinned GraphQL SSE dependency root");
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("schema.graphql"), SCHEMA).unwrap();
    fs::write(
        directory.path().join("server.cjs"),
        include_str!("capability-server.cjs"),
    )
    .unwrap();
    struct Server(std::process::Child);
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut server = Server(
        Command::new("node")
            .arg(directory.path().join("server.cjs"))
            .arg(directory.path().join("schema.graphql"))
            .env("POOLSTER_GRAPHQL_SSE_ROOT", root)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut endpoint = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut endpoint)
        .unwrap();
    assert!(!endpoint.trim().is_empty());
    let (regular, incremental) = fixtures();
    let mappings = BTreeMap::from([(
        "DateTime".into(),
        GraphqlScalarMapping::new("time.Time", "time.Time"),
    )]);
    for (case, style, custom) in [
        ("raw", GraphqlStyle::Raw, false),
        ("flat", GraphqlStyle::Flat, false),
        ("grouped", GraphqlStyle::Idiomatic, false),
        ("custom", GraphqlStyle::Idiomatic, true),
    ] {
        let project = directory.path().join(case);
        fs::create_dir_all(&project).unwrap();
        let regular_groups = if custom {
            BTreeMap::from([(
                "Users".into(),
                BTreeMap::from([
                    ("Read".into(), "Read".into()),
                    ("Ticks".into(), "Ticks".into()),
                ]),
            )])
        } else {
            BTreeMap::new()
        };
        let incremental_groups = if custom {
            BTreeMap::from([(
                "Feeds".into(),
                BTreeMap::from([("Feed".into(), "Feed".into())]),
            )])
        } else {
            BTreeMap::new()
        };
        let files = render_capabilities(
            &regular,
            "capabilities/regular",
            style,
            &regular_groups,
            true,
            &mappings,
            None,
        )
        .unwrap()
        .0;
        let stream = render_capabilities(
            &incremental.definition,
            "capabilities/incremental",
            style,
            &incremental_groups,
            false,
            &mappings,
            Some(&incremental),
        )
        .unwrap()
        .0;
        for (prefix, files) in [("regular", files), ("incremental", stream)] {
            for (path, source) in files {
                let path = project.join(prefix).join(path);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, source).unwrap();
            }
        }
        fs::write(project.join("go.mod"), "module capabilities\n\ngo 1.22\n").unwrap();
        let mut test = include_str!("capability_test.go.tmpl").to_owned();
        match style {
            GraphqlStyle::Raw => {
                for variable in ["client", "bad"] {
                    test = test.replace(
                        &format!("{variable}.Read(ctx,"),
                        &format!("regular.Read(ctx,{variable},"),
                    );
                }
                for variable in ["client", "copy"] {
                    test = test.replace(
                        &format!("{variable}.Ticks(ctx,"),
                        &format!(
                            "regular.Ticks(ctx,{},",
                            if variable == "copy" {
                                "&copy"
                            } else {
                                variable
                            }
                        ),
                    );
                }
                test = test.replace("client.Feed(ctx,", "incremental.Feed(ctx,client,");
            }
            GraphqlStyle::Idiomatic => {
                test = test
                    .replace(
                        ".Read(ctx,",
                        if custom {
                            ".Users().Read(ctx,"
                        } else {
                            ".Query().Read(ctx,"
                        },
                    )
                    .replace(
                        ".Ticks(ctx,",
                        if custom {
                            ".Users().Ticks(ctx,"
                        } else {
                            ".Subscription().Ticks(ctx,"
                        },
                    )
                    .replace(
                        ".Feed(ctx,",
                        if custom {
                            ".Feeds().Feed(ctx,"
                        } else {
                            ".Query().Feed(ctx,"
                        },
                    );
            }
            GraphqlStyle::Flat => {}
        }
        fs::write(project.join("main_test.go"), test).unwrap();
        let output = Command::new("go")
            .args(["test", "./..."])
            .current_dir(&project)
            .env("GOWORK", "off")
            .env(
                "GOCACHE",
                std::env::var_os("GOCACHE")
                    .unwrap_or_else(|| directory.path().join("cache").into_os_string()),
            )
            .env("GRAPHQL_ENDPOINT", endpoint.trim())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
