use super::*;
use poolster_core::native::{ModelField, ModelKind, ModelType};
fn contract() -> GraphqlOperations {
    GraphqlOperations {
        schema_source: "type Query { hello(id: ID): String }".into(),
        operation_source: String::new(),
        input_objects: Default::default(),
        operations: vec![GraphqlOperation {
            name: "ReadUser".into(),
            kind: GraphqlOperationKind::Query,
            document: "query ReadUser($id: ID) { hello(id: $id) }".into(),
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
                kind: ModelKind::Object(vec![]),
            },
        }],
    }
}
#[test]
fn generation_is_deterministic_and_requires_current_complete_blocks() {
    let contract = contract();
    let blocks = contract.operation_blocks("provider");
    let tree = render(&contract, &blocks, "example", "example-cli", "0.1.0", None).unwrap();
    let again = render(&contract, &blocks, "example", "example-cli", "0.1.0", None).unwrap();
    assert!(tree.iter().eq(again.iter()));
    assert!(
        tree.get(format!(
            "src/operations/{}.ts",
            poolster_core::files::source_file_stem("ReadUser")
        ))
        .unwrap()
        .contains("ReadUser")
    );
    let mut stale = blocks.clone();
    stale.parent.as_mut().unwrap().revision = "stale".into();
    assert!(render(&contract, &stale, "example", "example-cli", "0.1.0", None).is_err());
    let mut partial = blocks.clone();
    partial.state = poolster_core::blocks::CollectionState::Partial {
        diagnostics: vec!["incomplete".into()],
    };
    assert!(render(&contract, &partial, "example", "example-cli", "0.1.0", None).is_err());
    let mut duplicate = contract.clone();
    let mut mutation = duplicate.operations[0].clone();
    mutation.kind = GraphqlOperationKind::Mutation;
    duplicate.operations.push(mutation);
    assert!(
        render(
            &duplicate,
            &duplicate.operation_blocks("provider"),
            "example",
            "example-cli",
            "0.1.0",
            None
        )
        .is_err()
    );
    let mut subscription = contract.clone();
    subscription.operations[0].kind = GraphqlOperationKind::Subscription;
    assert!(
        render(
            &subscription,
            &subscription.operation_blocks("provider"),
            "example",
            "example-cli",
            "0.1.0",
            None
        )
        .is_err()
    );
}

#[test]
fn reordered_large_plans_keep_bounded_sources_and_stable_names() {
    let mut input = contract();
    let operation = input.operations[0].clone();
    input.operations = (0..301)
        .map(|i| {
            let mut op = operation.clone();
            op.name = format!("ReadUser{i}");
            op.document = format!("query ReadUser{i} {{ hello }}");
            op
        })
        .collect();
    let first = render(
        &input,
        &input.operation_blocks("source"),
        "example",
        "example-cli",
        "0.1.0",
        None,
    )
    .unwrap();
    input.operations.reverse();
    let second = render(
        &input,
        &input.operation_blocks("source"),
        "example",
        "example-cli",
        "0.1.0",
        None,
    )
    .unwrap();
    assert!(first.iter().eq(second.iter()));
    for (path, contents) in first.iter() {
        if path.to_string_lossy().ends_with(".ts") {
            assert!(contents.lines().count() < 150, "{}", path.display());
        }
    }
}

#[test]
fn provider_substitution_and_explicit_operation_blocks_use_graph_order() {
    use poolster_core::engine::{Meta, Packages, Plugin, PluginContext, Provision};
    struct Source {
        meta: Meta,
    }
    impl Plugin<TypeScriptCli> for Source {
        fn kind(&self) -> &'static str {
            "custom-graphql-source"
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
        fn generate(&self, cx: &mut PluginContext<'_, TypeScriptCli>) -> Result<()> {
            let whole = contract();
            let parent = ContractReference::from_bytes(
                GraphqlOperations::NAME,
                "custom-source",
                &serde_json::to_vec(&whole)?,
            );
            cx.publish_with_reference(whole, parent)
        }
    }
    let source = Source { meta: Meta::new() };
    let whole = source.meta.handle::<GraphqlOperations>();
    let blocks = poolster_core::engine::hooks::<TypeScriptCli>()
        .extract_blocks(Some(whole), |input| {
            Ok(input.operation_blocks("custom-source"))
        });
    let generator = graphql()
        .input(whole)
        .blocks(blocks.handle())
        .command_name("custom");
    let tree = poolster_core::engine::Packages::new()
        .package(
            crate::package("cli")
                .with(generator)
                .with(blocks)
                .with(source),
        )
        .generate_native()
        .unwrap();
    assert!(tree.get("cli/src/index.ts").unwrap().contains("custom"));
    let source = Source { meta: Meta::new() };
    let generator = graphql().input(source.meta.handle());
    assert!(
        Packages::new()
            .package(crate::package("cli").with(generator).with(source))
            .generate_native()
            .is_ok()
    );
}

