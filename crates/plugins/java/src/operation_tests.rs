//! Independent generated buffered-operation smoke tests; never contact an API.
use super::*;
use poolster_core::engine::{Handle, Meta, Plugin, PluginContext, Requirement};
use serde_json::{Value, json};
pub struct OperationTests {
    meta: Meta,
    sdk: Option<Handle<NativeSdk>>,
    max_operations: usize,
}
pub fn operation_tests() -> OperationTests {
    OperationTests {
        meta: Meta::new(),
        sdk: None,
        max_operations: 128,
    }
}
impl OperationTests {
    pub fn using_sdk(mut self, handle: Handle<NativeSdk>) -> Self {
        self.sdk = Some(handle);
        self
    }
    pub fn sdk_from(self, sdk: &Sdk) -> Self {
        self.using_sdk(sdk.contract())
    }
    pub fn max_operations(mut self, limit: usize) -> Self {
        self.max_operations = limit;
        self
    }
}
impl Plugin<Java> for OperationTests {
    fn kind(&self) -> &'static str {
        "java-operation-tests"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.sdk)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Java>) -> Result<()> {
        anyhow::ensure!(
            self.max_operations > 0 && self.max_operations <= 128,
            "operation test bound must be 1..128"
        );
        let sdk = cx.inputs.get::<NativeSdk>()?;
        let api = prepare_api(&operation_samples::clean_api(cx.api));
        let mut code = String::new();
        let mut cases = vec![];
        let mut unsupported = vec![];
        for (index, operation) in api.operations.iter().enumerate() {
            if index >= self.max_operations {
                unsupported.push(
                    json!({"operation":operation.id,"reason":"operation count bound exceeded"}),
                );
                continue;
            }
            match operation_samples::case(&api, operation) {
                Ok(mut fixture) => {
                    fixture["encoded_path"] = json!(operation_samples::encoded_path(&fixture));
                    let fixture_json = serde_json::to_string(&fixture)?;
                    let mut input = serde_json::Map::new();
                    for parameter in fixture["parameters"].as_array().unwrap() {
                        input.insert(
                            parameter_name(
                                operation
                                    .parameters
                                    .iter()
                                    .find(|candidate| {
                                        candidate.name == parameter["name"].as_str().unwrap()
                                            && candidate.location
                                                == parameter["location"].as_str().unwrap()
                                    })
                                    .unwrap(),
                            ),
                            parameter["value"].clone(),
                        );
                    }
                    if fixture["has_body"] == true {
                        input.insert("body".into(), fixture["body"].clone());
                    }
                    let arg = if input.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "MAPPER.readValue({},Client.{}Request.class)",
                            serde_json::to_string(&Value::Object(input).to_string())?,
                            type_name(&operation.id)
                        )
                    };
                    code += &format!(
                        "        {{ var driver=new Driver(MAPPER.readTree({})); var client=new Client(new ClientConfig(\"https://kaji-test.invalid\",null,\"Authorization\",\"Bearer\",Map.of(),driver,Duration.ofSeconds(1),null,null)); var result=client.{}({arg}); assertResult(driver,result); }}\n",
                        serde_json::to_string(&fixture_json)?,
                        method_name(&operation.id)
                    );
                    cases.push(fixture);
                }
                Err(error) => {
                    unsupported.push(json!({"operation":operation.id,"reason":error.to_string()}))
                }
            }
        }
        let source = include_str!("operation_driver.java.txt")
            .replace("__PACKAGE__", &sdk.namespace)
            .replace("__CASES__", &code);
        cx.files.emit(GeneratedFile::new(
            format!(
                "src/test/java/{}/PoolsterOperationTests.java",
                sdk.namespace.replace('.', "/")
            ),
            source,
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "operation-test-report.json",
            serde_json::to_string_pretty(
                &json!({"version":1,"supported":cases,"unsupported":unsupported}),
            )?,
        )?)?;
        cx.files.emit(GeneratedFile::new("OPERATION_TESTS.md",format!("# Generated operation smoke tests\n\nAll HTTP is fake and in memory. Run:\n\n~~~sh\nmvn -q test-compile org.codehaus.mojo:exec-maven-plugin:3.5.0:java -Dexec.mainClass={}.PoolsterOperationTests -Dexec.classpathScope=test\n~~~\n\noperation-test-report.json records supported cases and exclusions. Bounded structural samples strip source examples, defaults and annotations. These tests verify public operation calls, wire controls, bodies and decoded models; dedicated fixtures are still needed for constraints, recursive/compositional schemas, auth, pagination, retries, and streaming.\n",sdk.namespace))?)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use poolster_core::engine::Packages;
    use poolster_core::{
        AdditionalProperties, Field, HttpMethod, OperationMediaType, OperationParameter,
        OperationRequestBody, OperationResponse,
    };
    fn api() -> Api {
        let mut optional = SchemaValue::new(SchemaKind::String);
        optional.nullable = true;
        let mut body = SchemaValue::new(SchemaKind::Object {
            fields: vec![
                Field {
                    name: "id".into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "enabled".into(),
                    value: SchemaValue::new(SchemaKind::Boolean),
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "count".into(),
                    value: SchemaValue::new(SchemaKind::Integer),
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "note".into(),
                    value: optional,
                    required: true,
                    annotations: Default::default(),
                },
            ],
            additional_properties: AdditionalProperties::Forbidden,
        });
        body.default = Some(json!("DO_NOT_COPY_SOURCE_SECRET"));
        body.extensions
            .insert("x-secret".into(), json!("DO_NOT_COPY_SOURCE_SECRET"));
        body.constraints
            .insert("examples".into(), json!(["DO_NOT_COPY_SOURCE_SECRET"]));
        let parameter =
            |name: &str, location: &str, kind: SchemaKind, required: bool| OperationParameter {
                name: name.into(),
                location: location.into(),
                required,
                schema: Some(SchemaValue::new(kind)),
                description: None,
                annotations: Default::default(),
            };
        let echo = Operation {
            id: "echoContact".into(),
            method: HttpMethod::Post,
            path: "/contacts/{contact_id}".into(),
            parameters: vec![
                parameter("contact_id", "path", SchemaKind::String, true),
                parameter("enabled", "query", SchemaKind::Boolean, false),
                parameter("count", "query", SchemaKind::Integer, false),
                parameter("x-label", "header", SchemaKind::String, false),
            ],
            request_body: Some(OperationRequestBody::json(
                SchemaValue::reference("#/components/schemas/Contact"),
                true,
            )),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::reference("#/components/schemas/Contact"),
            )],
            ..Default::default()
        };
        let mut unsupported = echo.clone();
        unsupported.id = "streamContacts".into();
        unsupported.request_body = None;
        unsupported.responses[0].media_types = vec![OperationMediaType {
            content_type: "text/event-stream".into(),
            schema: None,
        }];
        let get = Operation {
            id: "getContact".into(),
            method: HttpMethod::Get,
            path: "/contacts".into(),
            responses: echo.responses.clone(),
            ..Default::default()
        };
        Api {
            name: "Test".into(),
            version: "1.0.0".into(),
            schemas: vec![Schema::new("Contact", body)],
            operations: vec![echo, get, unsupported],
            ..Default::default()
        }
    }
    fn generate() -> GeneratedTree {
        let sdk = crate::sdk();
        Packages::new()
            .package(
                crate::package("sdk")
                    .name("io.poolster.operationtest")
                    .with(operation_tests().sdk_from(&sdk))
                    .with(sdk),
            )
            .generate(&api(), None)
            .unwrap()
    }
    #[test]
    fn generated_smoke_tests_use_sdk_contract_and_sanitized_bounded_samples() {
        let tree = generate();
        let report: Value =
            serde_json::from_str(tree.get("sdk/operation-test-report.json").unwrap()).unwrap();
        assert_eq!(report["supported"].as_array().unwrap().len(), 2);
        assert_eq!(report["unsupported"][0]["operation"], "streamContacts");
        let report_text = tree.get("sdk/operation-test-report.json").unwrap();
        assert!(!report_text.contains("DO_NOT_COPY_SOURCE_SECRET"));
        assert!(report_text.contains("encoded_path"));
        let source = tree
            .get("sdk/src/test/java/io/kaji/operationtest/PoolsterOperationTests.java")
            .unwrap();
        assert!(source.contains("client.echoContact("));
        assert!(source.contains("request.bodyPublisher()"));
        assert!(source.contains("one native request required"));
        let error = Packages::new()
            .package(crate::package("sdk").with(operation_tests()))
            .generate(&api(), None)
            .unwrap_err();
        assert!(format!("{error:#}").contains("native-sdk"));
        assert!(
            Packages::new()
                .package(
                    crate::package("sdk")
                        .with(crate::sdk())
                        .with(operation_tests().max_operations(0))
                )
                .generate(&api(), None)
                .is_err()
        );
        let bounded = Packages::new()
            .package(
                crate::package("sdk")
                    .with(crate::sdk())
                    .with(operation_tests().max_operations(1)),
            )
            .generate(&api(), None)
            .unwrap();
        let report: Value =
            serde_json::from_str(bounded.get("sdk/operation-test-report.json").unwrap()).unwrap();
        assert_eq!(report["supported"].as_array().unwrap().len(), 1);
        assert!(
            report["unsupported"]
                .as_array()
                .unwrap()
                .iter()
                .any(|case| case["reason"] == "operation count bound exceeded")
        );
    }
    #[test]
    #[ignore = "requires Maven and JDK17; executes generated public operation calls through in-memory native HTTP driver"]
    fn native_generated_operation_tests_execute() {
        let tree = generate();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let result = std::process::Command::new("mvn")
            .args([
                "-q",
                "test-compile",
                "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
                "-Dexec.mainClass=io.poolster.operationtest.PoolsterOperationTests",
                "-Dexec.classpathScope=test",
            ])
            .current_dir(dir.path().join("sdk"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
