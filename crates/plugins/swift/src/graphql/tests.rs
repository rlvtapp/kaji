use super::*;
use poolster_core::native::{GraphqlOperation, ModelField, ModelKind, ModelType};
use std::{
    fs,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};
fn string(nullable: bool) -> ModelType {
    ModelType {
        nullable,
        kind: ModelKind::Scalar("String".into()),
    }
}
fn fixture() -> GraphqlOperations {
    GraphqlOperations{schema_source:"type Query { readUser(id: ID!, nickname: String): User } type User { name: String! nickname: String }".into(),operation_source:"query ReadUser($id: ID!, $nickname: String) { readUser(id: $id, nickname: $nickname) { name nickname } }".into(),input_objects:BTreeMap::new(),operations:vec![GraphqlOperation{name:"ReadUser".into(),kind:GraphqlOperationKind::Query,document:"query ReadUser($id: ID!, $nickname: String) { readUser(id: $id, nickname: $nickname) { name nickname } }".into(),variables:vec![ModelField{name:"id".into(),ty:string(false),optional:false,default_value:None},ModelField{name:"nickname".into(),ty:string(true),optional:true,default_value:None}],result:ModelType{nullable:false,kind:ModelKind::Object(vec![ModelField{name:"readUser".into(),optional:false,default_value:None,ty:ModelType{nullable:true,kind:ModelKind::Object(vec![ModelField{name:"name".into(),ty:string(false),optional:false,default_value:None},ModelField{name:"nickname".into(),ty:string(true),optional:true,default_value:None}])}}])}}]}
}
#[test]
fn validates_contract_and_configuration() {
    for style in [
        GraphqlStyle::Raw,
        GraphqlStyle::Flat,
        GraphqlStyle::Idiomatic,
    ] {
        let (files, _) = render(&fixture(), style, &BTreeMap::new()).unwrap();
        let source = files.values().cloned().collect::<Vec<_>>().join("\n");
        assert!(source.contains("GraphqlField<String>"));
        assert!(source.contains("URLSession"));
    }
    let mut c = fixture();
    c.operations[0].kind = GraphqlOperationKind::Subscription;
    assert!(render(&c, GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    assert!(
        render(
            &fixture(),
            GraphqlStyle::Idiomatic,
            &BTreeMap::from([(
                "users".into(),
                BTreeMap::from([("read".into(), "missing".into())])
            )])
        )
        .is_err()
    );
    assert_eq!(literal("\u{1}\\\"\n"), "\"\\u{1}\\\\\\\"\\n\"");
}
#[test]
#[ignore = "requires Swift 5.9+, pinned GraphQL.js 16.14.2 and loopback sockets"]
fn generated_clients_compile_and_execute_against_graphql_server() {
    let root = std::env::var("POOLSTER_GRAPHQL_JS_ROOT").expect("GraphQL.js root");
    let mut input = fixture();
    input.operations.push(abstract_operation());
    input.operations.push(GraphqlOperation {
        name: "Ping".into(),
        kind: GraphqlOperationKind::Mutation,
        document: "mutation Ping { ping }".into(),
        variables: vec![],
        result: ModelType {
            nullable: false,
            kind: ModelKind::Object(vec![ModelField {
                name: "ping".into(),
                ty: string(false),
                optional: false,
                default_value: None,
            }]),
        },
    });
    input.input_objects.insert(
        "StrictInput".into(),
        vec![ModelField {
            name: "count".into(),
            optional: true,
            default_value: Some("1".into()),
            ty: ModelType {
                nullable: false,
                kind: ModelKind::Scalar("Int".into()),
            },
        }],
    );
    let server = r#"
const {graphql,buildSchema,version}=require(process.env.POOLSTER_GRAPHQL_JS_ROOT+'/graphql');if(version!=='16.14.2')throw Error('expected pinned GraphQL.js');
const schema=buildSchema('type Query { readUser(id: ID!, nickname: String): User node:Node! } type User { name: String! nickname: String } type Mutation { ping: String! } interface Node{id:ID!} type Person implements Node{id:ID! name:String!} type Robot implements Node{id:ID! code:String!}');
const server=require('http').createServer(async(req,res)=>{let body='';for await(const chunk of req)body+=chunk;const v=JSON.parse(body);
if(req.url==='/slow'){await new Promise(resolve=>setTimeout(resolve,300));}if(req.url==='/http'){res.writeHead(503);res.end('unavailable');return;}if(req.url==='/malformed'){res.end('invalid');return;}
const result=await graphql({schema,source:v.query,operationName:v.operationName,variableValues:v.variables,rootValue:{node:()=>({__typename:'Robot',id:'r',code:'R2'}),ping:()=> 'pong',readUser:()=>({name:v.variables.id,nickname:()=>{if(v.variables.id==='partial')throw Error('partial failure');return v.variables.nickname??null;}})}});
res.setHeader('content-type','application/json');res.end(JSON.stringify(result));});server.listen(0,'127.0.0.1',()=>console.log(server.address().port));
"#;
    struct Server(std::process::Child);
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut server = Server(
        Command::new("node")
            .args(["-e", server])
            .env("POOLSTER_GRAPHQL_JS_ROOT", root)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut port = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut port)
        .unwrap();
    assert!(!port.trim().is_empty());
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
                BTreeMap::from([
                    ("read".into(), "ReadUser".into()),
                    ("ping".into(), "Ping".into()),
                ]),
            )])
        } else {
            BTreeMap::new()
        };
        let mut generated = Vec::new();
        for (path, source) in render(&input, style, &groups).unwrap().0 {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            if path.extension().and_then(|s| s.to_str()) == Some("swift") {
                generated.push(path.clone());
            }
            fs::write(path, source).unwrap();
        }
        let call = match style {
            GraphqlStyle::Raw => "readUser(client:client,variables:variables)",
            GraphqlStyle::Flat => "client.readUser(variables:variables)",
            GraphqlStyle::Idiomatic if custom => "client.users.read(variables:variables)",
            GraphqlStyle::Idiomatic => "client.query.readUser(variables:variables)",
        };
        let ping = match style {
            GraphqlStyle::Raw => "ping(client:client)",
            GraphqlStyle::Flat => "client.ping()",
            GraphqlStyle::Idiomatic if custom => "client.users.ping()",
            GraphqlStyle::Idiomatic => "client.mutation.ping()",
        };
        fs::write(
            dir.path().join("Program.swift"),
            include_str!("test.swift.tmpl")
                .replace("PING_CALL", ping)
                .replace("CALL", call)
                .replace("ENDPOINT", &format!("http://127.0.0.1:{}", port.trim())),
        )
        .unwrap();
        let result = Command::new("swiftc")
            .args([
                "-swift-version",
                "5",
                "-warnings-as-errors",
                "-parse-as-library",
                "Program.swift",
                "-o",
                "test-client",
            ])
            .args(&generated)
            .env("CLANG_MODULE_CACHE_PATH", dir.path().join("cache"))
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let result = Command::new(dir.path().join("test-client"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
struct Source {
    meta: Meta,
    name: &'static str,
}
impl Plugin<crate::Swift> for Source {
    fn kind(&self) -> &'static str {
        "test-input"
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
    fn generate(&self, cx: &mut PluginContext<'_, crate::Swift>) -> Result<()> {
        let mut c = fixture();
        c.operations[0].name = self.name.into();
        cx.publish(c)
    }
}
#[test]
fn provider_substitution_and_regeneration() {
    for name in ["ReadUser", "Renamed"] {
        let input = Source {
            meta: Meta::new(),
            name,
        };
        let other = Source {
            meta: Meta::new(),
            name: "Ignored",
        };
        let plugin = graphql(Some(input.meta.handle()));
        let tree = poolster_core::engine::Packages::new()
            .package(crate::package("sdk").with(other).with(plugin).with(input))
            .generate_native()
            .unwrap();
        let source = tree.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
        assert!(source.contains(&member(name)));
        assert!(!source.contains("func `ignored`"));
        assert!(source.contains("swift-tools-version: 5.9"));
    }
}
#[test]
fn many_operations_have_stable_bounded_files_and_migrate_owned_monolith() {
    let mut input = fixture();
    let operation = input.operations[0].clone();
    input.operations = (0..1000)
        .map(|i| {
            let mut op = operation.clone();
            op.name = format!("Operation{i:04}");
            op.document = op.document.replace("ReadUser", &op.name);
            op
        })
        .collect();
    for style in [
        GraphqlStyle::Raw,
        GraphqlStyle::Flat,
        GraphqlStyle::Idiomatic,
    ] {
        let (files, _) = render(&input, style, &BTreeMap::new()).unwrap();
        assert_eq!(
            files
                .keys()
                .filter(|p| p.starts_with("Operations/"))
                .count(),
            1000
        );
        assert!(files.keys().any(|p| p.starts_with("Models/")));
        assert!(files.contains_key("Client/GraphqlClient.swift"));
        assert!(!files.contains_key("Graphql.swift"));
        assert!(files.values().all(|s| s.len() <= 128 * 1024));
        let mut reversed = input.clone();
        reversed.operations.reverse();
        assert_eq!(files, render(&reversed, style, &BTreeMap::new()).unwrap().0);
    }
    let dir = tempfile::tempdir().unwrap();
    let mut previous = poolster_core::GeneratedTree::default();
    previous
        .insert(
            GeneratedFile::new(
                "Sources/GraphqlSdk/Graphql.swift",
                "// previous owned monolith",
            )
            .unwrap(),
        )
        .unwrap();
    previous.write_to(dir.path()).unwrap();
    fs::write(dir.path().join("custom.swift"), "// user code").unwrap();
    let mut tree = poolster_core::GeneratedTree::default();
    for (path, source) in render(&fixture(), GraphqlStyle::Flat, &BTreeMap::new())
        .unwrap()
        .0
    {
        tree.insert(GeneratedFile::new(format!("Sources/GraphqlSdk/{path}"), source).unwrap())
            .unwrap();
    }
    tree.write_to(dir.path()).unwrap();
    assert!(!dir.path().join("Sources/GraphqlSdk/Graphql.swift").exists());
    assert!(dir.path().join("custom.swift").exists());
    assert!(tree.check(dir.path()).unwrap().is_empty());
}
#[test]
fn oversized_atomic_swift_models_have_explicit_layout_diagnostics() {
    let mut input = fixture();
    input.input_objects.insert(
        "LargeInput".into(),
        (0..1500)
            .map(|i| ModelField {
                name: format!("property{i:04}"),
                ty: string(true),
                optional: true,
                default_value: None,
            })
            .collect(),
    );
    let files = render(&input, GraphqlStyle::Flat, &BTreeMap::new())
        .unwrap()
        .0;
    let diagnostics: serde_json::Value =
        serde_json::from_str(&files[".poolster/source-layout-diagnostics.json"]).unwrap();
    assert!(diagnostics.as_array().unwrap().iter().any(|d| {
        d["path"]
            .as_str()
            .unwrap()
            .starts_with("Models/ModelLargeInput_")
    }));
}
#[test]
fn group_method_filenames_encode_tuple_identity() {
    let mut input = fixture();
    let mut other = input.operations[0].clone();
    other.name = "ReadOther".into();
    input.operations.push(other);
    let groups = BTreeMap::from([
        (
            "a".into(),
            BTreeMap::from([("bMethodC".into(), "ReadUser".into())]),
        ),
        (
            "aMethodB".into(),
            BTreeMap::from([("c".into(), "ReadOther".into())]),
        ),
    ]);
    let files = render(&input, GraphqlStyle::Idiomatic, &groups).unwrap().0;
    assert_eq!(
        files
            .keys()
            .filter(|p| p.starts_with("Groups/") && !p.contains("Group_"))
            .count(),
        2
    );
    assert_eq!(files.keys().filter(|p| p.starts_with("Groups/")).count(), 4);
}

fn abstract_operation() -> GraphqlOperation {
    let alternatives = [("Person", "name"), ("Robot", "code")]
        .into_iter()
        .map(|(name, field)| ModelType {
            nullable: false,
            kind: ModelKind::Object(vec![
                ModelField {
                    name: "kind".into(),
                    optional: false,
                    default_value: None,
                    ty: ModelType {
                        nullable: false,
                        kind: ModelKind::Literal(name.into()),
                    },
                },
                ModelField {
                    name: field.into(),
                    optional: false,
                    default_value: None,
                    ty: string(false),
                },
            ]),
        })
        .collect();
    GraphqlOperation {
        name: "Search".into(),
        kind: GraphqlOperationKind::Query,
        document:
            "query Search { node { kind:__typename ... on Person { name } ... on Robot { code } } }"
                .into(),
        variables: vec![],
        result: ModelType {
            nullable: false,
            kind: ModelKind::Object(vec![ModelField {
                name: "node".into(),
                optional: false,
                default_value: None,
                ty: ModelType {
                    nullable: false,
                    kind: ModelKind::Union(alternatives),
                },
            }]),
        },
    }
}
#[test]
fn abstract_selections_require_a_stable_typename_discriminator() {
    let mut models = models::Models::default();
    let operation = abstract_operation();
    let ModelKind::Object(fields) = &operation.result.kind else {
        panic!()
    };
    let mut ty = fields[0].ty.clone();
    let ModelKind::Union(alternatives) = &mut ty.kind else {
        panic!()
    };
    for alternative in alternatives.iter_mut() {
        let ModelKind::Object(fields) = &mut alternative.kind else {
            panic!()
        };
        fields.remove(0);
    }
    assert!(
        models
            .ty("Untagged", &ty)
            .unwrap_err()
            .to_string()
            .contains("__typename")
    );
}

#[test]
#[ignore = "requires Swift5.9+, GraphQL.js16.14.2 local SSE/multipart server"]
fn advanced_transports_compile_and_execute() {
    for incremental in [false, true] {
        for style in [
            GraphqlStyle::Raw,
            GraphqlStyle::Flat,
            GraphqlStyle::Idiomatic,
        ] {
            let mut c = fixture();
            c.operations[0].variables[0].ty.kind = ModelKind::Scalar("ID".into());
            c.operations[0].variables.truncate(1);
            c.operations[0].kind = if incremental {
                GraphqlOperationKind::Query
            } else {
                GraphqlOperationKind::Subscription
            };
            c.operations[0].document = if incremental {
                "query ReadUser($id:ID!){readUser(id:$id){... @defer {name nickname}}}"
            } else {
                "subscription ReadUser($id:ID!){readUser(id:$id){name nickname}}"
            }
            .into();
            let ModelKind::Object(fields) = &mut c.operations[0].result.kind else {
                panic!()
            };
            let ModelKind::Object(fields) = &mut fields[0].ty.kind else {
                panic!()
            };
            fields[0].ty.kind = ModelKind::Scalar("Date".into());
            let dir = tempfile::tempdir().unwrap();
            let mut generated = Vec::new();
            for (path, source) in
                render_advanced(&c, style, &BTreeMap::new(), !incremental, incremental)
                    .unwrap()
                    .0
            {
                if !path.ends_with(".swift") {
                    continue;
                }
                let path = dir.path().join(path);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, source).unwrap();
                generated.push(path);
            }
            let call = match style {
                GraphqlStyle::Raw => "readUser(client:client,variables:variables)",
                GraphqlStyle::Flat => "client.readUser(variables:variables)",
                GraphqlStyle::Idiomatic => {
                    if incremental {
                        "client.query.readUser(variables:variables)"
                    } else {
                        "client.subscription.readUser(variables:variables)"
                    }
                }
            };
            let check=if incremental{"var frames:[GraphqlIncrementalSnapshot<ReadUserResult>]=[];for try await item in try await CALL {frames.append(item)};guard frames.count==2,!frames[0].complete,frames[1].complete else {fatalError(\"incremental frames\")};guard case .object(let first)?=frames[0].data,case .object(let user)?=first[\"readUser\"],user[\"name\"]==nil else{fatalError(\"snapshot mutated\")};guard case .string(let name) = try frames[1].decodeData().readUser!.name,name==\"decoded:Ada\" else{fatalError(\"scalar decode\")}"}else{"var frames:[GraphqlResponse<ReadUserResult>]=[];for try await item in try await CALL{frames.append(item)};guard frames.count==2,frames[1].errors?.count==1,case .string(let name)=frames[0].data!.readUser!.name,name==\"decoded:Ada\" else{fatalError(\"SSE scalar envelope\")}"}.replace("CALL",call);
            let branches = r#"if CommandLine.arguments.count>2 && CommandLine.arguments[2]=="cancel" {let task=Task {for try await _ in try await CALL{}};try await Task.sleep(nanoseconds:30_000_000);task.cancel();_ = try? await task.value;return} else if CommandLine.arguments.count>2 && CommandLine.arguments[2]=="json" {var frames:[GraphqlIncrementalSnapshot<ReadUserResult>]=[];for try await item in try await CALL{frames.append(item)};guard frames.count==1,frames[0].complete,case .string(let name)=try frames[0].decodeData().readUser!.name,name=="decoded:Ada" else{fatalError("JSON fallback")};return} else if CommandLine.arguments.count>2 {var failed=false;do {for try await _ in try await CALL{}}catch{failed=true};guard failed else{fatalError("Expected malformed stream failure")};return}"#;
            let branches = if incremental {
                branches.to_owned()
            } else {
                branches.replace(" else if CommandLine.arguments.count>2 && CommandLine.arguments[2]==\"json\" {var frames:[GraphqlIncrementalSnapshot<ReadUserResult>]=[];for try await item in try await CALL{frames.append(item)};guard frames.count==1,frames[0].complete,case .string(let name)=try frames[0].decodeData().readUser!.name,name==\"decoded:Ada\" else{fatalError(\"JSON fallback\")};return}", "")
            };
            let check = format!("{} else {{{check}}}", branches.replace("CALL", call));
            fs::write(dir.path().join("Program.swift"),format!("import Foundation\n@main struct Main {{static func main()async throws{{let codecs=[\"ID\":GraphqlScalarCodec(encode:{{value in guard case .string(let text)=value else{{throw URLError(.badServerResponse)}};return .string(text+\"!\")}},decode:{{$0}}),\"Date\":GraphqlScalarCodec(encode:{{$0}},decode:{{value in guard case .string(let text)=value else{{throw URLError(.badServerResponse)}};return .string(\"decoded:\"+text)}})];let client=GraphqlClient(endpoint:URL(string:CommandLine.arguments[1])!,scalarCodecs:codecs);let variables=ReadUserVariables(id:\"1\");{check}}}}}")).unwrap();
            let output = Command::new("swiftc")
                .args([
                    "-swift-version",
                    "5",
                    "-warnings-as-errors",
                    "-parse-as-library",
                    "Program.swift",
                    "-o",
                    "test-client",
                ])
                .args(generated)
                .env("CLANG_MODULE_CACHE_PATH", dir.path().join("cache"))
                .current_dir(dir.path())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let output = Command::new(std::env::var("POOLSTER_TEST_NODE").unwrap_or("node".into()))
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/graphql/advanced-test.cjs"
                ))
                .arg(dir.path())
                .arg(dir.path().join("test-client"))
                .arg(if incremental { "incremental" } else { "sse" })
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
