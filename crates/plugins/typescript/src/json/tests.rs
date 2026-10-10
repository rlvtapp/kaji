use super::*;
use poolster_core::{
    Field, HttpMethod, Operation, OperationMediaType, OperationRequestBody, OperationResponse,
    Schema, engine::Packages,
};
fn api() -> Api {
    let mut id = SchemaValue::new(SchemaKind::Integer);
    id.format = Some("int64".into());
    let fields = [
        ("id", id.clone()),
        ("count", SchemaValue::new(SchemaKind::Integer)),
        ("ratio", SchemaValue::new(SchemaKind::Number)),
        ("label", SchemaValue::new(SchemaKind::String)),
        (
            "ids",
            SchemaValue::new(SchemaKind::Array {
                items: Box::new(id),
            }),
        ),
    ]
    .into_iter()
    .map(|(name, value)| Field {
        name: name.into(),
        required: true,
        value,
        annotations: Default::default(),
    })
    .collect();
    let record = SchemaValue::new(SchemaKind::Object {
        fields,
        additional_properties: AdditionalProperties::Forbidden,
    });
    let reference = SchemaValue::reference("#/components/schemas/Record");
    Api {
        name: "Record API".into(),
        version: "1.0.0".into(),
        schemas: vec![Schema::new("Record", record)],
        operations: vec![
            Operation {
                id: "echoRecord".into(),
                method: HttpMethod::Post,
                path: "/records".into(),
                request_body: Some(OperationRequestBody {
                    required: true,
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "application/json".into(),
                        schema: Some(reference.clone()),
                    }],
                }),
                responses: vec![OperationResponse::json("200", reference.clone())],
                ..Default::default()
            },
            Operation {
                id: "streamRecords".into(),
                method: HttpMethod::Get,
                path: "/stream".into(),
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "text/event-stream".into(),
                        schema: Some(reference),
                    }],
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}
#[test]
fn default_number_operations_also_publish_response_shapes() {
    let api = api();
    let plan = operation_plan(&api, &api.operations[0], &ModelOptions::default()).unwrap();
    assert!(plan["responses"]["200"]["application/json"].is_object());
    assert!(plan["refs"].as_object().unwrap().contains_key("Record"));
}

#[test]
fn operation_count_does_not_multiply_shared_schema_descriptors() {
    let mut api = api();
    for index in 0..50 {
        let mut operation = api.operations[0].clone();
        operation.id = format!("echoRecord{index}");
        api.operations.push(operation);
    }
    let tree = Packages::new()
        .package(crate::package("ts").with(crate::sdk().raw().group_by_tag(false)))
        .generate(&api, None)
        .unwrap();
    let shared = tree.get("ts/clients/_poolster_json_refs_0001.ts").unwrap();
    assert!(shared.contains("\"Record\""));
    assert!(shared.contains("\"integer\""));
    for (path, source) in tree.iter() {
        if path.starts_with("ts/clients") && source.contains("jsonPlan:") {
            assert!(source.contains("poolsterJsonRefs"), "{}", path.display());
            assert!(!source.contains("\"integer\""), "{}", path.display());
            assert!(source.len() < 4_000, "{}: {}", path.display(), source.len());
        }
    }
}

