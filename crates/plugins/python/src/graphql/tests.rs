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
    let temp = tempfile::tempdir().unwrap();
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
    render(&input, "example", GraphqlStyle::Idiomatic, &groups)
        .unwrap()
        .0
        .write_to(temp.path())
        .unwrap();
    let script = r#"
import os, json, subprocess, threading
from example import Client
node = r'''
const { graphql, buildSchema } = require(process.env.POOLSTER_GRAPHQL_JS_ROOT + '/graphql');
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
    for result in [client.read_user(), client.users.read(), client.people.get({'id': None})]:
        assert result.status == 'partial' and result.data == {'hello':None}
        assert result.errors[0]['message'] == 'resolver failed'
finally:
    server.terminate(); server.wait(timeout=5)
"#;
    let output = Command::new("python3")
        .args(["-c", script])
        .env("POOLSTER_GRAPHQL_JS_ROOT", root)
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