#[test]
#[ignore = "requires pinned local Node/TypeScript/commander/GraphQL dependencies and loopback sockets"]
fn compiled_graphql_cli_executes_queries_mutations_and_reports_failures() {
    use std::process::Command;
    let root = std::env::var("POOLSTER_GRAPHQL_JS_ROOT").expect("set POOLSTER_GRAPHQL_JS_ROOT");
    let temp = tempfile::tempdir().unwrap();
    let mut input = contract();
    let mut mutation = input.operations[0].clone();
    mutation.name = "RenameUser".into();
    mutation.kind = GraphqlOperationKind::Mutation;
    mutation.document = "mutation RenameUser($name: String!) { rename(name: $name) }".into();
    mutation.variables = vec![ModelField {
        name: "name".into(),
        ty: ModelType {
            nullable: false,
            kind: ModelKind::Scalar("String".into()),
        },
        optional: false,
        default_value: None,
    }];
    input.operations.push(mutation);
    render(
        &input,
        &input.operation_blocks("source"),
        "example",
        "example-cli",
        "0.1.0",
        None,
    )
    .unwrap()
    .write_to(temp.path())
    .unwrap();
    std::os::unix::fs::symlink(&root, temp.path().join("node_modules")).unwrap();
    let compile = Command::new("node")
        .arg(format!("{root}/typescript/lib/tsc.js"))
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );
    let script = r#"
const {spawn,spawnSync} = require('child_process');
const fs=require('fs');
const assert=require('assert');
assert.equal(require(process.env.POOLSTER_GRAPHQL_JS_ROOT+'/graphql/package.json').version,'16.14.2');
assert.equal(require(process.env.POOLSTER_GRAPHQL_JS_ROOT+'/typescript/package.json').version,'5.9.3');
assert.equal(require(process.env.POOLSTER_GRAPHQL_JS_ROOT+'/commander/package.json').version,'13.1.0');
const serverCode=`
const {graphql,buildSchema}=require(process.env.POOLSTER_GRAPHQL_JS_ROOT+'/graphql');
const schema=buildSchema('type Query { hello(id: ID): String } type Mutation { rename(name: String!): String! }');
const http=require('http');
http.createServer(async(req,res)=>{
if(req.headers.authorization!=='Bearer secret'||req.headers['x-test']!=='value'){res.statusCode=401;res.end('{}');return;}
if(req.url==='/malformed'){res.end('invalid JSON');return;}
if(req.url==='/error'){res.statusCode=400;res.end(JSON.stringify({errors:[{message:'bad request'}]}));return;}
let body='';for await(const chunk of req)body+=chunk;
const payload=JSON.parse(body);
const root={hello:args=>{if(args.id==='partial')throw new Error('partial resolver');return Object.hasOwn(args,'id')?(args.id===null?'null':args.id):'omitted'},rename:args=>args.name};
const result=await graphql({schema,source:payload.query,variableValues:payload.variables,operationName:payload.operationName,rootValue:root});res.setHeader('Content-Type','application/json');res.end(JSON.stringify(result));
}).listen(0,'127.0.0.1',function(){console.log(this.address().port)});
`;
const server=spawn(process.execPath,['-e',serverCode],{env:process.env});
server.stderr.pipe(process.stderr);
server.stdout.once('data',buffer=>{
  const endpoint='http://127.0.0.1:'+buffer.toString().trim();
  const run=(args,url=endpoint)=>spawnSync(process.execPath,['dist/index.js',...args,'--header','X-Test: value'],{encoding:'utf8',env:{...process.env,GRAPHQL_ENDPOINT:url,GRAPHQL_TOKEN:'secret'}});
  try {
    let help=spawnSync(process.execPath,['dist/index.js','--help'],{encoding:'utf8'});assert.equal(help.status,0,help.stderr);assert(help.stdout.includes('read-user'));
    let result=run(['read-user']);assert.equal(result.status,0,result.stderr);assert.equal(JSON.parse(result.stdout).data.hello,'omitted');
    result=run(['read-user','--variables','{"id":null}']);assert.equal(result.status,0,result.stderr);assert.equal(JSON.parse(result.stdout).data.hello,'null');
    fs.writeFileSync('variables.json','{"name":"Ada"}');result=run(['rename-user','--variables-file','variables.json']);assert.equal(result.status,0,result.stderr);assert.equal(JSON.parse(result.stdout).data.rename,'Ada');
    result=run(['read-user','--variables','{"id":"partial"}']);assert.equal(result.status,3,result.stderr);assert(JSON.parse(result.stdout).errors.length);
    result=run(['read-user'],endpoint+'/error');assert.equal(result.status,4,result.stderr);
    result=run(['read-user'],endpoint+'/malformed');assert.equal(result.status,2,result.stderr);
    for(const args of [['read-user','--variables','[]'],['read-user','--variables','invalid'],['rename-user'],['rename-user','--variables','{"name":null}']]) {
      result=run(args);assert.equal(result.status,2,result.stderr);assert.equal(result.stdout,'');
    }
  } catch(error){console.error(error);process.exitCode=1;} finally {server.kill();}
});
"#;
    let output = Command::new("node")
        .args(["-e", script])
        .env("POOLSTER_GRAPHQL_JS_ROOT", &root)
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
