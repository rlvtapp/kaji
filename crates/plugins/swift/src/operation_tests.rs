use super::*;
use kaji_core::engine::{Meta, Plugin, PluginContext};
pub struct OperationTests {
    meta: Meta,
    options: kaji_core::samples::SampleOptions,
    limit: usize,
}
pub fn operation_tests() -> OperationTests {
    OperationTests {
        meta: Meta::new(),
        options: Default::default(),
        limit: 128,
    }
}
impl OperationTests {
    pub fn sample_options(mut self, options: kaji_core::samples::SampleOptions) -> Self {
        self.options = options;
        self
    }
    pub fn max_operations(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}
fn sample(
    api: &Api,
    value: &SchemaValue,
    options: kaji_core::samples::SampleOptions,
) -> std::result::Result<Value, String> {
    let report = kaji_core::samples::schema_samples(api, value, options);
    report
        .samples
        .into_iter()
        .next()
        .map(|x| x.value)
        .ok_or_else(|| report.diagnostics.join("; "))
}
fn fixture(
    api: &Api,
    op: &Operation,
    options: kaji_core::samples::SampleOptions,
) -> std::result::Result<Value, String> {
    if crate::multipart::selected(op) {
        return Err("multipart MIME requests require the dedicated native test adapter".into());
    }
    if !op.security.is_empty() {
        return Err("authentication requires a fixture adapter".into());
    }
    let mut args = serde_json::Map::new();
    let mut path = op.path.clone();
    let mut query = serde_json::Map::new();
    let mut headers = serde_json::Map::new();
    for p in &op.parameters {
        let schema = p.schema.as_ref().ok_or("parameter schema unavailable")?;
        if schema.nullable
            || !matches!(
                schema.kind,
                SchemaKind::String | SchemaKind::Boolean | SchemaKind::Integer | SchemaKind::Number
            )
        {
            return Err("non-scalar parameters require an adapter".into());
        }
        let value = sample(api, schema, options)?;
        let text = if let Some(s) = value.as_str() {
            s.to_owned()
        } else {
            value.to_string()
        };
        match p.location.as_str() {
            "path" => {
                if !text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-._~".contains(c))
                {
                    return Err("reserved path fixture needs an encoding adapter".into());
                }
                path = path.replace(&format!("{{{}}}", p.name), &text);
            }
            "query" => {
                query.insert(p.name.clone(), Value::String(text.clone()));
            }
            "header" => {
                headers.insert(p.name.clone(), Value::String(text));
            }
            _ => return Err("parameter location requires an adapter".into()),
        }
        args.insert(p.name.clone(), value);
    }
    let body = if let Some(body) = &op.request_body {
        let media = body
            .media_types
            .iter()
            .find(|m| m.content_type == "application/json" || m.content_type.ends_with("+json"))
            .ok_or("non-JSON request body requires an adapter")?;
        Some(sample(
            api,
            media.schema.as_ref().ok_or("request schema unavailable")?,
            options,
        )?)
    } else {
        None
    };
    let response = op
        .responses
        .iter()
        .find(|r| {
            r.status
                .parse::<u16>()
                .is_ok_and(|n| (200..300).contains(&n))
        })
        .ok_or("explicit successful response required")?;
    let payload = if response.media_types.is_empty() {
        None
    } else {
        let media = response
            .media_types
            .iter()
            .find(|m| m.content_type == "application/json" || m.content_type.ends_with("+json"))
            .ok_or("non-JSON response requires an adapter")?;
        Some(sample(
            api,
            media.schema.as_ref().ok_or("response schema unavailable")?,
            options,
        )?)
    };
    if body.as_ref().is_some_and(Value::is_null) || payload.as_ref().is_some_and(Value::is_null) {
        return Err("nullable root sample requires an adapter".into());
    }
    Ok(
        serde_json::json!({"args":args,"method":op.method.as_str(),"path":path,"query":query,"headers":headers,"body":body,"response":payload,"status":response.status}),
    )
}
impl Plugin<crate::Swift> for OperationTests {
    fn kind(&self) -> &'static str {
        "swift-operation-tests"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Swift>) -> Result<()> {
        let mut skipped = BTreeMap::new();
        let mut code = String::new();
        let mut count = 0;
        for (index, op) in cx.api.operations.iter().enumerate() {
            let result = if index >= self.limit {
                Err("operation bound exhausted".into())
            } else {
                fixture(cx.api, op, self.options)
            };
            match result {
                Err(reason) => {
                    skipped.insert(op.id.clone(), reason);
                }
                Ok(value) => {
                    let mut args = Vec::new();
                    for p in &op.parameters {
                        let v = &value["args"][&p.name];
                        let ty = swift_type(p.schema.as_ref().unwrap(), false);
                        let literal = serde_json::to_string(&v.to_string())?;
                        args.push(format!(
                            "{}: try JSONDecoder().decode({ty}.self, from: Data({literal}.utf8))",
                            identifier(&p.name)
                        ));
                    }
                    if let Some(body) = &op.request_body {
                        let schema = body
                            .media_types
                            .iter()
                            .find(|m| {
                                m.content_type == "application/json"
                                    || m.content_type.ends_with("+json")
                            })
                            .and_then(|m| m.schema.as_ref())
                            .unwrap();
                        let ty = swift_type(schema, false);
                        let literal = serde_json::to_string(&value["body"].to_string())?;
                        args.push(format!(
                            "body: try JSONDecoder().decode({ty}.self, from: Data({literal}.utf8))"
                        ));
                    }
                    let fixture = serde_json::to_string(&value.to_string())?;
                    let call = format!(
                        "try await client.{}({})",
                        function_name(&op.id),
                        args.join(", ")
                    );
                    let result = if op.success_schema().is_some() {
                        format!(
                            "let result = {call}\n let encoded=try JSONEncoder().encode(result)\n let actual=try JSONSerialization.jsonObject(with:encoded,options:[.fragmentsAllowed])\n guard try canonical(actual)==canonical(sample[\"response\"]!) else {{throw ProbeError.mismatch}}"
                        )
                    } else {
                        call
                    };
                    code.push_str(&format!("do {{let sample=try JSONSerialization.jsonObject(with:Data({fixture}.utf8)) as! [String:Any]\n let driver=ProbeTransport(sample:sample)\n let client=KajiClient(options:.init(baseURL:URL(string:\"https://unused.example\")!),transport:driver)\n {result}\n guard driver.calls==1 else {{throw ProbeError.mismatch}}\n}}\n"));
                    count += 1;
                }
            }
        }
        cx.files.emit(GeneratedFile::new(
            "test/operation-diagnostics.json",
            serde_json::to_string_pretty(
                &serde_json::json!({"generated":count,"skipped":skipped}),
            )?,
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "test/OperationTests.swift",
            include_str!("operation_tests.swift.txt").replace("__CASES__", &code),
        )?)?;
        cx.files.emit(GeneratedFile::new("OPERATION_TESTS.md","Compile `swiftc -parse-as-library Sources/*/*.swift test/OperationTests.swift -o /tmp/kaji-operation-tests`, then run `/tmp/kaji-operation-tests`. Public native calls use a fake driver without network. Bounded structural samples assert wire values and decoded responses; explicit unsupported operations are in test/operation-diagnostics.json. Optional webhook dependencies require SwiftPM module search paths when compiling this probe.")?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires native Swift compiler"]
    fn generated_operation_tests_execute_native() {
        let mut api = Api {
            name: "Probe".into(),
            operations: vec![
                Operation {
                    id: "readThing".into(),
                    method: kaji_core::HttpMethod::Get,
                    path: "/thing".into(),
                    responses: vec![kaji_core::OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![kaji_core::OperationMediaType {
                            content_type: "application/json".into(),
                            schema: Some(SchemaValue::new(SchemaKind::String)),
                        }],
                    }],
                    ..Default::default()
                },
                Operation {
                    id: "deleteThing".into(),
                    method: kaji_core::HttpMethod::Delete,
                    path: "/thing".into(),
                    responses: vec![kaji_core::OperationResponse {
                        status: "204".into(),
                        description: None,
                        media_types: vec![],
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        api.operations.push(Operation {
            id: "createThing".into(),
            method: kaji_core::HttpMethod::Post,
            path: "/thing".into(),
            request_body: Some(kaji_core::OperationRequestBody::json(
                SchemaValue::new(SchemaKind::Boolean),
                true,
            )),
            parameters: vec![
                kaji_core::OperationParameter {
                    name: "flag".into(),
                    location: "query".into(),
                    required: true,
                    schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                    description: None,
                    annotations: Default::default(),
                },
                kaji_core::OperationParameter {
                    name: "count".into(),
                    location: "header".into(),
                    required: true,
                    schema: Some(SchemaValue::new(SchemaKind::Integer)),
                    description: None,
                    annotations: Default::default(),
                },
            ],
            responses: vec![kaji_core::OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![kaji_core::OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                }],
            }],
            ..Default::default()
        });
        api.operations.push(Operation {
            id: "retryThing".into(),
            method: kaji_core::HttpMethod::Post,
            path: "/thing".into(),
            parameters: vec![kaji_core::OperationParameter {
                name: "X-Once".into(),
                location: "header".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: Default::default(),
            }],
            responses: vec![kaji_core::OperationResponse {
                status: "204".into(),
                description: None,
                media_types: vec![],
            }],
            annotations: std::collections::BTreeMap::from([(
                "x-kaji-idempotency".into(),
                serde_json::json!({"header":"X-Once","auto_generate":true}),
            )]),
            ..Default::default()
        });
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .with(crate::sdk())
                    .with(operation_tests()),
            )
            .generate(&api, None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let cwd = dir.path().join("sdk");
        let probe = cwd.join("test/OperationTests.swift");
        let source = std::fs::read_to_string(&probe).unwrap();
        let extra = r#"
let retry=RetryProbe();let client=KajiClient(options:.init(baseURL:URL(string:"https://unused.example")!,maxAttempts:2,retryBaseDelay:0),transport:retry)
try await client.deleteThing();guard retry.calls==2 else{throw ProbeError.mismatch}
retry.calls=0;retry.keys=[];try await client.retryThing();guard retry.calls==2,retry.keys[0]==retry.keys[1],retry.keys[0]?.count==36 else{throw ProbeError.mismatch}
retry.calls=0;retry.keys=[];var request=URLRequest(url:URL(string:"https://unused.example")!);request.httpMethod="POST"
do{try await client.sendVoid(request);throw ProbeError.mismatch}catch KajiAPIError.status{ } ;guard retry.calls==1 else{throw ProbeError.mismatch}
retry.calls=0;retry.keys=[];request.httpMethod="PATCH";request.setValue("stable",forHTTPHeaderField:"X-Once")
try await client.sendVoid(request,idempotencyHeader:"X-Once");guard retry.calls==2,retry.keys==["stable","stable"] else{throw ProbeError.mismatch}
retry.calls=0;request.setValue("  ",forHTTPHeaderField:"X-Once");do{try await client.sendVoid(request,idempotencyHeader:"X-Once");throw ProbeError.mismatch}catch KajiAPIError.status{ };guard retry.calls==1 else{throw ProbeError.mismatch}
let waiting=RetryProbe();waiting.delay="60000";let slow=KajiClient(options:.init(baseURL:URL(string:"https://unused.example")!,maxAttempts:2,retryMaxDelay:60),transport:waiting)
let cancelled=Task {try await slow.deleteThing()};try await Task.sleep(nanoseconds:20_000_000);cancelled.cancel();do{try await cancelled.value;throw ProbeError.mismatch}catch is CancellationError{};guard waiting.calls==1 else{throw ProbeError.mismatch}
"#;
        let source = source.replace("\n}}\n", &(extra.to_owned() + "\n}}\n"))
            + include_str!("retry_probe.swift.txt");
        std::fs::write(probe, source).unwrap();
        let mut command = std::process::Command::new("swiftc");
        command
            .arg("-parse-as-library")
            .env(
                "CLANG_MODULE_CACHE_PATH",
                std::env::temp_dir().join("kaji-swift-cache"),
            )
            .env(
                "SWIFT_MODULECACHE_PATH",
                std::env::temp_dir().join("kaji-swift-cache"),
            );
        for (path, _) in tree.iter() {
            if path.to_string_lossy().ends_with(".swift") && !path.ends_with("Package.swift") {
                command.arg(dir.path().join(path));
            }
        }
        let output = command
            .args(["-o", "probe"])
            .current_dir(&cwd)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let output = std::process::Command::new(cwd.join("probe"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
