use super::*;
use poolster_core::native::{GraphqlOperation, ModelField, ModelKind, ModelType};
fn scalar(name: &str, nullable: bool) -> ModelType {
    ModelType {
        nullable,
        kind: ModelKind::Scalar(name.into()),
    }
}
fn field(name: &str, ty: ModelType, optional: bool) -> ModelField {
    ModelField {
        name: name.into(),
        ty,
        optional,
        default_value: None,
    }
}
fn fixture() -> GraphqlOperations {
    GraphqlOperations{schema_source:String::new(),operation_source:String::new(),input_objects:BTreeMap::new(),operations:vec![GraphqlOperation{name:"ReadUser".into(),kind:GraphqlOperationKind::Query,document:"query ReadUser($id: ID!, $nick: Boolean = true) { readUser(id: $id) { name nickname @include(if: $nick) } }".into(),variables:vec![field("id",scalar("ID",false),false),field("nick",scalar("Boolean",false),true)],result:ModelType{nullable:false,kind:ModelKind::Object(vec![field("readUser",ModelType{nullable:true,kind:ModelKind::Object(vec![field("name",scalar("String",false),false),field("nickname",scalar("String",true),true)])},false)])}},GraphqlOperation{name:"Ping".into(),kind:GraphqlOperationKind::Mutation,document:"mutation Ping {ping}".into(),variables:vec![],result:ModelType{nullable:false,kind:ModelKind::Object(vec![field("ping",scalar("Boolean",false),false)])}}]}
}

