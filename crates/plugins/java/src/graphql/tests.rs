use super::*;
use poolster_core::native::{GraphqlOperation, ModelField, ModelKind, ModelType};
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
    GraphqlOperations {
        schema_source: String::new(),
        operation_source: String::new(),
        input_objects: BTreeMap::new(),
        operations: vec![GraphqlOperation {
            name: "ReadUser".into(),
            kind: GraphqlOperationKind::Query,
            document: "query ReadUser($id: ID!) { readUser(id: $id) { name nickname } }".into(),
            variables: vec![field("id", scalar("ID", false), false)],
            result: ModelType {
                nullable: false,
                kind: ModelKind::Object(vec![field(
                    "readUser",
                    ModelType {
                        nullable: true,
                        kind: ModelKind::Object(vec![
                            field("name", scalar("String", false), false),
                            field("nickname", scalar("String", true), true),
                        ]),
                    },
                    false,
                )]),
            },
        }],
    }
}
fn classpath() -> String {
    let home = std::env::var("HOME").unwrap();
    ["annotations","core","databind"].iter().map(|a|format!("{home}/.m2/repository/com/fasterxml/jackson/core/jackson-{a}/2.18.3/jackson-{a}-2.18.3.jar")).collect::<Vec<_>>().join(":")
}
#[test]
#[ignore = "requires JDK 17+ and pinned Jackson 2.18.3 jars in Maven cache"]
fn compiles_executes_all_styles() {
    for style in [
        GraphqlStyle::Raw,
        GraphqlStyle::Flat,
        GraphqlStyle::Idiomatic,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (tree, _) = render(&fixture(), "io.test", style, &BTreeMap::new()).unwrap();
        for (p, s) in tree.iter() {
            let p = dir.path().join(p);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, s).unwrap();
        }
        let call = match style {
            GraphqlStyle::Raw => "Client.readUser(client,new Client.ReadUserVariables(\"1\"))",
            GraphqlStyle::Flat => "client.readUser(new Client.ReadUserVariables(\"1\"))",
            GraphqlStyle::Idiomatic => {
                "client.query().readUser(new Client.ReadUserVariables(\"1\"))"
            }
        };
        fs::write(dir.path().join("Test.java"),format!(r#"import io.test.Client;import com.fasterxml.jackson.databind.*;public class Test {{public static void main(String[] args)throws Exception {{Client client=new Client(new Client.Transport(){{public <T> Client.Envelope<T> execute(String doc,String op,com.fasterxml.jackson.databind.node.ObjectNode vars,Client.Decoder<T> decoder)throws java.io.IOException{{if(!vars.path("id").asText().equals("1"))throw new AssertionError();return new Client.Envelope<>(true,decoder.decode(new ObjectMapper().readTree("{{\"readUser\":{{\"name\":\"Ada\",\"nickname\":null}}}}")),java.util.List.of(),null,200);}}}});var response={call};if(!response.requireData().readUser().name().equals("Ada"))throw new AssertionError();if(!response.data().readUser().nickname().present()||response.data().readUser().nickname().value()!=null)throw new AssertionError();}}}}"#)).unwrap();
        let output = Command::new("javac")
            .args([
                "-cp",
                &classpath(),
                "-d",
                "classes",
                "src/main/java/io/test/Client.java",
                "Test.java",
            ])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new("java")
            .args(["-cp", &format!("classes:{}", classpath()), "Test"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn rejects_subscription_and_bad_group() {
    let mut c = fixture();
    c.operations[0].kind = GraphqlOperationKind::Subscription;
    assert!(render(&c, "io.test", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    assert!(
        render(
            &fixture(),
            "io.test",
            GraphqlStyle::Idiomatic,
            &BTreeMap::from([(
                "users".into(),
                BTreeMap::from([("read".into(), "Unknown".into())])
            )])
        )
        .is_err()
    );
}

#[test]
#[ignore = "requires pinned GraphQL.js 16.14.2 modules and local TCP server"]
fn live_graphql_server_all_styles() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let root = std::env::var("POOLSTER_GRAPHQL_JS_ROOT").expect("set POOLSTER_GRAPHQL_JS_ROOT");
    let script = r#"const g=require(process.env.POOLSTER_GRAPHQL_JS_ROOT+'/graphql');if(require(process.env.POOLSTER_GRAPHQL_JS_ROOT+'/graphql/package.json').version!=='16.14.2')throw Error('Pinned GraphQL.js required');const schema=g.buildSchema('type Query {readUser(id: ID!): User} type User {name:String! nickname:String} type Mutation {ping:Boolean!}');const server=require('http').createServer(async(req,res)=>{let body='';for await(const c of req)body+=c;const v=JSON.parse(body);let result=await g.graphql({schema,source:v.query,operationName:v.operationName,variableValues:v.variables,rootValue:{ping:()=>true,readUser:({id})=>({name:'Ada',nickname:()=>{if(id==='partial')throw Error('nickname failed');return null}})}});res.setHeader('Content-Type','application/json');res.end(JSON.stringify(result));});server.listen(0,'127.0.0.1',()=>console.log(server.address().port));"#;
    let mut server = Command::new("node")
        .args(["-e", script])
        .env("POOLSTER_GRAPHQL_JS_ROOT", root)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    struct Stop<'a>(&'a mut std::process::Child);
    impl Drop for Stop<'_> {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut port = String::new();
    BufReader::new(server.stdout.take().unwrap())
        .read_line(&mut port)
        .unwrap();
    let _stop = Stop(&mut server);
    for (style, custom) in [
        (GraphqlStyle::Raw, false),
        (GraphqlStyle::Flat, false),
        (GraphqlStyle::Idiomatic, false),
        (GraphqlStyle::Idiomatic, true),
    ] {
        let groups = if custom {
            BTreeMap::from([(
                "users".into(),
                BTreeMap::from([("read".into(), "ReadUser".into())]),
            )])
        } else {
            BTreeMap::new()
        };
        let mut groups = groups;
        if custom {
            groups.insert(
                "actions".into(),
                BTreeMap::from([("ping".into(), "Ping".into())]),
            );
        }
        let mut input = fixture();
        input.operations.push(GraphqlOperation {
            name: "Ping".into(),
            kind: GraphqlOperationKind::Mutation,
            document: "mutation Ping { ping }".into(),
            variables: vec![],
            result: ModelType {
                nullable: false,
                kind: ModelKind::Object(vec![field("ping", scalar("Boolean", false), false)]),
            },
        });
        let dir = tempfile::tempdir().unwrap();
        let (tree, _) = render(&input, "io.test", style, &groups).unwrap();
        for (p, s) in tree.iter() {
            let p = dir.path().join(p);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, s).unwrap();
        }
        let invoke = match style {
            GraphqlStyle::Raw => "Client.readUser(client,variables)",
            GraphqlStyle::Flat => "client.readUser(variables)",
            GraphqlStyle::Idiomatic if custom => "client.users().read(variables)",
            GraphqlStyle::Idiomatic => "client.query().readUser(variables)",
        };
        let ping = match style {
            GraphqlStyle::Raw => "Client.ping(client)",
            GraphqlStyle::Flat => "client.ping()",
            GraphqlStyle::Idiomatic if custom => "client.actions().ping()",
            GraphqlStyle::Idiomatic => "client.mutation().ping()",
        };
        let test = format!(
            r#"import io.test.Client;public class Test {{public static void main(String[] args)throws Exception {{var client=new Client("http://127.0.0.1:{}");var variables=new Client.ReadUserVariables("1");var result={invoke};if(!result.requireData().readUser().name().equals("Ada")||!result.data().readUser().nickname().present()||result.data().readUser().nickname().value()!=null)throw new AssertionError();variables=new Client.ReadUserVariables("partial");result={invoke};if(!result.hasErrors()||!result.dataPresent()||!result.data().readUser().name().equals("Ada")||!result.errors().get(0).message().equals("nickname failed")||result.errors().get(0).path()==null)throw new AssertionError();try{{result.requireData();throw new AssertionError();}}catch(Client.GraphqlException expected){{}} if(!{ping}.requireData().ping())throw new AssertionError(); }} }}"#,
            port.trim()
        );
        fs::write(dir.path().join("Test.java"), test).unwrap();
        let output = Command::new("javac")
            .args([
                "-cp",
                &classpath(),
                "-d",
                "classes",
                "src/main/java/io/test/Client.java",
                "Test.java",
            ])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new("java")
            .args(["-cp", &format!("classes:{}", classpath()), "Test"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
#[ignore = "requires JDK 17+ and pinned Jackson 2.18.3 jars in Maven cache"]
fn complex_selections_compile_and_decode() {
    let mut input = fixture();
    input.input_objects.insert(
        "Filter".into(),
        vec![field("name", scalar("String", true), true)],
    );
    input.operations[0].variables.push(field(
        "filter",
        ModelType {
            nullable: true,
            kind: ModelKind::Named("Filter".into()),
        },
        true,
    ));
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
    input.operations[0].result = ModelType {
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
    };
    let dir = tempfile::tempdir().unwrap();
    let (tree, _) = render(&input, "io.test", GraphqlStyle::Flat, &BTreeMap::new()).unwrap();
    for (p, s) in tree.iter() {
        let p = dir.path().join(p);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, s).unwrap();
    }
    fs::write(dir.path().join("Test.java"),r#"import io.test.Client;import com.fasterxml.jackson.databind.ObjectMapper;public class Test {public static void main(String[]args)throws Exception {var vars=new Client.ReadUserVariables("1",Client.Field.of(new Client.Filter(Client.Field.absent())));if(vars.toJson().get("filter").has("name"))throw new AssertionError();var absent=new Client.ReadUserVariables("1",Client.Field.absent());if(absent.toJson().has("filter"))throw new AssertionError();var explicit=new Client.ReadUserVariables("1",Client.Field.of(null));if(!explicit.toJson().get("filter").isNull())throw new AssertionError();var result=Client.ReadUserResult.fromJson(new ObjectMapper().readTree("{\"nodes\":[{\"__typename\":\"User\",\"name\":\"Ada\"},null,{\"__typename\":\"Team\",\"name\":\"Staff\"}],\"matrix\":[[\"a\",null],null]}"));if(!(result.nodes().get(0) instanceof Client.ReadUserResultNodesVariant0 user)||!user.name().equals("Ada")||result.nodes().get(1)!=null||result.matrix().get(0).get(1)!=null)throw new AssertionError();}}"#).unwrap();
    let output = Command::new("javac")
        .args([
            "-cp",
            &classpath(),
            "-d",
            "classes",
            "src/main/java/io/test/Client.java",
            "Test.java",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new("java")
        .args(["-cp", &format!("classes:{}", classpath()), "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn selection_models_and_graph_are_native() {
    let (tree, methods) = render(
        &fixture(),
        "io.test",
        GraphqlStyle::Idiomatic,
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(methods["ReadUser"], "readUser");
    let client = tree.get("src/main/java/io/test/Client.java").unwrap();
    assert!(client.contains("ReadUserVariables(String id)"));
    assert!(client.contains("Field<String> nickname"));
    assert!(client.contains("public QueryGroup query()"));
    assert!(client.contains("operationName"));
}
struct Source {
    meta: Meta,
    name: &'static str,
}
impl Plugin<Java> for Source {
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
    fn generate(&self, cx: &mut PluginContext<'_, Java>) -> Result<()> {
        let mut input = fixture();
        input.operations[0].name = self.name.into();
        cx.publish(input)
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
        let source = tree.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
        assert!(source.contains(&format!("record {name}Variables")));
        assert!(!source.contains("record IgnoredVariables"));
    }
}
