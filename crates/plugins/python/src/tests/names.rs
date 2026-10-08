use super::*;

#[test]
fn native_normalized_names_preserve_distinct_wire_fields_and_arguments() {
    let mut source = Api {
        name: "Collision".into(),
        ..Default::default()
    };
    source.schemas.push(Schema::new(
        "Probe",
        SchemaValue::new(SchemaKind::Object {
            fields: ["+1", "-1", "field", "x-axis", "x_axis"]
                .iter()
                .map(|name| poolster_core::Field {
                    name: (*name).into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: false,
                    annotations: Default::default(),
                })
                .collect(),
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    source.schemas.push(Schema::new(
        "Nested/Model",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "name".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: true,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    source.schemas.push(Schema::new(
        "Holder",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "nested".into(),
                value: SchemaValue::reference("#/components/schemas/Nested~1Model"),
                required: true,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    source.operations.push(Operation {
        id: "probe".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/{id}".into(),
        parameters: ["path", "query", "header"]
            .iter()
            .map(|location| poolster_core::OperationParameter {
                name: "id".into(),
                location: (*location).into(),
                required: true,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: Default::default(),
            })
            .collect(),
        request_body: None,
        responses: vec![],
        security: vec![],
        annotations: Default::default(),
    });
    let root = tempfile::tempdir().unwrap();
    render_sdk(
        &source,
        "sdk",
        Some("collision-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    let script = r#"import inspect
from collision_sdk.models import Probe, Holder, NestedModel
from collision_sdk.models import _to_wire
from collision_sdk import Client
wire={'+1':'positive','-1':'negative','field':'safe','x-axis':'dash','x_axis':'underscore'}
model=Probe.from_dict(wire)
assert _to_wire(model)==wire
holder=Holder.from_dict({'nested': {'name': 'value'}})
assert isinstance(holder.nested, NestedModel) and _to_wire(holder)=={'nested': {'name': 'value'}}
assert {'id', 'id_'} <= set(inspect.signature(Client.probe).parameters)
seen=[]
client=Client('https://example.invalid')
client._request=lambda *args, **kwargs: seen.append((args, kwargs))
client.probe(id='path value', id_='query value', id__='header value')
assert seen[0][0][1]=='/path%20value' and seen[0][1]['query']=={'id':'query value'} and seen[0][1]['headers']=={'id':'header value'}
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
fn consumer_plugins_execute_roundtrip_fixtures_and_webhook_vectors() {
    use poolster_core::engine::Packages;
    let mut source = api();
    if let SchemaKind::Object {
        fields,
        additional_properties,
    } = &mut source.schemas[0].value.kind
    {
        fields[1].value.nullable = true;
        fields.push(Field {
            name: "additional_properties".into(),
            value: SchemaValue::new(SchemaKind::String),
            required: false,
            annotations: Default::default(),
        });
        *additional_properties = AdditionalProperties::Any;
    }
    let sdk = crate::sdk();
    let roundtrips = crate::roundtrips().models_from(&sdk);
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("example-api-sdk")
                .with(roundtrips)
                .with(crate::webhooks())
                .with(sdk),
        )
        .generate(&source, None)
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let status = Command::new("python3")
        .arg(root.path().join("sdk/tests/test_model_roundtrips.py"))
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::write(
        root.path().join("vector.json"),
        include_str!("../../testdata/webhook-vectors.json"),
    )
    .unwrap();
    let script = r#"import json, sys
from example_api_sdk.webhooks import verify_webhook, WebhookVerificationError, verify_and_decode
from example_api_sdk.models import Contact
vector = json.load(open(sys.argv[1]))
raw = vector['raw_body'].encode(); headers = vector['headers']; secret = vector['secret']; now = vector['now']
assert verify_webhook(raw, headers, [secret], now=now) == vector['payload']
rotated = {**headers, 'webhook-signature': 'v1,invalid v2,ignored '+headers['webhook-signature']}
assert verify_webhook(raw, rotated, [secret], now=now) == vector['payload']
for changed_body, changed_headers, changed_now in [(raw+b' ', headers, now), (raw, headers, now+301), (raw, headers, now-301), (raw, {**headers, 'webhook-id':'msg.evil'}, now), (raw, {**headers, 'webhook-signature':'v1,invalid'}, now)]:
    try: verify_webhook(changed_body, changed_headers, [secret], now=changed_now)
    except WebhookVerificationError: pass
    else: raise AssertionError('invalid webhook accepted')
"#;
    let status = Command::new("python3")
        .args(["-c", script])
        .arg(root.path().join("vector.json"))
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .status()
        .unwrap();
    assert!(status.success());
}