fn complex_fixture() -> GraphqlOperations {
    let mut c = fixture();
    c.input_objects.insert(
        "Filter".into(),
        vec![field("name", scalar("String", true), true)],
    );
    c.operations[0].variables.push(field(
        "filter",
        ModelType {
            nullable: true,
            kind: ModelKind::Named("Filter".into()),
        },
        true,
    ));
    c.operations[0].document="query ReadUser($id: ID!, $nick: Boolean = true, $filter: Filter) {readUser(id:$id,filter:$filter){name nickname @include(if:$nick)}}".into();
    let variant = |name: &str| ModelType {
        nullable: false,
        kind: ModelKind::Object(vec![
            field(
                "__typename",
                ModelType {
                    nullable: false,
                    kind: ModelKind::Literal(name.into()),
                },
                false,
            ),
            field("name", scalar("String", false), false),
        ]),
    };
    c.operations.push(GraphqlOperation {
        name: "Complex".into(),
        kind: GraphqlOperationKind::Query,
        document: "query Complex {nodes {__typename ... on User {name} ... on Team {name}} matrix}"
            .into(),
        variables: vec![],
        result: ModelType {
            nullable: false,
            kind: ModelKind::Object(vec![
                field(
                    "nodes",
                    ModelType {
                        nullable: false,
                        kind: ModelKind::List(Box::new(ModelType {
                            nullable: true,
                            kind: ModelKind::Union(vec![variant("User"), variant("Team")]),
                        })),
                    },
                    false,
                ),
                field(
                    "matrix",
                    ModelType {
                        nullable: true,
                        kind: ModelKind::List(Box::new(ModelType {
                            nullable: true,
                            kind: ModelKind::List(Box::new(scalar("String", true))),
                        })),
                    },
                    false,
                ),
            ]),
        },
    });
    c
}
#[test]
fn renders_native_styles() {
    for style in [
        GraphqlStyle::Raw,
        GraphqlStyle::Flat,
        GraphqlStyle::Idiomatic,
    ] {
        let (tree, methods) = render(&fixture(), "example", style, &BTreeMap::new()).unwrap();
        assert_eq!(methods["ReadUser"], "read_user");
        assert!(
            tree.get("lib/example/models/read_user_result_read_user.ex")
                .unwrap()
                .contains("nickname: String.t() | nil | :poolster_absent")
        );
        assert!(tree.get("mix.exs").unwrap().contains("== 1.4.5"));
    }
}
#[test]
fn rejects_subscription_and_unknown_group() {
    let mut input = fixture();
    input.operations[0].kind = GraphqlOperationKind::Subscription;
    assert!(render(&input, "example", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    assert!(
        render(
            &fixture(),
            "example",
            GraphqlStyle::Idiomatic,
            &BTreeMap::from([(
                "Users".into(),
                BTreeMap::from([("read".into(), "Missing".into())])
            )])
        )
        .is_err()
    );
}
#[test]
#[ignore = "requires Elixir Mix toolchain with pinned Finch/Jason and GraphQL.js16.14.2 local server"]
fn live_server_all_styles() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let root =
        std::env::var("POOLSTER_GRAPHQL_JS_ROOT").expect("set pinned GraphQL.js modules root");
    let mut server = std::process::Command::new("node")
        .args(["-e", include_str!("server.cjs")])
        .env("POOLSTER_GRAPHQL_JS_ROOT", root)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut port = String::new();
    BufReader::new(server.stdout.take().unwrap())
        .read_line(&mut port)
        .unwrap();
    struct Stop<'a>(&'a mut std::process::Child);
    impl Drop for Stop<'_> {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _stop = Stop(&mut server);
    let host = std::env::var("POOLSTER_GRAPHQL_HOST").unwrap_or("127.0.0.1".into());
    let endpoint = format!("http://{host}:{}", port.trim());
    let output = std::env::var("POOLSTER_ELIXIR_GRAPHQL_OUTPUT")
        .expect("set output directory for toolchain test");
    for (style, name, custom) in [
        (GraphqlStyle::Raw, "raw", false),
        (GraphqlStyle::Flat, "flat", false),
        (GraphqlStyle::Idiomatic, "grouped", false),
        (GraphqlStyle::Idiomatic, "custom", true),
    ] {
        let mut groups = if custom {
            BTreeMap::from([
                (
                    "Users".into(),
                    BTreeMap::from([("read".into(), "ReadUser".into())]),
                ),
                (
                    "Actions".into(),
                    BTreeMap::from([("ping".into(), "Ping".into())]),
                ),
            ])
        } else {
            BTreeMap::new()
        };
        if custom {
            groups.insert(
                "Analytics".into(),
                BTreeMap::from([("complex".into(), "Complex".into())]),
            );
        }
        let (tree, _) = render(&complex_fixture(), "example", style, &groups).unwrap();
        let dir = std::path::Path::new(&output).join(name);
        std::fs::create_dir_all(&dir).unwrap();
        tree.write_to(&dir).unwrap();
        let call = match style {
            GraphqlStyle::Raw => "Example.Operations.read_user",
            GraphqlStyle::Flat => "Example.read_user",
            GraphqlStyle::Idiomatic if custom => "Example.Users.read",
            GraphqlStyle::Idiomatic => "Example.Query.read_user",
        };
        let ping = match style {
            GraphqlStyle::Raw => "Example.Operations.ping",
            GraphqlStyle::Flat => "Example.ping",
            GraphqlStyle::Idiomatic if custom => "Example.Actions.ping",
            GraphqlStyle::Idiomatic => "Example.Mutation.ping",
        };
        let complex = match style {
            GraphqlStyle::Raw => "Example.Operations.complex",
            GraphqlStyle::Flat => "Example.complex",
            GraphqlStyle::Idiomatic if custom => "Example.Analytics.complex",
            GraphqlStyle::Idiomatic => "Example.Query.complex",
        };
        let script = include_str!("runtime_test.exs.tmpl")
            .replace("__CALL__", call)
            .replace("__PING__", ping)
            .replace("__COMPLEX__", complex);
        std::fs::write(dir.join("probe.exs"), script).unwrap();
        let command = std::env::var("POOLSTER_ELIXIR_RUNNER").unwrap_or("mix".into());
        let args = if command == "mix" {
            vec!["run", "probe.exs"]
        } else {
            vec![dir.to_str().unwrap()]
        };
        let out = std::process::Command::new(command)
            .args(args)
            .current_dir(&dir)
            .env("POOLSTER_GRAPHQL_ENDPOINT", &endpoint)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

struct Source {
    meta: Meta,
    name: &'static str,
}
impl Plugin<Elixir> for Source {
    fn kind(&self) -> &'static str {
        "test-graphql-source"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlOperations>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Elixir>) -> Result<()> {
        let mut c = fixture();
        c.operations[0].name = self.name.into();
        cx.publish(c)
    }
}
#[test]
fn selected_provider_and_regeneration() {
    for name in ["ReadUser", "Renamed"] {
        let source = Source {
            meta: Meta::new(),
            name,
        };
        let other = Source {
            meta: Meta::new(),
            name: "Ignored",
        };
        let plugin = graphql(Some(source.meta.handle())).flat();
        let tree = poolster_core::engine::Packages::new()
            .package(crate::package("sdk").with(other).with(plugin).with(source))
            .generate_native()
            .unwrap();
        let content = tree.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
        assert!(content.contains(&format!("Models.{name}Variables")));
        assert!(!content.contains("Models.IgnoredVariables"));
    }
}

#[test]
fn many_operations_have_bounded_modules() {
    let mut c = fixture();
    let original = c.operations[0].clone();
    c.operations=(0..300).map(|i|{let mut op=original.clone();op.name=format!("Read{i}");op.document=format!("query Read{i}($id: ID!, $nick: Boolean = true) {{readUser(id:$id){{name nickname @include(if:$nick)}}}}");op}).collect();
    let (tree, _) = render(&c, "example", GraphqlStyle::Idiomatic, &BTreeMap::new()).unwrap();
    assert_eq!(
        tree.iter()
            .filter(|(p, _)| p.to_string_lossy().contains("/operations/"))
            .count(),
        300
    );
    assert_eq!(
        tree.iter()
            .filter(|(p, _)| p.to_string_lossy().contains("/models/"))
            .count(),
        900
    );
    assert!(
        !tree
            .get("lib/example/operations.ex")
            .unwrap()
            .contains("query Read")
    );
    assert!(
        tree.get("lib/example/operations.ex")
            .unwrap()
            .contains("use Example.Internal")
    );
    for (p, source) in tree.iter() {
        if p.extension().is_some_and(|ext| ext == "ex") {
            assert!(source.len() < 128 * 1024, "{} oversized", p.display());
        }
    }
}
#[test]
fn filenames_are_portable_and_case_collisions_reject() {
    let stem = layout::stem(&"UpperCase".repeat(30));
    assert!(stem.len() <= 180);
    assert_eq!(stem, layout::stem(&"UpperCase".repeat(30)));
    let mut c = fixture();
    c.input_objects.insert("FOO".into(), vec![]);
    c.input_objects.insert("Foo".into(), vec![]);
    assert!(
        render(&c, "example", GraphqlStyle::Flat, &BTreeMap::new())
            .unwrap_err()
            .to_string()
            .contains("model collision")
    );
    let mut c = fixture();
    c.operations[0].name = "A".repeat(250);
    assert!(
        render(&c, "example", GraphqlStyle::Flat, &BTreeMap::new())
            .unwrap_err()
            .to_string()
            .contains("atom name limit")
    );
}
