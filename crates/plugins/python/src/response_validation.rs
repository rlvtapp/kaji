//! Generated opt-in structural response checks for Python sync and async clients.
use super::*;
use serde_json::json;
fn write_only(
    api: &Api,
    value: &SchemaValue,
    seen: &mut std::collections::BTreeSet<String>,
) -> bool {
    if value.write_only {
        return true;
    }
    if let SchemaKind::Reference { reference } = &value.kind {
        let name = reference.rsplit('/').next().unwrap_or(reference);
        if seen.insert(name.into()) {
            return api
                .schemas
                .iter()
                .find(|s| s.name == name)
                .is_some_and(|s| write_only(api, &s.value, seen));
        }
    }
    false
}
fn shape(api: &Api, value: &SchemaValue) -> Value {
    let mut result = match &value.kind {
        SchemaKind::Reference { reference } => {
            json!({"ref":reference.rsplit('/').next().unwrap_or(reference)})
        }
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let fields_map = fields
                .iter()
                .map(|f| (f.name.clone(), shape(api, &f.value)))
                .collect::<serde_json::Map<_, _>>();
            let required = fields
                .iter()
                .filter(|f| f.required && !write_only(api, &f.value, &mut Default::default()))
                .map(|f| &f.name)
                .collect::<Vec<_>>();
            let additional = if let AdditionalProperties::Schema { value } = additional_properties {
                shape(api, value)
            } else {
                Value::Null
            };
            json!({"kind":"object","fields":fields_map,"required":required,"additional":additional})
        }
        SchemaKind::Array { items } => json!({"kind":"array","items":shape(api,items)}),
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            json!({"kind":if matches!(value.kind,SchemaKind::AllOf {..}) {"allOf"} else {"union"},"variants":variants.iter().map(|v|shape(api,v)).collect::<Vec<_>>()})
        }
        SchemaKind::String => json!({"kind":"string"}),
        SchemaKind::Boolean => json!({"kind":"boolean"}),
        SchemaKind::Integer => json!({"kind":"integer"}),
        SchemaKind::Number => json!({"kind":"number"}),
        SchemaKind::Null => json!({"kind":"null"}),
        _ => json!({"kind":"any"}),
    };
    result["nullable"] = json!(value.nullable || value.nullish);
    result
}
pub(super) fn render(api: &Api) -> String {
    let refs = api
        .schemas
        .iter()
        .map(|s| (s.name.clone(), shape(api, &s.value)))
        .collect::<serde_json::Map<_, _>>();
    let operations = api
        .operations
        .iter()
        .map(|op| {
            let statuses = op
                .responses
                .iter()
                .map(|r| {
                    let media = r
                        .media_types
                        .iter()
                        .filter_map(|m| {
                            m.schema
                                .as_ref()
                                .map(|s| (m.content_type.clone(), shape(api, s)))
                        })
                        .collect::<serde_json::Map<_, _>>();
                    (r.status.clone(), Value::Object(media))
                })
                .collect::<serde_json::Map<_, _>>();
            (op.id.clone(), Value::Object(statuses))
        })
        .collect::<serde_json::Map<_, _>>();
    // A JSON string literal is also a valid Python string literal, while JSON booleans
    // are only parsed by json.loads rather than emitted as Python expressions.
    let encoded = serde_json::to_string(&json!({"refs":refs,"operations":operations})).unwrap();
    format!(
        "{}\nPLANS = json.loads({})\n",
        include_str!("response_validation.py"),
        serde_json::to_string(&encoded).unwrap()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sync_and_async_generated_operations_reject_malformed_middleware_responses() {
        use kaji_core::{Field, HttpMethod, OperationResponse, Schema};
        let api = Api {
            name: "Strict".into(),
            version: "1.0.0".into(),
            schemas: vec![Schema::new(
                "Item",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![Field {
                        name: "id".into(),
                        required: true,
                        value: SchemaValue::new(SchemaKind::Integer),
                        annotations: Default::default(),
                    }],
                    additional_properties: AdditionalProperties::Any,
                }),
            )],
            operations: vec![Operation {
                id: "getItem".into(),
                method: HttpMethod::Get,
                path: "/items".into(),
                responses: vec![OperationResponse::json(
                    "200",
                    SchemaValue::reference("#/components/schemas/Item"),
                )],
                ..Default::default()
            }],
            ..Default::default()
        };
        let root = tempfile::tempdir().unwrap();
        crate::render_sdk_with_async(&api, "sdk", Some("strict-sdk"), SdkClientStyle::Flat, true)
            .unwrap()
            .write_to(root.path())
            .unwrap();
        let script = r#"import asyncio, io, json
from email.message import Message
from strict_sdk import Client, AsyncClient, ResponseDecodeError
from strict_sdk.response_validation import assert_shape, decode_json
class Response(io.BytesIO):
    status=200
    def __init__(self,raw):
        super().__init__(raw); self.headers=Message(); self.headers['content-type']='application/json'; self.close_calls=0
    def close(self): self.close_calls+=1; super().close()
for raw in [b'{"id":"secret-payload"}',b'{}',b'{"id":true}',b'{"id":null}',b'{"id":1} {}',b'{"id":NaN}']:
    responses=[]; failures=[]
    def cached(request,next):
        response=Response(raw); responses.append(response); return response
    client=Client('https://unused.test',validate_responses=True,middleware=(cached,),on_error=lambda error,_:failures.append(error))
    try: client.get_item(); raise AssertionError('accepted invalid response')
    except ResponseDecodeError as error:
        assert error.path.startswith('$') and 'secret-payload' not in str(error)
    assert len(responses)==1 and responses[0].close_calls==1 and len(failures)==1
valid=Client('https://unused.test',validate_responses=True,middleware=(lambda request,next:Response(b'{"id":42,"future":true}'),)).get_item()
assert valid.id==42
assert Client('https://unused.test',middleware=(lambda request,next:Response(b'{"id":"compatible"}'),)).get_item().id=='compatible'
assert_shape(None,{'kind':'union','variants':[{'kind':'string'},{'kind':'null'}]}, {})
assert_shape('future',{'kind':'string','literals':['known']},{})
class AsyncResponse:
    status_code=200; headers={'content-type':'application/json'}
    def __init__(self,raw): self.raw=raw;self.close_calls=0
    async def aread(self): return self.raw
    async def aclose(self): self.close_calls+=1
class Transport:
    def build_request(self,*args,**kwargs): return kwargs
    async def send(self,*args,**kwargs): raise AssertionError('middleware should short circuit')
async def run():
    for raw in [b'{"id":"wrong"}',b'{"id":false}',b'{}',b'{"id":Infinity}']:
        response=AsyncResponse(raw);failures=[]
        async def cached(request,next): return response
        client=AsyncClient('https://unused.test',http_client=Transport(),validate_responses=True,async_middleware=(cached,),on_error=lambda error,_:failures.append(error))
        try: await client.get_item();raise AssertionError('accepted invalid async response')
        except ResponseDecodeError: pass
        assert response.close_calls==1 and len(failures)==1
asyncio.run(run())
"#;
        let status = std::process::Command::new("python3")
            .args(["-c", script])
            .env("PYTHONPATH", root.path().join("sdk/src"))
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .status()
            .unwrap();
        assert!(status.success());
    }
}
