use super::*;

#[test]
fn native_auto_idempotency_reuses_secure_keys_and_preserves_explicit_values() {
    let mut source = api();
    source.operations.truncate(1);
    let op = &mut source.operations[0];
    op.id = "createContact".into();
    op.path = "/contacts".into();
    op.method = poolster_core::HttpMethod::Post;
    op.parameters = vec![poolster_core::OperationParameter {
        name: "requestKey".into(),
        location: "header".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: Default::default(),
    }];
    op.annotations.insert("x-kaji-idempotency-resolved".into(),serde_json::json!({"header":"X-Request-Key","parameter_name":"requestKey","auto_generate":true}));
    // Normalization preserves the wire header as the actual parameter name.
    op.parameters[0].name = "X-Request-Key".into();
    op.annotations
        .get_mut("x-kaji-idempotency-resolved")
        .unwrap()["parameter_name"] = serde_json::json!("X-Request-Key");
    let mut collision = op.parameters[0].clone();
    collision.name = "X_Request_Key".into();
    collision.location = "query".into();
    op.parameters.insert(0, collision);
    let root = tempfile::tempdir().unwrap();
    render_sdk_with_async(
        &source,
        "python",
        Some("idempotency-sdk"),
        SdkClientStyle::Flat,
        true,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    let script = r#"import uuid, asyncio, io
from email.message import Message
from urllib.error import HTTPError
from idempotency_sdk import Client
from idempotency_sdk.async_client import AsyncClient
import idempotency_sdk.runtime as runtime
seen=[]
class Response:
    status=200
    def __init__(self): self.headers=Message(); self.headers['Content-Type']='application/json'
    def read(self):return b'{"id":"ok"}'
    def __enter__(self):return self
    def __exit__(self,*args):pass
def send(request,**kwargs):
    seen.append(request.get_header('X-request-key'))
    if len(seen)%2==1:raise HTTPError(request.full_url,503,'retry',Message(),io.BytesIO(b'{}'))
    return Response()
runtime.urlopen=send
client=Client('https://example.invalid',max_retries=1,retry_initial_delay=0,retry_max_delay=0)
client.create_contact(x_request_key='query-value');assert seen[0]==seen[1] and uuid.UUID(seen[0]).version==4
delays=[];runtime.time.sleep=delays.append
client.retry_max_delay=.4
client._retry_delay(0,'2','250');assert delays.pop()==.25
client._retry_delay(0,'2','99999');assert delays.pop()==.4
client._retry_delay(0,'.2','invalid');assert delays.pop()==.2
client._retry_delay(0,'2','0');assert delays.pop()==0
client.retry_max_delay=0
first=seen[0];client.create_contact();assert seen[2]==seen[3] and seen[2]!=first
client.create_contact(x_request_key_='provided');assert seen[-2:]==['provided','provided']
# The same header on an unconfigured operation cannot confer retry safety.
seen.clear()
try: client._request('POST','/contacts',headers={'X-Request-Key':'untrusted'},retryable=True)
except Exception: pass
assert len(seen)==1
seen.clear()
try: client.create_contact(x_request_key_='')
except Exception: pass
assert seen==['']
class AsyncResponse:
    def __init__(self,status):self.status_code=status;self.headers={'content-type':'application/json'};self.content=b'{"id":"ok"}';self.text=self.content.decode()
    async def aclose(self):pass
    async def aread(self):return self.content
class Driver:
    def build_request(self,method,url,**kwargs):return kwargs['headers']
    async def send(self,request,**kwargs):
        seen.append(request['X-Request-Key']);return AsyncResponse(503 if len(seen)%2 else 200)
async def run():
    seen.clear();client=AsyncClient('https://example.invalid',http_client=Driver(),max_retries=1,retry_initial_delay=0,retry_max_delay=0)
    await client.create_contact();assert seen[0]==seen[1] and uuid.UUID(seen[0]).version==4
    await client.create_contact(x_request_key_='provided');assert seen[-2:]==['provided','provided']
    seen.clear()
    try: await client.create_contact(x_request_key_='')
    except Exception: pass
    assert seen==['']
    delays=[]
    async def record(delay):delays.append(delay)
    asyncio.sleep=record;client.retry_max_delay=.4
    await client._retry_delay_async(0,'2','250');assert delays.pop()==.25
    await client._retry_delay_async(0,'2','99999');assert delays.pop()==.4
    await client._retry_delay_async(0,'.2','nan');assert delays.pop()==.2
    await client._retry_delay_async(0,'2','0');assert delays.pop()==0

asyncio.run(run())
"#;
    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(script)
        .env("PYTHONPATH", root.path().join("python/src"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn native_forward_union_aliases_allow_recursive_model_imports() {
    let mut source = api();
    source.schemas.push(Schema::new(
        "Event",
        SchemaValue::new(SchemaKind::OneOf {
            variants: vec![
                SchemaValue::reference("#/components/schemas/Contact"),
                SchemaValue::reference("#/components/schemas/EventList"),
            ],
        }),
    ));
    source.schemas.push(Schema::new(
        "EventList",
        SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::reference("#/components/schemas/Event")),
        }),
    ));
    let root = tempfile::tempdir().unwrap();
    render_sdk(&source, "sdk", Some("probe-sdk"), SdkClientStyle::Flat)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let output = std::process::Command::new("python3")
            .args(["-c", "from probe_sdk.models import Event, EventList, Contact; assert Event == 'Contact | EventList'; assert EventList == 'list[Event]'; assert Contact.from_dict({'id': 'one'}).id == 'one'"])
            .env("PYTHONPATH", root.path().join("sdk/src"))
            .output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_nested_reference_arrays_decode_models_and_round_trip_wire() {
    let mut source = api();
    source.schemas.push(Schema::new(
        "ContactPage",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "items".into(),
                required: true,
                annotations: Default::default(),
                value: SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::reference("#/components/schemas/Contact")),
                }),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    let root = tempfile::tempdir().unwrap();
    render_sdk(&source, "sdk", Some("probe-sdk"), SdkClientStyle::Flat)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"from probe_sdk.models import ContactPage,Contact
from probe_sdk.runtime import to_wire
wire={'items':[{'id':'one'},{'id':'two','display-name':None}]}
page=ContactPage.from_dict(wire)
assert all(isinstance(item,Contact) for item in page.items)
assert [item.id for item in page.items]==['one','two'] and to_wire(page)==wire
"#;
    let output = std::process::Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn native_call_scopes_preserve_headers_timeouts_and_shared_async_driver() {
    let root = tempfile::tempdir().unwrap();
    render_sdk_with_async(
        &api(),
        "sdk/python",
        Some("example-api-sdk"),
        SdkClientStyle::Flat,
        true,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    let output = Command::new("python3")
        .args([
            "-c",
            include_str!("../../tests/fixtures/call_options_probe.py.txt"),
        ])
        .env("PYTHONPATH", root.path().join("sdk/python/src"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
