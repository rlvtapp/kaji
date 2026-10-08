//! Bounded native operation tests; native transport ABI is verified before emission.
use crate::{
    TypeScript,
    composition::{Models, Operations, Transport},
    workspace::Symbol,
};
use anyhow::Result;
use poolster_core::{
    Api, GeneratedFile, Operation, SchemaKind, SchemaValue,
    engine::{Handle, Meta, Plugin, PluginContext, Requirement},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write;
pub struct OperationTests {
    meta: Meta,
    models: Option<Handle<Models>>,
    operations: Option<Handle<Operations>>,
    transport: Option<Handle<Transport>>,
    options: poolster_core::samples::SampleOptions,
    max_operations: usize,
}
pub fn operation_tests() -> OperationTests {
    OperationTests {
        meta: Meta::new(),
        models: None,
        operations: None,
        transport: None,
        options: Default::default(),
        max_operations: 128,
    }
}
impl OperationTests {
    pub fn using_models(mut self, handle: Handle<Models>) -> Self {
        self.models = Some(handle);
        self
    }
    pub fn using_operations(mut self, handle: Handle<Operations>) -> Self {
        self.operations = Some(handle);
        self
    }
    pub fn using_transport(mut self, handle: Handle<Transport>) -> Self {
        self.transport = Some(handle);
        self
    }
    pub fn sample_options(mut self, options: poolster_core::samples::SampleOptions) -> Self {
        self.options = options;
        self
    }
    pub fn max_operations(mut self, limit: usize) -> Self {
        self.max_operations = limit;
        self
    }
}
fn sample(
    api: &Api,
    schema: &SchemaValue,
    options: poolster_core::samples::SampleOptions,
) -> std::result::Result<Value, String> {
    let report = poolster_core::samples::schema_samples(api, schema, options);
    report
        .samples
        .into_iter()
        .next()
        .map(|sample| sample.value)
        .ok_or_else(|| format!("no bounded sample: {}", report.diagnostics.join("; ")))
}
fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}
fn render(
    api: &Api,
    op: &Operation,
    symbol: &Symbol,
    index: usize,
    options: poolster_core::samples::SampleOptions,
    axios: bool,
) -> std::result::Result<String, String> {
    if !op.security.is_empty() {
        return Err("secured operation needs an authentication fixture adapter".into());
    }
    let mut input = json!({});
    let mut path = quote(&op.path);
    let mut checks = String::new();
    for parameter in &op.parameters {
        let schema = parameter
            .schema
            .as_ref()
            .ok_or("parameter schema unavailable")?;
        if matches!(schema.format.as_deref(), Some("byte" | "binary"))
            || !matches!(
                schema.kind,
                SchemaKind::String | SchemaKind::Boolean | SchemaKind::Integer | SchemaKind::Number
            )
        {
            return Err("non-scalar or referenced parameter needs an adapter".into());
        }
        let mut value = sample(api, schema, options)?;
        if parameter.location == "path"
            && matches!(schema.kind, SchemaKind::String)
            && schema.enum_values.is_empty()
        {
            value = Value::String("fixture/雪".into());
        }
        let encoded = match &value {
            Value::String(value) => value.clone(),
            Value::Bool(value) => value.to_string(),
            Value::Number(value) => value.to_string(),
            _ => return Err("nullable parameter needs an adapter".into()),
        };
        let group = match parameter.location.as_str() {
            "path" => "path",
            "query" => "query",
            "header" => "headers",
            _ => return Err("parameter location needs an adapter".into()),
        };
        if input.get(group).is_none() {
            input[group] = json!({})
        }
        input[group][&parameter.name] = value;
        let name = quote(&parameter.name);
        let expected = quote(&encoded);
        match parameter.location.as_str() {
            "path" => {
                path = format!(
                    "{path}.replace({},encodeURIComponent({expected}))",
                    quote(&format!("{{{}}}", parameter.name))
                )
            }
            "query" => {
                writeln!(
                    checks,
                    "check(url.searchParams.get({name}) === {expected}, 'query input');"
                )
                .unwrap();
            }
            "header" => {
                writeln!(
                    checks,
                    "check(headers.get({name}) === {expected}, 'header input');"
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
    }
    if let Some(body) = &op.request_body {
        let media = body
            .media_types
            .iter()
            .find(|media| media.content_type == "application/json")
            .ok_or("non-JSON request body needs an adapter")?;
        input["body"] = sample(
            api,
            media.schema.as_ref().ok_or("body schema unavailable")?,
            options,
        )?;
        if input["body"].is_null() {
            return Err("nullable body root needs an adapter".into());
        }
        writeln!(
            checks,
            "same(JSON.parse(body as string), {}, 'request JSON');",
            input["body"]
        )
        .unwrap();
    }
    let response = op
        .responses
        .iter()
        .find(|response| {
            response
                .status
                .parse::<u16>()
                .is_ok_and(|status| (200..300).contains(&status))
        })
        .ok_or("explicit successful status unavailable")?;
    let status = response.status.parse::<u16>().unwrap();
    let expected = if response.media_types.is_empty() {
        None
    } else {
        let media = response
            .media_types
            .iter()
            .find(|media| media.content_type == "application/json")
            .ok_or("stream/text/binary response needs an adapter")?;
        Some(sample(
            api,
            media.schema.as_ref().ok_or("response schema unavailable")?,
            options,
        )?)
    };
    let payload = expected.clone().unwrap_or(Value::Null);
    let method = quote(op.method.as_str());
    let fetch_payload = if matches!(status, 204 | 205) {
        "null".to_owned()
    } else {
        format!("JSON.stringify({payload})")
    };
    let transport = if axios {
        format!(
            "const instance=axios.create({{adapter:async config=>{{ const url=new URL(String(config.url), 'https://unused.example'); for(const [name,value]of Object.entries(config.params??{{}}))url.searchParams.set(name,String(value));const headers=new Headers(Object.fromEntries(Object.entries(config.headers.toJSON()).map(([name,value])=>[name,String(value)])));const body=config.data; inspect(String(config.method).toUpperCase(),url,headers,body);return {{status:{status},data:{payload},headers:{{'content-type':'application/json'}},config,statusText:'sample'}} }} }});const client=createClient({{client:instance,retry:false}});"
        )
    } else {
        format!(
            "const client=createClient({{baseUrl:'https://unused.example',retry:false,fetch:async(url,init)=>{{inspect(init?.method??'GET',new URL(String(url)),new Headers(init?.headers),init?.body);return new Response({fetch_payload},{{status:{status},headers:{{'content-type':'application/json'}}}})}}}});"
        )
    };
    let response_check = expected
        .map(|value| format!("same(result,{value},'decoded JSON');"))
        .unwrap_or_default();
    let module = symbol
        .import_from("tests/operation-tests.ts")
        .map_err(|error| error.to_string())?;
    let input_fields = input
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, value)| format!("{}:{value}", quote(key)))
        .collect::<Vec<_>>()
        .join(",");
    let input_fields = if input_fields.is_empty() {
        input_fields
    } else {
        input_fields + ","
    };
    Ok(format!(
        "import {{ {} as operation{index} }} from {};\nasync function test{index}() {{let calls=0;const inspect=(method:string,url:URL,headers:Headers,body:unknown)=>{{calls++;check(method==={method},'method');check(url.pathname==={path},'path');{checks}}};{transport}const result=await operation{index}({{{input_fields}client,throwOnError:true}});check(calls===1,'one native attempt');{response_check}}}\n",
        symbol.name,
        quote(&module)
    ))
}
impl Plugin<TypeScript> for OperationTests {
    fn kind(&self) -> &'static str {
        "typescript-operation-tests"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![
            Requirement::on(self.models),
            Requirement::on(self.operations),
            Requirement::on(self.transport),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let models = cx.inputs.get::<Models>()?;
        let operations = cx.inputs.get::<Operations>()?;
        let transport = cx.inputs.get::<Transport>()?;
        let native = cx
            .workspace
            .native_transports
            .get(&transport.module)
            .copied();
        let module = Symbol {
            module: transport.module.clone(),
            name: "createClient".into(),
        }
        .import_from("tests/operation-tests.ts")?;
        let mut source = String::from("// Generated by Poolster. Do not edit.\n");
        if native.is_some() {
            source.push_str(&format!(
                "import {{createClient}} from {};\n",
                quote(&module)
            ));
        }
        if native == Some(crate::sdk::SdkTransport::Axios) {
            source.push_str("import axios from 'axios';\n")
        }
        source.push_str("function check(value:boolean,message:string):asserts value{if(!value)throw new Error(message)}\nfunction normalize(value:unknown):unknown{if(Array.isArray(value))return value.map(normalize);if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).sort(([a],[b])=>a.localeCompare(b)).map(([key,value])=>[key,normalize(value)]));return value}\nfunction same(actual:unknown,expected:unknown,message:string){check(JSON.stringify(normalize(actual))===JSON.stringify(normalize(expected)),message)}\n");
        let mut skipped = BTreeMap::new();
        let mut tests = vec![];
        for (index, operation) in cx.api.operations.iter().enumerate() {
            let rendered = if index >= self.max_operations {
                Err("operation bound reached".into())
            } else if models.options.integer_as_string
                || models.options.int64_type != crate::Int64Type::Number
            {
                Err("lossless/string integer model representation needs a fixture adapter".into())
            } else if native.is_none() {
                Err("custom transport needs a deterministic native test adapter".into())
            } else {
                operations
                    .functions
                    .get(&operation.id)
                    .ok_or_else(|| "operation unavailable".into())
                    .and_then(|symbol| {
                        render(
                            cx.api,
                            operation,
                            symbol,
                            index,
                            self.options,
                            native == Some(crate::sdk::SdkTransport::Axios),
                        )
                    })
            };
            match rendered {
                Ok(test) => {
                    source.push_str(&test);
                    tests.push(format!("await test{index}();"));
                }
                Err(reason) => {
                    skipped.insert(operation.id.clone(), reason);
                }
            }
        }
        for (id, reason) in &skipped {
            source.push_str(&format!(
                "console.info({});\n",
                quote(&format!("SKIP {id}: {reason}"))
            ));
        }
        source.push_str(&format!("async function main(){{{}}}\nvoid main().catch(error=>{{console.error(error);throw error}});\n",tests.join("\n")));
        cx.files
            .emit(GeneratedFile::new("tests/operation-tests.ts", source)?)?;
        cx.files.emit(GeneratedFile::new(
            ".poolster/operation-test-diagnostics.json",
            serde_json::to_string_pretty(&json!({"generated":tests.len(),"skipped":skipped}))?
                + "\n",
        )?)?;
        cx.files.emit(GeneratedFile::new("OPERATION_TESTS.md",format!("# Generated operation tests\n\nCompile with `npx tsc -p tsconfig.json`, then run `node dist/tests/operation-tests.js`. {} bounded operation tests use the native Fetch/Axios driver with an in-memory response; no network is used. Inspect `.poolster/operation-test-diagnostics.json` for {} unsupported operations. Tests assert one attempt, method/path escaping, scalar query/header serialization, JSON body encoding and decoded JSON or void success. This is a structural serialization smoke suite, not complete service/schema coverage. Fixtures use structural samples rather than source examples, with a reserved Unicode path sentinel for unconstrained string paths. Security requirements, non-scalar/referenced parameters, non-JSON media, nullable root bodies, lossless integer model modes, unknown community transport ABIs and exhausted bounds require adapters and are reported explicitly.\n",tests.len(),skipped.len()))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PackageExt;
    use poolster_core::{
        AdditionalProperties, Field, HttpMethod, OperationMediaType, OperationParameter,
        OperationRequestBody, OperationResponse, Schema, engine::Packages,
    };
    fn api() -> Api {
        let object = SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "id".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: true,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        });
        let mut operation = Operation {
            id: "createWidget".into(),
            method: HttpMethod::Post,
            path: "/widgets/{id}".into(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::reference("#/components/schemas/Widget"),
            )],
            request_body: Some(OperationRequestBody {
                required: true,
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::reference("#/components/schemas/Widget")),
                }],
            }),
            ..Default::default()
        };
        for (name, location, kind) in [
            ("id", "path", SchemaKind::String),
            ("count", "query", SchemaKind::Integer),
            ("X-Flag", "header", SchemaKind::Boolean),
        ] {
            operation.parameters.push(OperationParameter {
                name: name.into(),
                location: location.into(),
                required: true,
                schema: Some(SchemaValue::new(kind)),
                description: None,
                annotations: Default::default(),
            });
        }
        Api {
            name: "Smoke".into(),
            version: "1.0.0".into(),
            schemas: vec![Schema::new("Widget", object)],
            operations: vec![
                operation,
                Operation {
                    id: "deleteWidget".into(),
                    method: HttpMethod::Delete,
                    path: "/widgets".into(),
                    responses: vec![OperationResponse {
                        status: "204".into(),
                        description: None,
                        media_types: vec![],
                    }],
                    ..Default::default()
                },
                Operation {
                    id: "events".into(),
                    responses: vec![OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![OperationMediaType {
                            content_type: "text/event-stream".into(),
                            schema: None,
                        }],
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }
    #[test]
    fn bounds_and_unsupported_media_are_explicit() {
        let tree = Packages::new()
            .package(
                crate::package("sdk")
                    .name("smoke-sdk")
                    .with(crate::sdk())
                    .with(operation_tests().max_operations(1)),
            )
            .generate(&api(), None)
            .unwrap();
        let report: Value = serde_json::from_str(
            tree.get("sdk/.poolster/operation-test-diagnostics.json")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report["generated"], 1);
        assert_eq!(report["skipped"].as_object().unwrap().len(), 2);
        assert!(
            tree.get("sdk/tests/operation-tests.ts")
                .unwrap()
                .contains("SKIP deleteWidget")
        );
    }
    #[test]
    #[ignore = "requires Node, KAJI_TSC_JS and KAJI_AXIOS_NODE_MODULES"]
    fn native_generated_fetch_and_axios_operation_tests_execute() {
        let compiler = std::env::var("KAJI_TSC_JS").unwrap();
        let modules = std::env::var("KAJI_AXIOS_NODE_MODULES").unwrap();
        for axios in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let sdk = if axios {
                crate::sdk().axios()
            } else {
                crate::sdk().fetch()
            };
            let tree = Packages::new()
                .package(
                    crate::package("sdk")
                        .name("smoke-sdk")
                        .with(sdk)
                        .with(operation_tests()),
                )
                .generate(&api(), None)
                .unwrap();
            tree.write_to(temp.path()).unwrap();
            let root = temp.path().join("sdk");
            #[cfg(unix)]
            std::os::unix::fs::symlink(&modules, root.join("node_modules")).unwrap();
            let output = std::process::Command::new("node")
                .arg(&compiler)
                .args(["-p", "tsconfig.json"])
                .current_dir(&root)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let output = std::process::Command::new("node")
                .arg("dist/tests/operation-tests.js")
                .current_dir(&root)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("SKIP events"));
        }
    }
}
