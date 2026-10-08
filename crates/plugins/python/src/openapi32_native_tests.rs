use super::{
    AdditionalProperties, Api, Operation, Schema, SchemaKind, SchemaValue, SdkClientStyle,
    render_sdk,
};
use poolster_core::Field;
use std::process::Command;
#[test]
fn native_buffered_sequences_round_trip_and_reject_invalid_records() {
    let root = tempfile::tempdir().unwrap();
    let mut api = poolster_core::Api {
        name: "Example API".into(),
        version: "1".into(),
        ..Default::default()
    };
    let mut nullable = poolster_core::SchemaValue::new(poolster_core::SchemaKind::String);
    nullable.nullable = true;
    api.schemas.push(poolster_core::Schema::new(
        "Note",
        poolster_core::SchemaValue::new(poolster_core::SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "note-value".into(),
                value: nullable,
                required: false,
                annotations: Default::default(),
            }],
            additional_properties: poolster_core::AdditionalProperties::Forbidden,
        }),
    ));
    api.operations.push(poolster_core::Operation {
        id: "search".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/search".into(),
        parameters: vec![poolster_core::OperationParameter {
            name: "filter".into(),
            location: "querystring".into(),
            required: true,
            schema: Some(poolster_core::SchemaValue::new(
                poolster_core::SchemaKind::String,
            )),
            description: None,
            annotations: Default::default(),
        }],
        request_body: None,
        responses: vec![],
        security: vec![],
        annotations: Default::default(),
    });
    let parameters = [("selector", "path"), ("filter", "query"), ("condition", "header"), ("preferences", "cookie")].into_iter().map(|(name, location)| poolster_core::OperationParameter {
            name: name.into(), location: location.into(), required: true, description: None,
            schema: Some(poolster_core::SchemaValue::unknown()), annotations: std::collections::BTreeMap::from([("poolster.parameter_content".into(), serde_json::json!([{"content_type":"application/json","schema_definition":{"type":"object"}}]))]),
        }).collect();
    api.operations.push(poolster_core::Operation {
        id: "filter".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/items/{selector}".into(),
        parameters,
        request_body: None,
        responses: vec![],
        security: vec![],
        annotations: Default::default(),
    });
    super::render_sdk_with_async(
        &api,
        "sdk/python",
        Some("example-api-sdk"),
        poolster_core::SdkClientStyle::Flat,
        true,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    let script = r#"import json
from email.message import Message
from example_api_sdk import Client
import example_api_sdk.runtime as runtime
from example_api_sdk.response_validation import decode_sequence, ResponseDecodeError
from example_api_sdk import Note,with_present_fields
from example_api_sdk.runtime import to_wire
original=Note()
assert to_wire(original)=={}
explicit=with_present_fields(original,'note-value')
assert to_wire(explicit)=={'note-value':None} and to_wire(original)=={}
try: with_present_fields(original,'typo')
except ValueError: pass
else: raise AssertionError('unknown presence key accepted')

seen=[]
class Response:
    status=200
    headers=Message()
    headers['content-type']='application/json-seq'
    def __enter__(self): return self
    def __exit__(self,*args): pass
    def read(self): return b'\x1e{"id":1}\n\x1enull\n'
def opened(request,timeout): seen.append(request); return Response()
runtime.urlopen=opened
client=Client('https://api.example', max_retries=0)
result=client._request('POST','/items?tag=a&tag=b',body=[{'id':2},None],body_kind='json-seq')
assert result == [{'id':1},None]
assert seen[0].data == b'\x1e{"id": 2}\n\x1enull\n',seen[0].data
assert seen[0].full_url == 'https://api.example/items?tag=a&tag=b'
client.search(filter='tag=a&tag=b')
assert seen[-1].full_url == 'https://api.example/search?tag=a&tag=b'
client.filter(selector={'id':0},filter={'enabled':False},condition={'id':1},preferences={'id':2})
from urllib.parse import urlsplit,parse_qs,unquote
assert unquote(urlsplit(seen[-1].full_url).path)=='/items/{"id":0}'
assert parse_qs(urlsplit(seen[-1].full_url).query)=={'filter':['{"enabled":false}']}
assert seen[-1].get_header('Condition')=='{"id":1}'
assert unquote(seen[-1].get_header('Cookie'))=='preferences={"id":2}'

assert decode_sequence(b'{"id":3}\nnull\n','application/x-ndjson') == [{'id':3},None]
import asyncio
from types import SimpleNamespace
from example_api_sdk.async_runtime import AsyncBaseClient
class AsyncResponse:
    status_code=200
    headers={'content-type':'application/x-ndjson'}
    async def aread(self): return b'{"id":5}\nnull\n'
    async def aclose(self): pass
class Driver:
    def build_request(self,method,url,**options):
        assert options['content'] == b'\x1e{"id": 6}\n\x1enull\n'
        assert url.endswith('?tag=a&tag=b')
        return SimpleNamespace(method=method,url=url)
    async def send(self,request,stream=False): return AsyncResponse()
async def verify():
    result=await AsyncBaseClient('https://api.example',http_client=Driver(),max_retries=0)._request('POST','/items?tag=a&tag=b',body=[{'id':6},None],body_kind='json-seq')
    assert result == [{'id':5},None]
asyncio.run(verify())
from example_api_sdk.multipart import MultipartBody,JsonPart,FilePart
from email.parser import BytesParser
from email.policy import default
nested=MultipartBody.positional(['nested',None,FilePart(b'\x00\xff')])
plan={'content_type':'multipart/mixed','prefix_encoding':[{'contentType':'application/json'},{'contentType':'multipart/mixed','prefixEncoding':[{'contentType':'text/plain'},{'contentType':'application/json'}],'itemEncoding':{'contentType':'application/octet-stream'}}]}
body=MultipartBody.positional([JsonPart({'id':7}),nested])
encoded,media=body.with_encoding(plan).encode()
message=BytesParser(policy=default).parsebytes(('Content-Type: '+media+'\r\nMIME-Version: 1.0\r\n\r\n').encode()+encoded)
parts=list(message.iter_parts())
assert len(parts)==2 and json.loads(parts[0].get_payload(decode=True))=={'id':7}
children=list(parts[1].iter_parts())
assert children[0].get_payload(decode=True)==b'nested'
assert children[1].get_payload(decode=True)==b'null'
assert children[2].get_payload(decode=True)==b'\x00\xff'
assert body.encode()[1].startswith('multipart/form-data;')

for malformed in [b'\x1e{"bad":\n',b'{}\x1e{}']:
    try: decode_sequence(malformed,'application/json-seq')
    except (ResponseDecodeError,ValueError): pass
    else: raise AssertionError('malformed JSON sequence accepted')
"#;
    let status = Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("sdk/python/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .status()
        .unwrap();
    assert!(status.success());
}
fn byte_boundary_api() -> Api {
    let parameters = (0..80)
        .map(|index| poolster_core::OperationParameter {
            name: format!("queryParameter{index:03}{}", "LongName".repeat(20)),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        })
        .collect::<Vec<_>>();
    Api {
        name: "Byte Probe".into(),
        operations: (0..20)
            .map(|index| Operation {
                id: format!("getItem{index}"),
                path: format!("/items/{index}"),
                parameters: parameters.clone(),
                responses: vec![poolster_core::OperationResponse::json(
                    "200",
                    SchemaValue::new(SchemaKind::String),
                )],
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn native_partitioned_validation_and_errors_preserve_strict_checks_and_public_identity() {
    let mut source = Api {
        name: "Strict Probe".into(),
        ..Default::default()
    };
    source.schemas = (0..600)
        .map(|index| {
            Schema::new(
                format!("Model{index}"),
                SchemaValue::new(SchemaKind::Object {
                    fields: (0..20)
                        .map(|field| Field {
                            name: format!("field{field}"),
                            value: SchemaValue::new(SchemaKind::String),
                            required: true,
                            annotations: Default::default(),
                        })
                        .collect(),
                    additional_properties: AdditionalProperties::Any,
                }),
            )
        })
        .collect();
    source.operations = (0..600)
        .map(|index| Operation {
            id: format!("getItem{index}"),
            path: format!("/items/{index}"),
            responses: vec![
                poolster_core::OperationResponse::json(
                    "200",
                    SchemaValue::new(SchemaKind::Reference {
                        reference: "#/components/schemas/Model599".into(),
                    }),
                ),
                poolster_core::OperationResponse::json(
                    "400",
                    SchemaValue::new(SchemaKind::Reference {
                        reference: "#/components/schemas/Model599".into(),
                    }),
                ),
            ],
            ..Default::default()
        })
        .collect();
    let tree = render_sdk(
        &source,
        "python",
        Some("strict-probe"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    assert!(
        tree.get("python/src/strict_probe/response_shapes/refs_0000.py")
            .is_some()
    );
    assert!(
        tree.get("python/src/strict_probe/errors/chunk_0000.py")
            .is_some()
    );
    assert!(
        tree.iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "py"))
            .all(|(_, value)| value.len() <= 128 * 1024)
    );
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let script = r#"import pickle, typing
from strict_probe import GetItem599Status400Error, ApiError
from strict_probe.runtime import GetItem599Status400Error as RuntimeErrorClass
from strict_probe.models import Model599
from strict_probe.response_validation import PLANS, assert_shape, ResponseDecodeError
assert len(PLANS['refs'])==600 and len(PLANS['operations'])==600
assert RuntimeErrorClass is GetItem599Status400Error
assert typing.get_type_hints(RuntimeErrorClass)['body'] is Model599
wire={f'field{i}':'value' for i in range(20)}
assert_shape(wire, {'ref':'Model599'}, PLANS['refs'])
try: assert_shape({**wire,'field19':42}, {'ref':'Model599'}, PLANS['refs'])
except ResponseDecodeError as error: assert error.path=='$["field19"]'
else: raise AssertionError('strict shape mismatch accepted')
error=GetItem599Status400Error(400,{},wire)
assert isinstance(error,ApiError) and error.__class__.__module__=='strict_probe.runtime'
assert pickle.loads(pickle.dumps(RuntimeErrorClass)) is RuntimeErrorClass
"#;
    let output = std::process::Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("python/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_byte_bounded_operations_preserve_last_resource_and_transport() {
    let source = byte_boundary_api();
    let root = tempfile::tempdir().unwrap();
    let tree = render_sdk(
        &source,
        "python",
        Some("byte-probe"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let chunks = tree
        .iter()
        .filter(|(path, _)| path.to_string_lossy().contains("operations_"))
        .collect::<Vec<_>>();
    assert!(chunks.len() > 1);
    assert!(
        chunks
            .iter()
            .all(|(_, contents)| contents.len() <= 128 * 1024)
    );
    assert!(
        tree.iter()
            .filter(|(path, _)| path.to_string_lossy().contains("_part_"))
            .all(|(_, contents)| contents.len() <= 128 * 1024)
    );
    tree.write_to(root.path()).unwrap();
    let script = r#"from byte_probe import Client
client=Client('https://unused.example')
seen=[]
def transport(method,path,**options):
    seen.append((method,path,options)); return 'custom'
client._request=transport
assert client.get_item19()=='custom'
assert client.items.get_item19()=='custom'
assert [item[1] for item in seen]==['/items/19','/items/19']
"#;
    let output = std::process::Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("python/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
