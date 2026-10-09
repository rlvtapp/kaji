use super::*;
use poolster_core::native::{GraphqlOperation, ModelField};
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn contract() -> GraphqlOperations {
    let scalar = ModelType {
        nullable: false,
        kind: ModelKind::Scalar("String".into()),
    };
    GraphqlOperations {
        schema_source: String::new(),
        operation_source: String::new(),
        input_objects: BTreeMap::new(),
        operations: vec![GraphqlOperation {
            name: "ReadUser".into(),
            kind: GraphqlOperationKind::Query,
            document: "query ReadUser($id: ID) { hello(id:$id) other }".into(),
            variables: vec![ModelField {
                name: "id".into(),
                ty: ModelType {
                    nullable: true,
                    kind: ModelKind::Scalar("ID".into()),
                },
                optional: true,
                default_value: None,
            }],
            result: ModelType {
                nullable: false,
                kind: ModelKind::Object(vec![
                    ModelField {
                        name: "hello".into(),
                        ty: ModelType {
                            nullable: true,
                            kind: ModelKind::Scalar("String".into()),
                        },
                        optional: false,
                        default_value: None,
                    },
                    ModelField {
                        name: "other".into(),
                        ty: scalar,
                        optional: false,
                        default_value: None,
                    },
                ]),
            },
        }],
    }
}
#[test]
fn ruby_graphql_styles_are_selection_specific_and_reject_collisions() {
    for style in [
        GraphqlStyle::Raw,
        GraphqlStyle::Flat,
        GraphqlStyle::Idiomatic,
    ] {
        let (source, methods) =
            render(&contract(), "example_graphql", style, &BTreeMap::new()).unwrap();
        assert!(
            source
                .iter()
                .any(|(_, code)| code.contains("class ReadUserVariables"))
        );
        assert!(signature(&source).contains("class ReadUserResult"));
        assert_eq!(methods["ReadUser"], "read_user");
        let dir = tempfile::tempdir().unwrap();
        source.write_to(dir.path()).unwrap();
        let file = dir.path().join("lib/example_graphql.rb");
        let output =
            Command::new(std::env::var("POOLSTER_RUBY_BINARY").unwrap_or_else(|_| "ruby".into()))
                .args(["-c", file.to_str().unwrap()])
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut invalid = contract();
    invalid.operations.push(invalid.operations[0].clone());
    assert!(render(&invalid, "example", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    invalid.operations[0].kind = GraphqlOperationKind::Subscription;
    assert!(render(&invalid, "example", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    for group in ["1users", "def", "class"] {
        let groups = BTreeMap::from([(
            group.into(),
            BTreeMap::from([("read".into(), "ReadUser".into())]),
        )]);
        assert!(render(&contract(), "example", GraphqlStyle::Idiomatic, &groups).is_err());
    }
    let groups = BTreeMap::from([(
        "users".into(),
        BTreeMap::from([("read".into(), "Missing".into())]),
    )]);
    assert!(render(&contract(), "example", GraphqlStyle::Idiomatic, &groups).is_err());
    assert!(render(&contract(), "example", GraphqlStyle::Flat, &groups).is_err());
    let mut bad = contract();
    bad.input_objects.insert("String".into(), vec![]);
    assert!(render(&bad, "example", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    assert!(render(&contract(), "string", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
}
#[test]
#[ignore = "requires pinned GraphQL.js16.14.2 modules and a local HTTP server"]
fn ruby_graphql_executes_all_styles_against_pinned_server() {
    let modules = std::env::var("POOLSTER_GRAPHQL_JS_ROOT").expect("set GraphQL modules root");
    let node = std::env::var("POOLSTER_NODE_BIN").unwrap_or_else(|_| "node".into());
    let root = tempfile::tempdir().unwrap();
    let portfile = root.path().join("port");
    let serverfile = root.path().join("server.cjs");
    std::fs::write(&serverfile,r#"const fs=require('node:fs'),http=require('node:http');const g=require(process.argv[2]+'/graphql');if(require(process.argv[2]+'/graphql/package.json').version!=='16.14.2')throw Error('unpinned GraphQL');const schema=g.buildSchema('type Query {hello(id:ID):String other:String!}');http.createServer(async(req,res)=>{let b='';for await(const x of req)b+=x;const r=JSON.parse(b);if(req.headers['x-probe']) {const mode=req.headers['x-probe'];res.statusCode=mode==='http'?503:200;res.end(({missing:'{}',errors:'{"errors":null}',json:'[',http:'{"errors":[{"message":"unavailable"}]}'}[mode]));return;}const result=await g.graphql({schema,source:r.query,operationName:r.operationName,variableValues:r.variables,rootValue:{hello:({id})=>{if(id==='partial')throw Error('resolver failed');return id===null?null:'hello'},other:()=> 'ok'}});res.setHeader('content-type','application/json');res.end(JSON.stringify(result))}).listen(0,'0.0.0.0',function(){fs.writeFileSync(process.argv[3],String(this.address().port))});"#).unwrap();
    let mut server = Command::new(node)
        .arg(&serverfile)
        .arg(&modules)
        .arg(&portfile)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while !portfile.exists() {
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(server.try_wait().unwrap().is_none(), "server exited");
        std::thread::sleep(Duration::from_millis(20));
    }
    let port = std::fs::read_to_string(&portfile).unwrap();
    let host = std::env::var("POOLSTER_GRAPHQL_SERVER_HOST").unwrap_or("127.0.0.1".into());
    for (index, style) in [
        GraphqlStyle::Raw,
        GraphqlStyle::Flat,
        GraphqlStyle::Idiomatic,
        GraphqlStyle::Idiomatic,
    ]
    .into_iter()
    .enumerate()
    {
        let groups = if index == 3 {
            BTreeMap::from([(
                "users".into(),
                BTreeMap::from([("read".into(), "ReadUser".into())]),
            )])
        } else {
            BTreeMap::new()
        };
        let (source, _) = render(&contract(), "example_graphql", style, &groups).unwrap();
        let package = root.path().join(format!("package{index}"));
        source.write_to(&package).unwrap();
        for (path, _) in source
            .iter()
            .filter(|(path, _)| path.to_string_lossy().ends_with(".rbs"))
        {
            assert_rbs(&package.join(path));
        }
        let file = package.join("lib/example_graphql.rb");
        let call = match index {
            0 => "ExampleGraphql.read_user(client.transport, vars)",
            1 => "client.read_user(vars)",
            2 => "client.query.read_user(vars)",
            _ => "client.users.read(vars)",
        };
        let script = format!(
            r#"require ARGV[0]
client=ExampleGraphql::Client.new('http://{host}:{port}/graphql', timeout: 5)
vars={{}}
response={call}
raise 'success' unless response.status==:success && response.data.hello=='hello' && response.data.other=='ok'
vars={{'id'=>nil}}
raise 'null' unless ({call}).data.hello.nil?
vars={{'id'=>'partial'}}
response={call}
raise 'partial' unless response.status==:partial && response.data.other=='ok' && response.errors.first['path']==['hello']
begin; response.require_data; raise 'must raise'; rescue ExampleGraphql::GraphqlErrors => error; raise 'data lost' unless error.data.other=='ok'; end
%w[missing errors json http].each do |mode|
  broken=ExampleGraphql::Client.new('http://{host}:{port}/graphql', headers: {{'x-probe'=>mode}})
  begin; ExampleGraphql.read_user(broken.transport); raise 'malformed accepted'; rescue ArgumentError, JSON::ParserError, ExampleGraphql::HttpError; end
end
raise 'presence' if ExampleGraphql::ReadUserVariables.new.present?('id')
raise 'null presence' unless ExampleGraphql::ReadUserVariables.new('id'=>nil).present?('id')
begin; ExampleGraphql::ReadUserResult.new('hello'=>nil); raise 'missing required accepted'; rescue ArgumentError; end
"#
        );
        let result =
            Command::new(std::env::var("POOLSTER_RUBY_BINARY").unwrap_or_else(|_| "ruby".into()))
                .args(["-e", &script, file.to_str().unwrap()])
                .output()
                .unwrap();
        if !result.status.success() {
            let _ = server.kill();
            panic!("{}", String::from_utf8_lossy(&result.stderr));
        }
    }
    server.kill().unwrap();
    let _ = server.wait();
}

fn assert_rbs(file: &std::path::Path) {
    let output = Command::new(std::env::var("POOLSTER_RUBY_BINARY").unwrap_or("ruby".into()))
        .args([
            "-rrbs",
            "-e",
            "RBS::Parser.parse_signature(File.read(ARGV[0]))",
            file.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "RBS parsing failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
#[ignore = "requires Ruby3.1+ and RBS for typed generated model execution"]
fn ruby_nested_named_list_union_models_and_signatures() {
    let mut c = contract();
    let scalar = ModelType {
        nullable: false,
        kind: ModelKind::Scalar("String".into()),
    };
    let field = |name: &str, ty: ModelType, optional: bool| ModelField {
        name: name.into(),
        ty,
        optional,
        default_value: None,
    };
    c.input_objects.insert(
        "Filter".into(),
        vec![
            field(
                "label",
                ModelType {
                    nullable: true,
                    kind: ModelKind::Scalar("String".into()),
                },
                true,
            ),
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
            field(key, scalar.clone(), false),
        ]),
    };
    c.operations[0].result = ModelType {
        nullable: false,
        kind: ModelKind::Object(vec![
            field(
                "user",
                ModelType {
                    nullable: false,
                    kind: ModelKind::Object(vec![
                        field("name", scalar.clone(), false),
                        field(
                            "nickname",
                            ModelType {
                                nullable: true,
                                kind: ModelKind::Scalar("String".into()),
                            },
                            true,
                        ),
                    ]),
                },
                false,
            ),
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
        ]),
    };
    let mut second = c.operations[0].clone();
    second.name = "ReadAgain".into();
    c.operations.push(second);
    let (source, _) = render(
        &c,
        "example_graphql",
        GraphqlStyle::Idiomatic,
        &BTreeMap::new(),
    )
    .unwrap();
    let signature = signature(&source);
    assert!(signature.contains("def user: () -> ReadUserResultUser"));
    assert!(signature.contains("def filter: () -> ((Filter)?)?"));
    assert!(signature.contains("def read_user:"));
    let root = tempfile::tempdir().unwrap();
    source.write_to(root.path()).unwrap();
    let file = root.path().join("lib/example_graphql.rb");
    for (path, _) in source
        .iter()
        .filter(|(path, _)| path.to_string_lossy().ends_with(".rbs"))
    {
        assert_rbs(&root.path().join(path));
    }
    let script = r#"require ARGV[0]
f=ExampleGraphql::Filter.new(label:nil,next:{})
v=ExampleGraphql::ReadUserVariables.new(filter:f)
raise 'named model' unless v.filter.is_a?(ExampleGraphql::Filter) && v.filter.next.is_a?(ExampleGraphql::Filter)
raise 'wire' unless v.to_h=={'filter'=>{'label'=>nil,'next'=>{}}}
value={'user'=>{'name'=>'Ada'},'items'=>[{'__typename'=>'User','name'=>'Ada'},{'__typename'=>'Team','title'=>'Core'},nil]}
a=ExampleGraphql::ReadUserResult.new(value);b=ExampleGraphql::ReadAgainResult.new(value)
raise 'selection models' unless a.user.is_a?(ExampleGraphql::ReadUserResultUser)&&b.user.is_a?(ExampleGraphql::ReadAgainResultUser)
raise 'nested getters' unless a.user.name=='Ada'&&!a.user.present?('nickname')&&a.items[0].name=='Ada'&&a.items[1].title=='Core'&&a.items[2].nil?
raise 'nested serialization' unless a.to_h==value
"#;
    let out = Command::new(std::env::var("POOLSTER_RUBY_BINARY").unwrap_or("ruby".into()))
        .args(["-e", script, file.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
struct Source {
    meta: Meta,
    name: &'static str,
}
impl Plugin<Ruby> for Source {
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
    fn generate(&self, cx: &mut PluginContext<'_, Ruby>) -> Result<()> {
        let mut c = contract();
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

fn signature(tree: &poolster_core::GeneratedTree) -> String {
    tree.iter()
        .filter(|(path, _)| path.to_string_lossy().ends_with(".rbs"))
        .map(|(_, source)| source)
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
#[ignore = "requires Ruby3.1+ to execute modular large GraphQL packages"]
fn large_packages_use_bounded_files_and_regenerate() {
    let mut c = contract();
    let op = c.operations[0].clone();
    c.operations = (0..300)
        .map(|i| {
            let mut op = op.clone();
            op.name = format!("Operation{i:03}");
            op.document = op.document.replace("ReadUser", &op.name);
            op
        })
        .collect();
    for style in [
        GraphqlStyle::Raw,
        GraphqlStyle::Flat,
        GraphqlStyle::Idiomatic,
    ] {
        let (tree, _) = render(&c, "example_graphql", style, &BTreeMap::new()).unwrap();
        assert_eq!(
            tree.iter()
                .filter(|(p, _)| p.starts_with("lib/example_graphql/operations"))
                .count(),
            300
        );
        for (path, code) in tree.iter() {
            assert!(
                code.len() < 16384,
                "{} too large: {}",
                path.display(),
                code.len()
            );
        }
        let mut reversed = c.clone();
        reversed.operations.reverse();
        let (again, _) = render(&reversed, "example_graphql", style, &BTreeMap::new()).unwrap();
        assert_eq!(
            tree.iter().collect::<Vec<_>>(),
            again.iter().collect::<Vec<_>>()
        );
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let changes = again.check(dir.path()).unwrap();
        assert!(
            changes.added.is_empty() && changes.modified.is_empty() && changes.removed.is_empty()
        );
        let call = match style {
            GraphqlStyle::Raw => "ExampleGraphql.operation299(client.transport)",
            GraphqlStyle::Flat => "client.operation299",
            GraphqlStyle::Idiomatic => "client.query.operation299",
        };
        let script = format!(
            "require ARGV[0]\nclient=ExampleGraphql::Client.new('http://localhost');transport=Object.new;def transport.execute(document,name,variables,result);ExampleGraphql::GraphqlResponse.new(data:result.new('hello'=>nil,'other'=>'ok'),errors:[],data_present:true);end;client.instance_variable_set(:@transport,transport);r={call};raise 'wrong result' unless r.data.is_a?(ExampleGraphql::Operation299Result)\n"
        );
        let out = Command::new(std::env::var("POOLSTER_RUBY_BINARY").unwrap_or("ruby".into()))
            .args([
                "-e",
                &script,
                dir.path().join("lib/example_graphql.rb").to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let mut removed = c.clone();
        removed.operations.pop();
        let (reduced, _) = render(&removed, "example_graphql", style, &BTreeMap::new()).unwrap();
        assert!(!reduced.check(dir.path()).unwrap().removed.is_empty());
        reduced.write_to(dir.path()).unwrap();
        assert!(
            !dir.path()
                .join("lib/example_graphql/operations/operation299.rb")
                .exists()
        );
    }
}
