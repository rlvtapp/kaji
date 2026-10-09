use super::*;
use poolster_core::native::GraphqlOperation;
use std::{fs, process::Command};
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
    GraphqlOperations{schema_source:String::new(),operation_source:String::new(),input_objects:BTreeMap::new(),operations:vec![GraphqlOperation{name:"ReadUser".into(),kind:GraphqlOperationKind::Query,document:"query ReadUser($id: ID!, $include: Boolean! = false) { readUser(id: $id) { name nickname @include(if: $include) } }".into(),variables:vec![field("id",scalar("ID",false),false),field("include",scalar("Boolean",false),true)],result:ModelType{nullable:false,kind:ModelKind::Object(vec![field("readUser",ModelType{nullable:true,kind:ModelKind::Object(vec![field("name",scalar("String",false),false),field("nickname",scalar("String",true),true)])},false)])}}]}
}
#[test]
fn rejects_unsupported_and_invalid_configuration() {
    let mut c = fixture();
    c.operations[0].kind = GraphqlOperationKind::Subscription;
    assert!(
        render(&c, "poolster/test", GraphqlStyle::Flat, &BTreeMap::new())
            .unwrap_err()
            .to_string()
            .contains("subscriptions")
    );
    assert!(render(&fixture(), "../bad", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    let groups = BTreeMap::from([(
        "users".into(),
        BTreeMap::from([("read".into(), "Missing".into())]),
    )]);
    assert!(
        render(
            &fixture(),
            "poolster/test",
            GraphqlStyle::Idiomatic,
            &groups
        )
        .is_err()
    );
    let mut c = fixture();
    c.operations.push(c.operations[0].clone());
    assert!(render(&c, "poolster/test", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
}
#[test]
fn generated_php_compiles_and_executes() {
    run_generated(None)
}
fn run_generated(endpoint: Option<&str>) {
    let php = std::env::var("POOLSTER_TEST_PHP").unwrap_or("php".into());
    assert!(
        Command::new(&php)
            .arg("--version")
            .status()
            .expect("PHP 8.2+ required (set POOLSTER_TEST_PHP)")
            .success()
    );
    for (style, custom) in [
        (GraphqlStyle::Raw, false),
        (GraphqlStyle::Flat, false),
        (GraphqlStyle::Idiomatic, false),
        (GraphqlStyle::Idiomatic, true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let groups = if custom {
            BTreeMap::from([(
                "users".into(),
                BTreeMap::from([("read".into(), "ReadUser".into())]),
            )])
        } else {
            BTreeMap::new()
        };
        let (tree, _) = render(&fixture(), "poolster/test", style, &groups).unwrap();
        fs::write(
            dir.path().join("Graphql.php"),
            tree.get("src/Graphql.php").unwrap(),
        )
        .unwrap();
        let call = match style {
            GraphqlStyle::Raw => "\\Poolster\\Test\\readUser($client,$variables)",
            GraphqlStyle::Flat => "$client->readUser($variables)",
            GraphqlStyle::Idiomatic if custom => "$client->users()->read($variables)",
            GraphqlStyle::Idiomatic => "$client->query()->readUser($variables)",
        };
        let script = include_str!("runtime_test.php.tmpl").replace("CALL_OPERATION", call);
        fs::write(dir.path().join("test.php"), script).unwrap();
        let out = Command::new(&php)
            .arg("-l")
            .arg(dir.path().join("Graphql.php"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let mut command = Command::new(&php);
        command.arg(dir.path().join("test.php"));
        if let Some(endpoint) = endpoint {
            command.env("POOLSTER_GRAPHQL_ENDPOINT", endpoint);
        }
        let out = command.output().unwrap();
        assert!(
            out.status.success(),
            "{} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
#[ignore = "requires PHP 8.2 and pinned GraphQL.js16.14.2 (POOLSTER_GRAPHQL_JS_ROOT)"]
fn all_styles_against_pinned_graphql_server() {
    use std::io::{BufRead, BufReader};
    let root =
        std::env::var("POOLSTER_GRAPHQL_JS_ROOT").expect("POOLSTER_GRAPHQL_JS_ROOT required");
    let mut server = Command::new("node")
        .args(["-e", include_str!("server.cjs")])
        .env("POOLSTER_GRAPHQL_JS_ROOT", root)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(server.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let port = line.trim().parse::<u16>().expect("server port");
    let host = std::env::var("POOLSTER_GRAPHQL_SERVER_HOST").unwrap_or("127.0.0.1".into());
    let result = std::panic::catch_unwind(|| run_generated(Some(&format!("http://{host}:{port}"))));
    server.kill().unwrap();
    server.wait().unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
#[test]
fn nested_models_variables_mutations_and_no_variable_calls() {
    let mut c = fixture();
    c.input_objects.insert(
        "Filter".into(),
        vec![
            field("label", scalar("String", true), true),
            field(
                "next",
                ModelType {
                    nullable: true,
                    kind: ModelKind::Named("Filter".into()),
                },
                true,
            ),
        ],
    );
    c.operations[0].variables.push(field(
        "filter",
        ModelType {
            nullable: true,
            kind: ModelKind::Named("Filter".into()),
        },
        true,
    ));
    let variant = |name: &str, key: &str| ModelType {
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
            field(key, scalar("String", false), false),
        ]),
    };
    c.operations.push(GraphqlOperation {
        name: "Update".into(),
        kind: GraphqlOperationKind::Mutation,
        document: "mutation Update { update { __typename } }".into(),
        variables: vec![],
        result: ModelType {
            nullable: false,
            kind: ModelKind::Object(vec![
                field(
                    "items",
                    ModelType {
                        nullable: false,
                        kind: ModelKind::List(Box::new(ModelType {
                            nullable: true,
                            kind: ModelKind::Union(vec![
                                variant("User", "name"),
                                variant("Team", "title"),
                            ]),
                        })),
                    },
                    false,
                ),
                field(
                    "role",
                    ModelType {
                        nullable: false,
                        kind: ModelKind::Enum(vec!["ADMIN".into(), "USER".into()]),
                    },
                    false,
                ),
            ]),
        },
    });
    let (tree, _) = render(
        &c,
        "poolster/test",
        GraphqlStyle::Idiomatic,
        &BTreeMap::new(),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("Graphql.php"),
        tree.get("src/Graphql.php").unwrap(),
    )
    .unwrap();
    let script = r#"<?php
require __DIR__.'/Graphql.php';
use Poolster\Test\{Client,Presence,Filter,ReadUserVariables};
$variables=new ReadUserVariables(id:'1',filter:Presence::of(new Filter(label:Presence::of(null),next:Presence::of(new Filter()))));
$value=json_decode(json_encode($variables,JSON_THROW_ON_ERROR),true);if($value['filter']!==['label'=>null,'next'=>[]])throw new RuntimeException('presence lost');
$transport=function($url,$body){$body=json_decode($body,true);if($body['operationName']!=='Update'||$body['variables']!==[])throw new RuntimeException('bad operation');return [200,'{"data":{"items":[{"__typename":"User","name":"Ada"},{"__typename":"Team","title":"Core"},null],"role":"ADMIN"}}'];};
$r=(new Client('http://localhost',transport:$transport))->mutation()->update();
$data=$r->requireData();if($data->items[0]->name!=='Ada'||$data->items[1]->title!=='Core'||$data->items[2]!==null||$data->role!=='ADMIN')throw new RuntimeException('typed decode failed');
try{\Poolster\Test\UpdateResult::fromArray(['items'=>[],'role'=>'INVALID']);throw new RuntimeException('enum accepted');}catch(UnexpectedValueException $e){}
"#;
    fs::write(dir.path().join("test.php"), script).unwrap();
    let out = Command::new(std::env::var("POOLSTER_TEST_PHP").unwrap_or("php".into()))
        .arg(dir.path().join("test.php"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let ModelKind::Object(fields) = &mut c.operations[1].result.kind else {
        unreachable!()
    };
    let ModelKind::List(item) = &mut fields[0].ty.kind else {
        unreachable!()
    };
    let ModelKind::Union(members) = &mut item.kind else {
        unreachable!()
    };
    let ModelKind::Object(fields) = &mut members[0].kind else {
        unreachable!()
    };
    fields.remove(0);
    assert!(
        render(&c, "poolster/test", GraphqlStyle::Flat, &BTreeMap::new())
            .unwrap_err()
            .to_string()
            .contains("discriminator")
    );
}
struct Source {
    meta: Meta,
    name: &'static str,
}
impl Plugin<Php> for Source {
    fn kind(&self) -> &'static str {
        "test-graphql-input"
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
    fn generate(&self, cx: &mut PluginContext<'_, Php>) -> Result<()> {
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
        let ignored = Source {
            meta: Meta::new(),
            name: "Ignored",
        };
        let output = graphql(Some(source.meta.handle())).flat();
        let tree = poolster_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .with(output)
                    .with(ignored)
                    .with(source),
            )
            .generate_native()
            .unwrap();
        let code = tree.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
        assert!(code.contains(&format!("class {name}Result")));
        assert!(!code.contains("class IgnoredResult"));
    }
}
