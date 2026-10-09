use super::*;
use poolster_core::native::{GraphqlOperation, ModelField, ModelKind, ModelType};
use std::{fs, process::Command};
fn string(nullable: bool) -> ModelType {
    ModelType {
        nullable,
        kind: ModelKind::Scalar("String".into()),
    }
}
fn fixture() -> GraphqlOperations {
    GraphqlOperations{schema_source:"type Query { readUser(id: ID!): User } type User { name: String! nickname: String }".into(),operation_source:"query ReadUser($id: ID!, $nickname: String) { readUser(id: $id) { name nickname } }".into(),input_objects:BTreeMap::new(),operations:vec![GraphqlOperation{name:"ReadUser".into(),kind:GraphqlOperationKind::Query,document:"query ReadUser($id: ID!, $nickname: String) { readUser(id: $id) { name nickname } }".into(),variables:vec![ModelField{name:"id".into(),ty:string(false),optional:false,default_value:None},ModelField{name:"nickname".into(),ty:string(true),optional:true,default_value:None}],result:ModelType{nullable:false,kind:ModelKind::Object(vec![ModelField{name:"readUser".into(),optional:false,default_value:None,ty:ModelType{nullable:true,kind:ModelKind::Object(vec![ModelField{name:"name".into(),ty:string(false),optional:false,default_value:None},ModelField{name:"nickname".into(),ty:string(true),optional:true,default_value:None}])}}])}}]}
}
#[test]
fn generated_go_compiles_and_executes() {
    assert!(
        Command::new("go")
            .arg("version")
            .status()
            .expect("Go toolchain required")
            .success()
    );
    let cache = tempfile::tempdir().unwrap();
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
        let (source, _) = render(&fixture(), "graphqlclient", style, &groups).unwrap();
        fs::write(dir.path().join("graphql.go"), source).unwrap();
        fs::write(
            dir.path().join("go.mod"),
            "module graphqlclient\n\ngo 1.22\n",
        )
        .unwrap();
        let call = match style {
            GraphqlStyle::Raw => "ReadUser(ctx, client, variables)",
            GraphqlStyle::Flat => "client.ReadUser(ctx, variables)",
            GraphqlStyle::Idiomatic if custom => "client.Users().Read(ctx, variables)",
            GraphqlStyle::Idiomatic => "client.Query().ReadUser(ctx, variables)",
        };
        let test = include_str!("runtime_test.go.tmpl").replace("CALL_OPERATION", call);
        fs::write(dir.path().join("graphql_test.go"), test).unwrap();
        let out = Command::new("go")
            .args(["test", "./..."])
            .current_dir(dir.path())
            .env("GOWORK", "off")
            .env(
                "GOCACHE",
                std::env::var_os("GOCACHE")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| cache.path().to_path_buf()),
            )
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
#[test]
fn rejects_unsupported_and_invalid_configuration() {
    let mut c = fixture();
    c.operations[0].kind = GraphqlOperationKind::Subscription;
    assert!(
        render(&c, "client", GraphqlStyle::Flat, &BTreeMap::new())
            .unwrap_err()
            .to_string()
            .contains("subscriptions")
    );
    assert!(
        render(
            &fixture(),
            "../escape",
            GraphqlStyle::Flat,
            &BTreeMap::new()
        )
        .is_err()
    );
    let groups = BTreeMap::from([(
        "users".into(),
        BTreeMap::from([("read".into(), "Missing".into())]),
    )]);
    assert!(render(&fixture(), "client", GraphqlStyle::Idiomatic, &groups).is_err());
}
struct Source {
    meta: Meta,
    name: &'static str,
}
impl Plugin<crate::Go> for Source {
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
    fn generate(&self, cx: &mut PluginContext<'_, crate::Go>) -> Result<()> {
        let mut c = fixture();
        c.operations[0].name = self.name.into();
        cx.publish(c)
    }
}
#[test]
fn selected_provider_drives_native_graph_and_regeneration() {
    for name in ["ReadUser", "Renamed"] {
        let source = Source {
            meta: Meta::new(),
            name,
        };
        let unselected = Source {
            meta: Meta::new(),
            name: "Ignored",
        };
        let generator = graphql(Some(source.meta.handle()));
        let tree = poolster_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .with(unselected)
                    .with(generator)
                    .with(source),
            )
            .generate_native()
            .unwrap();
        let source = tree
            .iter()
            .map(|(_, source)| source)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(source.contains(&format!("func {name}")));
        assert!(!source.contains("func Ignored"));
    }
}
/// Execute with POOLSTER_GRAPHQL_JS_ROOT set to the parent of graphql@16.14.2.
#[test]
#[ignore = "requires pinned graphql@16.14.2 Node module and loopback sockets"]
fn go_graphql_against_real_graphql_server() {
    use std::io::{BufRead, BufReader};
    let root = std::env::var("POOLSTER_GRAPHQL_JS_ROOT").expect("set POOLSTER_GRAPHQL_JS_ROOT");
    let temp = tempfile::tempdir().unwrap();
    let mut contract = fixture();
    contract.operations[0].document="query ReadUser($id: ID!, $nickname: String) { readUser(id: $id, nickname: $nickname) { name nickname } }".into();
    let (source, _) = render(
        &contract,
        "graphqlclient",
        GraphqlStyle::Flat,
        &BTreeMap::new(),
    )
    .unwrap();
    fs::write(temp.path().join("graphql.go"), source).unwrap();
    fs::write(
        temp.path().join("go.mod"),
        "module graphqlclient\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(temp.path().join("graphql_test.go"),r#"package graphqlclient
import("context";"errors";"os";"testing")
func TestRealGraphQLServer(t *testing.T) {
 c:=NewClient(os.Getenv("GRAPHQL_ENDPOINT"),nil)
 success,err:=c.ReadUser(context.Background(),ReadUserVariables{ID:"42",Nickname:Some("Nick")})
 if err!=nil || success.Data.ReadUser.Name!="Alice" || *success.Data.ReadUser.Nickname.Value!="Nick" {t.Fatalf("success: %+v %v",success,err)}
 partial,err:=c.ReadUser(context.Background(),ReadUserVariables{ID:"partial"})
 var gql GraphQLErrors
 if !errors.As(err,&gql)||partial.Data.ReadUser.Name!="Alice" || !partial.Data.ReadUser.Nickname.Set || partial.Data.ReadUser.Nickname.Value!=nil || gql[0].Path[1]!="nickname" {t.Fatalf("partial: %+v %v",partial,err)}
 null,err:=c.ReadUser(context.Background(),ReadUserVariables{ID:"non-null"})
 if !errors.As(err,&gql)||null.Data.ReadUser!=nil {t.Fatalf("non-null propagation: %+v %v",null,err)}
}
"#).unwrap();
    let script = r#"
const root=process.env.POOLSTER_GRAPHQL_JS_ROOT;
if(require(root+'/graphql/package.json').version!=='16.14.2') throw new Error('expected graphql@16.14.2');
const {graphql,buildSchema}=require(root+'/graphql');
const schema=buildSchema('type Query { readUser(id: ID!, nickname: String): User } type User { name: String! nickname: String }');
const server=require('http').createServer(async(req,res)=>{
let body='';for await(const chunk of req)body+=chunk;
const value=JSON.parse(body);
const result=await graphql({schema,source:value.query,operationName:value.operationName,variableValues:value.variables,rootValue:{readUser:({id,nickname})=>({name:id==='non-null'?null:'Alice',nickname:()=>{if(id==='partial')throw new Error('nickname unavailable');return nickname??null;}})}});
res.setHeader('Content-Type','application/graphql-response+json');res.end(JSON.stringify(result));
});server.listen(0,'127.0.0.1',()=>console.log(server.address().port));
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
            .args(["-e", script])
            .env("POOLSTER_GRAPHQL_JS_ROOT", root)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut port = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut port)
        .unwrap();
    let port = port.trim().parse::<u16>().expect("GraphQL server port");
    let output = Command::new("go")
        .args(["test", "./..."])
        .current_dir(temp.path())
        .env("GOWORK", "off")
        .env(
            "GOCACHE",
            std::env::var_os("GOCACHE")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| temp.path().join("cache")),
        )
        .env("GRAPHQL_ENDPOINT", format!("http://127.0.0.1:{port}"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
