//! Optional native operation smoke tests using bounded structural fixtures.
use crate::{
    Rust,
    composition::{Client, Operations, Transport},
    render,
};
use anyhow::Result;
use kaji_core::{
    Api, GeneratedFile, Operation, SchemaKind, SchemaValue,
    engine::{Handle, Meta, Plugin, PluginContext, Requirement},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fmt::Write};
pub struct OperationTests {
    meta: Meta,
    client: Option<Handle<Client>>,
    operations: Option<Handle<Operations>>,
    transport: Option<Handle<Transport>>,
    options: kaji_core::samples::SampleOptions,
    max_operations: usize,
}
pub fn operation_tests() -> OperationTests {
    OperationTests {
        meta: Meta::new(),
        client: None,
        operations: None,
        transport: None,
        options: Default::default(),
        max_operations: 128,
    }
}
impl OperationTests {
    pub fn using_client(mut self, handle: Handle<Client>) -> Self {
        self.client = Some(handle);
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
    pub fn sample_options(mut self, options: kaji_core::samples::SampleOptions) -> Self {
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
    options: kaji_core::samples::SampleOptions,
) -> std::result::Result<Value, String> {
    let report = kaji_core::samples::schema_samples(api, schema, options);
    report
        .samples
        .into_iter()
        .next()
        .map(|sample| sample.value)
        .ok_or_else(|| format!("no bounded sample: {}", report.diagnostics.join("; ")))
}
fn render_test(
    api: &Api,
    operation: &Operation,
    method: &str,
    index: usize,
    options: kaji_core::samples::SampleOptions,
    module: &str,
) -> std::result::Result<String, String> {
    let mut fields = vec![];
    let mut path = format!("{:?}.to_owned()", operation.path);
    let mut checks = String::new();
    for parameter in &operation.parameters {
        let schema = parameter
            .schema
            .as_ref()
            .ok_or("parameter schema unavailable")?;
        if !matches!(
            schema.kind,
            SchemaKind::String | SchemaKind::Boolean | SchemaKind::Integer | SchemaKind::Number
        ) || matches!(schema.format.as_deref(), Some("byte" | "binary"))
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
        let json = value.to_string();
        let value = format!("serde_json::from_str({json:?}).unwrap()");
        fields.push(format!(
            "{}: {}",
            render::rust_field_name(&parameter.name),
            if parameter.required {
                value
            } else {
                format!("Some({value})")
            }
        ));
        match parameter.location.as_str() {
            "path" => {
                path = format!(
                    "{path}.replace({:?},&encode({encoded:?}))",
                    format!("{{{}}}", parameter.name)
                )
            }
            "query" => {
                writeln!(checks,"assert!(request.url().query_pairs().any(|(key,value)|key=={:?}&&value=={encoded:?}),\"query serialization\");",parameter.name).unwrap();
            }
            "header" => {
                writeln!(checks,"assert_eq!(request.headers().get({:?}).unwrap().to_str().unwrap(),{encoded:?},\"header serialization\");",parameter.name).unwrap();
            }
            _ => return Err("parameter location needs an adapter".into()),
        }
    }
    let input = if operation.parameters.is_empty() {
        String::new()
    } else {
        format!(
            ",crate::client::{}Request{{{}}}",
            render::type_name(&operation.id),
            fields.join(",")
        )
    };
    let mut body_setup = String::new();
    let mut body_arg = String::new();
    if let Some(body) = &operation.request_body {
        let media = body
            .media_types
            .iter()
            .find(|media| media.content_type == "application/json")
            .ok_or("non-JSON request body needs an adapter")?;
        let value = sample(
            api,
            media.schema.as_ref().ok_or("body schema unavailable")?,
            options,
        )?;
        if value.is_null() {
            return Err("nullable root body needs an adapter".into());
        }
        let json = value.to_string();
        body_setup = format!("let body=serde_json::from_str({json:?}).unwrap();");
        body_arg = ",&body".into();
        writeln!(checks,"assert_eq!(serde_json::from_slice::<serde_json::Value>(request.body().unwrap().as_bytes().expect(\"buffered JSON\")).unwrap(),serde_json::from_str::<serde_json::Value>({json:?}).unwrap(),\"request JSON\");").unwrap();
    }
    let response = operation
        .responses
        .iter()
        .find(|response| {
            response
                .status
                .parse::<u16>()
                .is_ok_and(|status| (200..300).contains(&status))
        })
        .ok_or("explicit success status unavailable")?;
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
    let json = expected.clone().unwrap_or(Value::Null).to_string();
    let response_check=expected.map(|_|format!("assert_eq!(serde_json::to_value(result).unwrap(),serde_json::from_str::<serde_json::Value>({json:?}).unwrap(),\"decoded JSON\");")).unwrap_or_else(||"let _=result;".into());
    let args = [
        input.trim_start_matches(','),
        body_arg.trim_start_matches(','),
    ]
    .into_iter()
    .filter(|arg| !arg.is_empty())
    .collect::<Vec<_>>()
    .join(",");
    Ok(format!(
        r#"
#[tokio::test] async fn operation_wire_{index}(){{
 struct Mock(std::sync::Arc<std::sync::atomic::AtomicUsize>);
 impl {module}::Transport for Mock{{fn execute(&self,request:reqwest::Request)->{module}::TransportFuture<'_>{{
 self.0.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
 assert_eq!(request.method().as_str(),{method_name:?});assert_eq!(request.url().path(),{path});{checks}
 Box::pin(async{{Ok(http::Response::builder().status({status}).header("content-type","application/json").body(reqwest::Body::from({json:?})).unwrap().into())}})
 }}}}
 let calls=std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
 let client=crate::Client::new("https://unused.example").with_transport(std::sync::Arc::new(Mock(calls.clone()))).with_retry(crate::client::RetryConfig{{max_attempts:1,..Default::default()}});
 {body_setup}let result=client.{method}({args}).await.expect("bounded successful response");
 assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst),1);{response_check}
}}
"#,
        method_name = operation.method.as_str()
    ))
}
impl Plugin<Rust> for OperationTests {
    fn kind(&self) -> &'static str {
        "rust-operation-tests"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![
            Requirement::on(self.client),
            Requirement::on(self.operations),
            Requirement::on(self.transport),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        let client = cx.inputs.get::<Client>()?;
        let operations = cx.inputs.get::<Operations>()?;
        let transport = cx.inputs.get::<Transport>()?;
        let mut source = String::from(
            "// Generated by Kaji. Do not edit.\n#[allow(dead_code)]\nfn encode(value:&str)->String{value.bytes().map(|byte|if byte.is_ascii_alphanumeric()||matches!(byte,b'-'|b'.'|b'_'|b'~'){(byte as char).to_string()}else{format!(\"%{byte:02X}\")}).collect()}\n",
        );
        let mut skipped = BTreeMap::new();
        let mut generated = 0;
        for (index, operation) in cx.api.operations.iter().enumerate() {
            let test = if index >= self.max_operations {
                Err("operation count bound reached".into())
            } else if client.symbol != "crate::Client" {
                Err("custom client needs a native test adapter".into())
            } else {
                operations
                    .methods
                    .get(&operation.id)
                    .ok_or_else(|| "operation unavailable".into())
                    .and_then(|method| {
                        render_test(
                            cx.api,
                            operation,
                            method,
                            index,
                            self.options,
                            &transport.module,
                        )
                    })
            };
            match test {
                Ok(test) => {
                    source.push_str(&test);
                    generated += 1
                }
                Err(reason) => {
                    source.push_str(&format!("#[test] #[ignore={reason:?}] fn operation_wire_{index}(){{panic!({reason:?})}}\n"));
                    skipped.insert(operation.id.clone(), reason);
                }
            }
        }
        cx.files
            .emit(GeneratedFile::new("src/operation_tests.rs", source)?)?;
        cx.workspace.operation_tests = true;
        cx.files.emit(GeneratedFile::new(
            ".kaji/operation-test-diagnostics.json",
            serde_json::to_string_pretty(&json!({"generated":generated,"skipped":skipped}))? + "\n",
        )?)?;
        cx.files.emit(GeneratedFile::new("OPERATION_TESTS.md",format!("# Generated native operation tests\n\nRun `cargo test`. Unsupported placeholders carry explicit ignore reasons and fail if explicitly forced with `--ignored`. {generated} bounded public-operation tests use an in-memory native Transport; no network is used. {} unsupported operations are documented in `.kaji/operation-test-diagnostics.json`. Method/path escaping, scalar query/header serialization, JSON body encoding, success status and typed response roundtrips are checked; each test permits one attempt. Fixtures use structural samples rather than source examples. Streaming, non-JSON media, non-scalar/referenced parameters, nullable body roots, custom client ABI, impossible samples and configured bounds are excluded. These are wire serialization smoke tests, not complete API acceptance/schema coverage.\n",skipped.len()))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PackageExt;
    use kaji_core::{
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
            tree.get("sdk/.kaji/operation-test-diagnostics.json")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report["generated"], 1);
        assert_eq!(report["skipped"].as_object().unwrap().len(), 2);
        assert!(
            tree.get("sdk/src/operation_tests.rs")
                .unwrap()
                .contains("#[ignore=")
        );
        assert!(
            tree.get("sdk/Cargo.toml")
                .unwrap()
                .contains("[dev-dependencies]")
        );
    }
    #[test]
    #[ignore = "requires Cargo and generated Reqwest dependencies (offline cache supported)"]
    fn native_generated_rust_operation_tests_execute() {
        let temp = tempfile::tempdir().unwrap();
        let tree = Packages::new()
            .package(
                crate::package("sdk")
                    .name("smoke-sdk")
                    .with(crate::sdk().operation_prefix("wire_"))
                    .with(operation_tests()),
            )
            .generate(&api(), None)
            .unwrap();
        tree.write_to(temp.path()).unwrap();
        let mut command = std::process::Command::new("cargo");
        command.args(["test", "--lib"]);
        if std::env::var("KAJI_RUNTIME_OFFLINE").as_deref() == Ok("1") {
            command.arg("--offline");
        }
        command.current_dir(temp.path().join("sdk")).env(
            "CARGO_TARGET_DIR",
            std::env::var("KAJI_RUNTIME_RUST_TARGET")
                .unwrap_or_else(|_| temp.path().join("target").to_string_lossy().into_owned()),
        );
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("2 passed"));
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 ignored"));
    }
}
