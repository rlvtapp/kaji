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
        let (files, _) = render(&fixture(), "TestSdk", style, &BTreeMap::new()).unwrap();
        let source = files.values().cloned().collect::<Vec<_>>().join("\n");
        assert!(source.contains("Optional<string?> Nickname"));
        assert!(source.contains("CancellationToken"));
    }
    let mut c = fixture();
    c.operations[0].kind = GraphqlOperationKind::Subscription;
    assert!(render(&c, "TestSdk", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    assert!(
        render(
            &fixture(),
            "TestSdk",
            GraphqlStyle::Idiomatic,
            &BTreeMap::from([(
                "users".into(),
                BTreeMap::from([("read".into(), "missing".into())])
            )])
        )
        .is_err()
    );
}
#[test]
#[ignore = "requires .NET 8 and pinned GraphQL.js 16.14.2; set POOLSTER_DOTNET and POOLSTER_GRAPHQL_JS_ROOT"]
fn generated_clients_compile_and_execute_against_graphql_server() {
    let dotnet = std::env::var("POOLSTER_DOTNET").unwrap_or_else(|_| "dotnet".into());
    let root = std::env::var("POOLSTER_GRAPHQL_JS_ROOT").expect("GraphQL.js node_modules root");
    let mut input = fixture();
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
    let server = r#"
const {graphql,buildSchema,version}=require(process.env.POOLSTER_GRAPHQL_JS_ROOT+'/graphql');
if(version!=='16.14.2')throw Error('Expected pinned GraphQL.js 16.14.2');
const schema=buildSchema('type Query { readUser(id: ID!, nickname: String): User } type User { name: String! nickname: String } type Mutation { ping: String! }');
const server=require('http').createServer(async(req,res)=>{let body='';for await(const chunk of req)body+=chunk;const v=JSON.parse(body);
if(req.url==='/http'){res.writeHead(503);res.end('unavailable');return;}
if(req.url==='/malformed'){res.end('invalid');return;}
const result=await graphql({schema,source:v.query,operationName:v.operationName,variableValues:v.variables,rootValue:{ping:()=>'pong',readUser:()=>({name:v.variables.id,nickname:()=>{if(v.variables.id==='partial')throw Error('partial failure');return v.variables.nickname??null;}})}});
res.setHeader('content-type','application/json');res.end(JSON.stringify(result));});server.listen(0,'127.0.0.1',()=>console.log(server.address().port));
"#;
    struct Server(std::process::Child);
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Server(
        Command::new("node")
            .args(["-e", server])
            .env("POOLSTER_GRAPHQL_JS_ROOT", root)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut port = String::new();
    BufReader::new(child.0.stdout.take().unwrap())
        .read_line(&mut port)
        .unwrap();
    assert!(
        !port.trim().is_empty(),
        "local GraphQL server did not start"
    );
    let endpoint = format!("http://127.0.0.1:{}", port.trim());
    for (style, custom) in [
        (GraphqlStyle::Raw, false),
        (GraphqlStyle::Flat, false),
        (GraphqlStyle::Idiomatic, false),
        (GraphqlStyle::Idiomatic, true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut groups = if custom {
            BTreeMap::from([(
                "users".into(),
                BTreeMap::from([("read".into(), "ReadUser".into())]),
            )])
        } else {
            BTreeMap::new()
        };
        if custom {
            groups
                .entry("users".into())
                .or_default()
                .insert("ping".into(), "Ping".into());
        }
        for (path, source) in render(&input, "TestSdk", style, &groups).unwrap().0 {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, source).unwrap();
        }
        fs::write(dir.path().join("Test.csproj"),"<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable><TreatWarningsAsErrors>true</TreatWarningsAsErrors></PropertyGroup></Project>").unwrap();
        let call = match style {
            GraphqlStyle::Raw => "GraphqlOperations.ReadUserAsync(client, variables)",
            GraphqlStyle::Flat => "client.ReadUserAsync(variables)",
            GraphqlStyle::Idiomatic if custom => "client.Users.ReadAsync(variables)",
            GraphqlStyle::Idiomatic => "client.Query.ReadUserAsync(variables)",
        };
        let ping = match style {
            GraphqlStyle::Raw => "GraphqlOperations.PingAsync(client)",
            GraphqlStyle::Flat => "client.PingAsync()",
            GraphqlStyle::Idiomatic if custom => "client.Users.PingAsync()",
            GraphqlStyle::Idiomatic => "client.Mutation.PingAsync()",
        };
        let source = include_str!("test.cs.tmpl")
            .replace("PING_CALL", ping)
            .replace("CALL", call)
            .replace("ENDPOINT", &endpoint);
        fs::write(dir.path().join("Program.cs"), source).unwrap();
        let result = Command::new(&dotnet)
            .args(["run", "--project", "Test.csproj", "--verbosity", "quiet"])
            .env("DOTNET_CLI_HOME", dir.path().join("home"))
            .env("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1")
            .current_dir(dir.path())
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
impl<L: poolster_core::engine::Language> Plugin<L> for Source {
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
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()> {
        let mut c = fixture();
        c.operations[0].name = self.name.into();
        cx.publish(c)
    }
}
#[test]
fn csharp_and_dotnet_provider_substitution_and_regeneration() {
    for name in ["ReadUser", "Renamed"] {
        for dotnet in [false, true] {
            let input = Source {
                meta: Meta::new(),
                name,
            };
            let other = Source {
                meta: Meta::new(),
                name: "Ignored",
            };
            let plugin = graphql(Some(input.meta.handle()));
            let packages = poolster_core::engine::Packages::new();
            let tree = if dotnet {
                packages
                    .package(
                        crate::dotnet_package("sdk")
                            .with(other)
                            .with(plugin)
                            .with(input),
                    )
                    .generate_native()
                    .unwrap()
            } else {
                packages
                    .package(crate::package("sdk").with(other).with(plugin).with(input))
                    .generate_native()
                    .unwrap()
            };
            let source = tree.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
            assert!(source.contains(&format!("{name}Async")));
            assert!(!source.contains("IgnoredAsync"));
        }
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
        let (files, _) = render(&input, "TestSdk", style, &BTreeMap::new()).unwrap();
        assert_eq!(
            files
                .keys()
                .filter(|p| p.starts_with("Operations/"))
                .count(),
            1000
        );
        assert!(files.keys().any(|p| p.starts_with("Models/")));
        assert!(files.contains_key("Client/GraphqlClient.cs"));
        assert!(!files.contains_key("Graphql.cs"));
        assert!(files.values().all(|s| s.len() <= 128 * 1024));
        let mut reversed = input.clone();
        reversed.operations.reverse();
        assert_eq!(
            files,
            render(&reversed, "TestSdk", style, &BTreeMap::new())
                .unwrap()
                .0
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let mut previous = poolster_core::GeneratedTree::default();
    previous
        .insert(GeneratedFile::new("Graphql.cs", "// previous owned monolith").unwrap())
        .unwrap();
    previous.write_to(dir.path()).unwrap();
    fs::write(dir.path().join("custom.cs"), "// user code").unwrap();
    let mut tree = poolster_core::GeneratedTree::default();
    for (path, source) in render(&fixture(), "TestSdk", GraphqlStyle::Flat, &BTreeMap::new())
        .unwrap()
        .0
    {
        tree.insert(GeneratedFile::new(path, source).unwrap())
            .unwrap();
    }
    tree.write_to(dir.path()).unwrap();
    assert!(!dir.path().join("Graphql.cs").exists());
    assert!(dir.path().join("custom.cs").exists());
    assert!(tree.check(dir.path()).unwrap().is_empty());
}
#[test]
fn large_records_split_at_property_boundaries() {
    let mut input = fixture();
    input.input_objects.insert(
        "LargeInput".into(),
        (0..2500)
            .map(|i| ModelField {
                name: format!("property{i:04}"),
                ty: string(true),
                optional: true,
                default_value: None,
            })
            .collect(),
    );
    let files = render(&input, "TestSdk", GraphqlStyle::Flat, &BTreeMap::new())
        .unwrap()
        .0;
    assert!(
        files
            .keys()
            .filter(|p| p.starts_with("Models/LargeInput_"))
            .count()
            > 1
    );
    assert!(files.values().all(|s| s.len() <= 128 * 1024));
}
