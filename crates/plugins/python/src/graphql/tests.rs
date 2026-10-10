use super::*;
use std::process::Command;
fn contract() -> GraphqlOperations {
    let string = ModelType {
        nullable: false,
        kind: ModelKind::Scalar("String".into()),
    };
    GraphqlOperations {
        schema_source: "type Query { hello: String! }".into(),
        operation_source: String::new(),
        input_objects: BTreeMap::new(),
        operations: vec![poolster_core::native::GraphqlOperation {
            name: "ReadUser".into(),
            kind: GraphqlOperationKind::Query,
            document: "query ReadUser($id: ID) { hello }".into(),
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
                kind: ModelKind::Object(vec![ModelField {
                    name: "hello".into(),
                    ty: string,
                    optional: true,
                    default_value: None,
                }]),
            },
        }],
    }
}
#[test]
fn python_graphql_compiles_and_executes_http() {
    let temp = tempfile::tempdir().unwrap();
    let (tree, _) = render(
        &contract(),
        "example",
        GraphqlStyle::Idiomatic,
        &BTreeMap::new(),
    )
    .unwrap();
    tree.write_to(temp.path()).unwrap();
    let script = r#"
import compileall, json, threading
from http.server import BaseHTTPRequestHandler, HTTPServer
assert compileall.compile_dir('src', quiet=1)
from example import Client, GraphqlErrors
from example.models import Operation0Variables, Operation0Result
assert Operation0Variables.__optional_keys__ == frozenset(['id'])
assert Operation0Result.__optional_keys__ == frozenset(['hello'])
class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        assert body['operationName'] == 'ReadUser'
        assert body['query'].startswith('query ReadUser')
        assert body['variables'] in ({}, {'id': None})
        self.send_response(200); self.end_headers()
        self.wfile.write(json.dumps({'data': {'hello': None}, 'errors': [{'message': 'partial'}]}).encode())
server=HTTPServer(('127.0.0.1',0), Handler)
threading.Thread(target=server.serve_forever,daemon=True).start()
client=Client('http://127.0.0.1:'+str(server.server_port))
result=client.read_user({})
assert result.status == 'partial' and result.data == {'hello':None} and result.data_present
try: result.require_data()
except GraphqlErrors as error: assert error.data == result.data
else: raise AssertionError('partial must be explicit')
assert client.query.read_user({'id':None}).status == 'partial'
server.shutdown()
"#;
    let output = Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", temp.path().join("src"))
        .env("PYTHONPYCACHEPREFIX", temp.path().join("pycache"))
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    );
}
#[test]
fn unsupported_and_colliding_contracts_fail() {
    let mut input = contract();
    input.operations[0].kind = GraphqlOperationKind::Subscription;
    assert!(render(&input, "example", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    input = contract();
    input.operations.push(input.operations[0].clone());
    assert!(render(&input, "example", GraphqlStyle::Flat, &BTreeMap::new()).is_err());
    let mut groups = BTreeMap::new();
    groups.insert(
        "users".into(),
        BTreeMap::from([("read".into(), "Unknown".into())]),
    );
    assert!(render(&contract(), "example", GraphqlStyle::Idiomatic, &groups).is_err());
}

/// Run with POOLSTER_GRAPHQL_JS_ROOT pointing to a directory containing graphql@16.14.2.
#[test]
#[ignore = "requires pinned graphql@16.14.2 Node module and loopback sockets"]
fn python_graphql_against_real_graphql_server() {
    let root = std::env::var("POOLSTER_GRAPHQL_JS_ROOT").expect("set POOLSTER_GRAPHQL_JS_ROOT");
    let mut input = contract();
    input.operations[0].document = "query ReadUser($id: ID) { hello(id: $id) }".into();
    let groups = BTreeMap::from([
        (
            "users".into(),
            BTreeMap::from([("read".into(), "ReadUser".into())]),
        ),
        (
            "people".into(),
            BTreeMap::from([("get".into(), "ReadUser".into())]),
        ),
    ]);
    for (style, custom) in [
        (GraphqlStyle::Raw, false),
        (GraphqlStyle::Flat, false),
        (GraphqlStyle::Idiomatic, false),
        (GraphqlStyle::Idiomatic, true),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let selected = if custom {
            groups.clone()
        } else {
            BTreeMap::new()
        };
        render(&input, "example", style, &selected)
            .unwrap()
            .0
            .write_to(temp.path())
            .unwrap();
        let call = match style {
            GraphqlStyle::Raw => "operations.read_user(client._transport, {'id':None})",
            GraphqlStyle::Flat => "client.read_user({'id':None})",
            GraphqlStyle::Idiomatic if custom => "client.users.read({'id':None})",
            GraphqlStyle::Idiomatic => "client.query.read_user({'id':None})",
        };
        let script = r#"
import os, json, subprocess, threading
from example import Client, operations
node = r'''
const { graphql, buildSchema, version } = require(process.env.POOLSTER_GRAPHQL_JS_ROOT + '/graphql');
if(version !== '16.14.2') throw Error('Expected pinned GraphQL16.14.2');
const schema = buildSchema('type Query { hello(id: ID): String }');
const http = require('http');
const server = http.createServer(async (req,res) => {
let body=''; for await(const chunk of req) body+=chunk;
const value=JSON.parse(body);
const result=await graphql({schema, source:value.query, variableValues:value.variables, operationName:value.operationName, rootValue:{hello:()=>{throw new Error('resolver failed')}}});
res.setHeader('Content-Type','application/json');res.end(JSON.stringify(result));
});
server.listen(0,'127.0.0.1',()=>console.log(server.address().port));
'''
server=subprocess.Popen(['node','-e',node],stdout=subprocess.PIPE,text=True)
try:
    port=int(server.stdout.readline())
    client=Client('http://127.0.0.1:'+str(port))
    for result in [CALL_OPERATION]:
        assert result.status == 'partial' and result.data == {'hello':None}
        assert result.errors[0]['message'] == 'resolver failed'
finally:
    server.terminate(); server.wait(timeout=5)
"#.replace("CALL_OPERATION",call);
        let output = Command::new("python3")
            .args(["-c", &script])
            .env("POOLSTER_GRAPHQL_JS_ROOT", &root)
            .env("PYTHONPATH", temp.path().join("src"))
            .env("PYTHONPYCACHEPREFIX", temp.path().join("pycache"))
            .current_dir(temp.path())
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
fn python_graphql_styles_and_recursive_inputs() {
    let mut input = contract();
    input.input_objects.insert(
        "Filter".into(),
        vec![ModelField {
            name: "next".into(),
            ty: ModelType {
                nullable: true,
                kind: ModelKind::Named("Filter".into()),
            },
            optional: true,
            default_value: None,
        }],
    );
    input.operations[0].variables.push(ModelField {
        name: "filter".into(),
        ty: ModelType {
            nullable: true,
            kind: ModelKind::Named("Filter".into()),
        },
        optional: true,
        default_value: None,
    });
    for style in [
        GraphqlStyle::Raw,
        GraphqlStyle::Flat,
        GraphqlStyle::Idiomatic,
    ] {
        let temp = tempfile::tempdir().unwrap();
        render(&input, "example", style, &BTreeMap::new())
            .unwrap()
            .0
            .write_to(temp.path())
            .unwrap();
        let script = r#"
from typing import get_type_hints
from example import Client
from example.models import Filter, Operation0Variables
from example import operations
assert 'next' in get_type_hints(Filter)
assert get_type_hints(Operation0Variables)['filter']
assert Filter.__optional_keys__ == frozenset(['next'])
assert callable(operations.read_user)
"#;
        let output = Command::new("python3")
            .args(["-c", script])
            .env("PYTHONPATH", temp.path().join("src"))
            .env("PYTHONPYCACHEPREFIX", temp.path().join("pycache"))
            .current_dir(temp.path())
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
fn large_modular_packages_preserve_types_files_and_regeneration() {
    let mut c = contract();
    let op = c.operations[0].clone();
    c.operations = (0..300)
        .map(|i| {
            let mut op = op.clone();
            op.name = format!("ReadOperation{i:03}");
            op.document = op.document.replace("ReadUser", &op.name);
            op
        })
        .collect();
    c.input_objects.insert(
        "Wide".into(),
        (0..512)
            .map(|i| ModelField {
                name: format!("field{i:03}"),
                ty: ModelType {
                    nullable: i % 3 == 0,
                    kind: ModelKind::Scalar("String".into()),
                },
                optional: i % 2 == 0,
                default_value: None,
            })
            .collect(),
    );
    c.input_objects.insert(
        "Filter".into(),
        vec![ModelField {
            name: "next".into(),
            ty: ModelType {
                nullable: true,
                kind: ModelKind::Named("Filter".into()),
            },
            optional: true,
            default_value: None,
        }],
    );
    c.operations[0].variables.push(ModelField {
        name: "filter".into(),
        ty: ModelType {
            nullable: true,
            kind: ModelKind::Named("Filter".into()),
        },
        optional: true,
        default_value: None,
    });
    for (style, custom) in [
        (GraphqlStyle::Raw, false),
        (GraphqlStyle::Flat, false),
        (GraphqlStyle::Idiomatic, false),
        (GraphqlStyle::Idiomatic, true),
    ] {
        let groups = if custom {
            BTreeMap::from([(
                "users".into(),
                BTreeMap::from([("read".into(), "ReadOperation299".into())]),
            )])
        } else {
            BTreeMap::new()
        };
        let (tree, _) = render(&c, "example", style, &groups).unwrap();
        assert!(
            !tree
                .iter()
                .any(|(p, _)| p == std::path::Path::new("src/example/models.py")
                    || p == std::path::Path::new("src/example/operations.py"))
        );
        for (path, code) in tree.iter() {
            assert!(
                code.len() < 16384,
                "{} too large: {}",
                path.display(),
                code.len()
            );
            assert!(
                code.lines().all(|line| line.len() < 8192),
                "{} long line",
                path.display()
            );
        }
        let mut reordered = c.clone();
        reordered.operations.reverse();
        reordered.input_objects.get_mut("Wide").unwrap().reverse();
        let (again, _) = render(&reordered, "example", style, &groups).unwrap();
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
            GraphqlStyle::Raw => "operations.read_operation299(client._transport)",
            GraphqlStyle::Flat => "client.read_operation299()",
            GraphqlStyle::Idiomatic if custom => "client.users.read()",
            GraphqlStyle::Idiomatic => "client.query.read_operation299()",
        };
        let script = format!(
            r#"
import compileall, typing, importlib
assert compileall.compile_dir('src',quiet=1)
from example import Client, operations
from example.models import Wide, Filter, ReadOperation000Variables, Operation0Variables, ReadOperation299Result
assert len(typing.get_type_hints(Wide))==512
assert len(Wide.__required_keys__)==256 and len(Wide.__optional_keys__)==256
assert typing.get_type_hints(Filter)['next']==typing.Optional[Filter]
assert typing.get_type_hints(ReadOperation000Variables)['filter']==typing.Optional[Filter]
assert Operation0Variables is ReadOperation000Variables
leaf=importlib.import_module('example.models.read_operation000_variables')
assert leaf.ReadOperation000Variables is ReadOperation000Variables
class Transport:
 def execute(self,*args):
  from example import GraphqlResponse
  return GraphqlResponse(data={{'hello':'ok'}},errors=[])
client=Client('http://localhost'); client._transport=Transport()
assert {call}.require_data()=={{'hello':'ok'}}
assert typing.get_type_hints(operations.read_operation299)['return'].__args__[0] is ReadOperation299Result
"#
        );
        let out = Command::new("python3")
            .args(["-c", &script])
            .env("PYTHONPATH", dir.path().join("src"))
            .env("PYTHONPYCACHEPREFIX", dir.path().join("pycache"))
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let mut removed = c.clone();
        removed.operations.pop();
        let reduced = render(&removed, "example", style, &BTreeMap::new())
            .unwrap()
            .0;
        assert!(!reduced.check(dir.path()).unwrap().removed.is_empty());
        reduced.write_to(dir.path()).unwrap();
        assert!(
            !dir.path()
                .join("src/example/operations/read_operation299.py")
                .exists()
        );
    }
}

#[test]
#[ignore = "requires pinned graphql-sse/GraphQL server and loopback sockets"]
fn python_sse_incremental_and_scalar_codecs_execute() {
    let root = std::env::var("POOLSTER_GRAPHQL_SSE_ROOT").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut input = contract();
    input.operations[0].variables[0].ty.kind = ModelKind::Scalar("DateTime".into());
    input.operations[0].document = "query ReadUser($id:DateTime){hello(id:$id)}".into();
    if let ModelKind::Object(fields) = &mut input.operations[0].result.kind {
        fields[0].ty.kind = ModelKind::Scalar("DateTime".into());
    }
    let mut subscription = input.operations[0].clone();
    subscription.name = "Changed".into();
    subscription.kind = GraphqlOperationKind::Subscription;
    subscription.document = "subscription Changed($id:DateTime){changed(id:$id)}".into();
    if let ModelKind::Object(fields) = &mut subscription.result.kind {
        fields[0].name = "changed".into();
    }
    input.operations.push(subscription);
    render_advanced(
        &input,
        "example",
        GraphqlStyle::Flat,
        &BTreeMap::new(),
        true,
        false,
    )
    .unwrap()
    .0
    .write_to(temp.path())
    .unwrap();
    input
        .operations
        .retain(|op| op.kind != GraphqlOperationKind::Subscription);
    render_advanced(
        &input,
        "incremental",
        GraphqlStyle::Flat,
        &BTreeMap::new(),
        false,
        true,
    )
    .unwrap()
    .0
    .write_to(temp.path().join("incremental"))
    .unwrap();
    let script = r#"
import json, subprocess, datetime, io
from example import Client
from example.streaming import sse_events, apply_incremental
from incremental import Client as IncrementalClient
server_code=r'''
const {createHandler}=require(process.env.POOLSTER_GRAPHQL_SSE_ROOT+'/graphql-sse/lib/use/http');
const {buildSchema,graphql}=require(process.env.POOLSTER_GRAPHQL_SSE_ROOT+'/graphql');
const schema=buildSchema('scalar DateTime type Query {hello(id:DateTime):DateTime} type Subscription {changed(id:DateTime):DateTime}');
const scalar=schema.getType('DateTime');scalar.parseValue=v=>new Date(v);scalar.serialize=v=>v.toISOString();
schema.getSubscriptionType().getFields().changed.subscribe=async function*(_source,args){yield{changed:args.id};};
const sse=createHandler({schema});
require('http').createServer(async(req,res)=>{
if(req.headers.authorization!=='Bearer secret'){res.statusCode=401;res.end();return;}
if(req.url==='/multipart'){res.setHeader('Content-Type','multipart/mixed; boundary="parts"; deferSpec=20220824');for(const frame of [{data:{},hasNext:true},{incremental:[{path:[],data:{hello:'2025-01-01T00:00:00.000Z'}}],hasNext:false}])res.write('--parts\r\nContent-Type: application/json\r\n\r\n'+JSON.stringify(frame)+'\r\n');res.end('--parts--\r\n');return;}
if(req.headers.accept==='text/event-stream'){await sse(req,res);return;}
let body='';for await(const chunk of req)body+=chunk;const value=JSON.parse(body);const result=await graphql({schema,source:value.query,operationName:value.operationName,variableValues:value.variables,rootValue:{hello:args=>args.id}});res.setHeader('Content-Type','application/json');res.end(JSON.stringify(result));
}).listen(0,'127.0.0.1',function(){console.log(this.address().port)});
'''
assert apply_incremental({'items':[1]}, {'incremental':[{'path':['items',1],'items':[2]}]}) == {'items':[1,2]}
assert apply_incremental({'items':[]}, {'incremental':[{'path':[],'errors':[{'message':'failed'}]}]}) == {'items':[]}
assert apply_incremental({}, {'incremental':[{'path':[],'data':None}]}) is None
server=subprocess.Popen(['node','-e',server_code],stdout=subprocess.PIPE,text=True)
try:
    endpoint='http://127.0.0.1:'+server.stdout.readline().strip()
    date=datetime.datetime(2025,1,1,tzinfo=datetime.timezone.utc)
    codecs={'DateTime':{'encode':lambda value:value.isoformat(),'decode':lambda value:datetime.datetime.fromisoformat(value.replace('Z','+00:00'))}}
    client=Client(endpoint,headers={'Authorization':'Bearer secret'},scalar_codecs=codecs)
    assert client.read_user({'id':date}).data['hello']==date
    events=list(client.changed({'id':date}));assert events[0].data['changed']==date
    frames=list(IncrementalClient(endpoint+'/multipart',headers={'Authorization':'Bearer secret'},scalar_codecs=codecs).read_user({'id':date}))
    assert frames[0].final is None and frames[0].data=={}
    assert frames[-1].final.data['hello']==date
    try:list(sse_events(io.BytesIO(b'event: next\ndata: invalid\n\n'),100))
    except ValueError:pass
    else:raise AssertionError('malformed SSE must fail')
    try:list(sse_events(io.BytesIO(b'event: next\ndata: {}\n\n'),100))
    except ValueError:pass
    else:raise AssertionError('EOF before complete must fail')
finally:server.terminate();server.wait(timeout=5)
"#;
    let paths =
        std::env::join_paths([temp.path().join("src"), temp.path().join("incremental/src")])
            .unwrap();
    let output = Command::new("python3")
        .args(["-c", script])
        .env("POOLSTER_GRAPHQL_SSE_ROOT", root)
        .env("PYTHONPATH", paths)
        .env("PYTHONPYCACHEPREFIX", temp.path().join("pycache"))
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