#[test]
fn integer_models_and_wire_plans_agree() {
    for (representation, expected) in [
        (Int64Type::String, "id: string"),
        (Int64Type::BigInt, "id: bigint"),
    ] {
        let tree = Packages::new()
            .package(
                crate::package("ts").with(crate::sdk().raw().group_by_tag(false).model_options(
                    ModelOptions {
                        int64_type: representation,
                        ..Default::default()
                    },
                )),
            )
            .generate(&api(), None)
            .unwrap();
        let source = tree.get("ts/models/Record.ts").unwrap();
        assert!(source.contains(expected), "{source}");
        assert!(source.contains("count: number"));
        assert!(
            tree.get("ts/clients/echoRecord.ts")
                .unwrap()
                .contains("jsonPlan:")
        );
        assert!(
            tree.get("ts/clients/streamRecords.ts")
                .unwrap()
                .contains("\"text/event-stream\"")
        );
    }
}
#[test]
#[ignore = "requires Node, POOLSTER_TSC_JS, and POOLSTER_AXIOS_NODE_MODULES"]
fn generated_lossless_fetch_axios_and_sse_round_trip() {
    let compiler = std::env::var("POOLSTER_TSC_JS").unwrap();
    let modules = std::env::var("POOLSTER_AXIOS_NODE_MODULES").unwrap();
    let directory = std::env::temp_dir().join(format!("poolster-json-{}", std::process::id()));
    let mut packages = Packages::new();
    for (name, representation, axios) in [
        ("big", Int64Type::BigInt, false),
        ("string", Int64Type::String, false),
        ("axios", Int64Type::BigInt, true),
    ] {
        let mut sdk = crate::sdk()
            .raw()
            .group_by_tag(false)
            .model_options(ModelOptions {
                int64_type: representation,
                ..Default::default()
            });
        if axios {
            sdk = sdk.axios();
        }
        packages = packages.package(crate::package(name).with(sdk));
    }
    let tree = packages.generate(&api(), None).unwrap();
    tree.write_to(&directory).unwrap();
    for package in ["big", "string", "axios"] {
        #[cfg(unix)]
        std::os::unix::fs::symlink(&modules, directory.join(package).join("node_modules")).unwrap();
        let result = std::process::Command::new("node")
            .arg(&compiler)
            .args(["--project"])
            .arg(directory.join(package).join("tsconfig.json"))
            .args([
                "--module",
                "commonjs",
                "--moduleResolution",
                "node",
                "--outDir",
                "compiled",
            ])
            .current_dir(directory.join(package))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        std::fs::write(
            directory.join(package).join("compiled/package.json"),
            "{\"type\":\"commonjs\"}",
        )
        .unwrap();
    }
    std::fs::write(directory.join("test.cjs"),r#"
const assert = require('node:assert/strict');
const wire = '{"id":9223372036854775807,"count":7,"ratio":2,"label":"quoted \\" digits 123","ids":[-9223372036854775808,9223372036854775807]}';
const inputWire = JSON.parse(wire);
async function run(name, integer) {
  const { echoRecord } = require(`./${name}/compiled/clients/echoRecord.js`);
  const { createClient, parseJson, stringifyJson } = require(`./${name}/compiled/.poolster/client.js`);
  const body = { id: integer('9223372036854775807'), count: 7, ratio: 2, label: inputWire.label, ids: [integer('-9223372036854775808'), integer('9223372036854775807')] };
  let requestBody;
  const transport = name === 'axios'
    ? createClient({ validateResponses:true, client: { request: async config => { requestBody = config.data; assert.equal(config.responseType,'text'); assert.ok(config.transformResponse); return { status:200, headers:{'content-type':'application/json'}, data:wire }; } } })
    : createClient({ validateResponses:true, fetch: async (_,config) => { requestBody = config.body; return new Response(wire,{headers:{'content-type':'application/json'}}); } });
  const result = await echoRecord({body,client:transport});
  assert.equal(result.id, integer('9223372036854775807'));
  assert.equal(result.ids[0], integer('-9223372036854775808'));
  assert.equal(result.count,7); assert.equal(result.ratio,2); assert.equal(result.label,inputWire.label);
  const malformed = createClient({validateResponses:true,middleware:[async()=>({status:200,contentType:'application/json',data:{...result,count:'wrong'}})]});
  await assert.rejects(()=>echoRecord({body,client:malformed}),error=>error.name==='ResponseDecodeError' && error.path.includes('count'));
  assert.match(requestBody,/"id":9223372036854775807/); assert.match(requestBody,/-9223372036854775808/);
  assert.equal(JSON.parse(requestBody).label,inputWire.label);
  assert.equal(parseJson('{"__proto__":{"polluted":true},"n":12}').n,12);
  assert.equal({}.polluted,undefined);
  assert.equal(parseJson('9.223372036854775807e18',{integer:'bigint'}),9223372036854775807n);
  assert.equal(parseJson('-9223372036854775808.0',{integer:'string'}),'-9223372036854775808');
  assert.throws(()=>parseJson('1.5',{integer:'bigint'}),/Fractional/);
  assert.equal(parseJson('1e3',{kind:'union',variants:[{kind:'string'},{kind:'integer',integer:'bigint'}]}),1000n);
  const additional = parseJson('{"any":1,"other":9223372036854775807}',{kind:'object',fields:{any:null},additional:{kind:'integer',integer:'bigint'}}); assert.equal(additional.any,1); assert.equal(additional.other,9223372036854775807n);
  assert.throws(()=>stringifyJson(9007199254740992,{integer:'bigint'}),/exact string or bigint/);
  {
    const { streamRecords } = require(`./${name}/compiled/clients/streamRecords.js`);
    const streamClient = name === 'axios' ? createClient({client:{request:async()=>({status:200,headers:{'content-type':'text/event-stream'},data:new Response(`data: ${wire}\n\n`).body})}}) : createClient({fetch:async()=>new Response(`data: ${wire}\n\n`,{headers:{'content-type':'text/event-stream'}})});
    const stream = await streamRecords({client:streamClient});
    for await (const record of stream) assert.equal(record.id,integer('9223372036854775807'));
  }
}
(async()=> { await run('big',BigInt); await run('string',String); await run('axios',BigInt); })().catch(error=> {console.error(error);process.exitCode=1;});
"#).unwrap();
    let result = std::process::Command::new("node")
        .arg(directory.join("test.cjs"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
