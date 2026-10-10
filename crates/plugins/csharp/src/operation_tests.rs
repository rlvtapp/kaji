//! Independent generated buffered-operation smoke tests; never contact an API.
use super::*;
use poolster_core::engine::{Handle, Meta, Plugin, PluginContext, Requirement};
use serde_json::{Value, json};
pub struct OperationTests {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    sdk: Option<Handle<NativeSdk>>,
    max_operations: usize,
}
pub fn operation_tests() -> OperationTests {
    OperationTests {
        http_input: Default::default(),
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
fn render(api: &Api, sdk: &NativeSdk, bound: usize) -> Result<(String, Value)> {
    anyhow::ensure!(
        bound > 0 && bound <= 128,
        "operation test bound must be 1..128"
    );
    let api = prepare_api(&operation_samples::clean_api(api));
    let mut code = String::new();
    let mut cases = vec![];
    let mut unsupported = vec![];
    for (index, operation) in api.operations.iter().enumerate() {
        if index >= bound {
            unsupported
                .push(json!({"operation":operation.id,"reason":"operation count bound exceeded"}));
            continue;
        }
        match operation_samples::case(&api, operation) {
            Ok(mut fixture) => {
                fixture["encoded_path"] = json!(operation_samples::encoded_path(&fixture));
                let fixture_json = serde_json::to_string(&fixture)?;
                let mut args = vec![];
                for parameter in &operation.parameters {
                    let value = fixture["parameters"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|value| {
                            value["name"] == parameter.name
                                && value["location"] == parameter.location
                        })
                        .unwrap();
                    args.push(format!(
                        "{}: JsonSerializer.Deserialize<{}>({})!",
                        parameter_name(parameter),
                        csharp_type(parameter.schema.as_ref().unwrap(), !parameter.required),
                        serde_json::to_string(&value["value"].to_string())?
                    ));
                }
                if fixture["has_body"] == true {
                    args.push(format!(
                        "body: JsonSerializer.Deserialize<{}>({})!",
                        operation_request_type(operation).unwrap(),
                        serde_json::to_string(&fixture["body"].to_string())?
                    ));
                }
                code += &format!(
                    "        {{ var driver=new Driver({}); using var http=new HttpClient(driver); var client=new PoolsterClient(http,new PoolsterClientOptions {{ BaseUrl=\"https://poolster-test.invalid\" }}); var result=await client.{}Async({}); AssertResult(driver,result); }}\n",
                    serde_json::to_string(&fixture_json)?,
                    pascal_case(&operation.id),
                    args.join(",")
                );
                cases.push(fixture);
            }
            Err(error) => {
                unsupported.push(json!({"operation":operation.id,"reason":error.to_string()}))
            }
        }
    }
    Ok((
        include_str!("../tests/fixtures/operation_driver.cs")
            .replace("__PACKAGE__", &sdk.namespace)
            .replace("__CASES__", &code),
        json!({"version":1,"supported":cases,"unsupported":unsupported}),
    ))
}
macro_rules! plugin {
    ($language:ty)=>{
        impl Plugin<$language> for OperationTests{
 fn supports_native_input(&self) -> bool { self.http_input.is_explicit() }
            fn kind(&self)->&'static str{"csharp-operation-tests"}
            fn meta(&self)->&Meta{&self.meta}
            fn requires(&self)->Vec<Requirement>{let mut requirements=self.http_input.requirements();requirements.push(Requirement::on(self.sdk));requirements}
            fn generate(&self,cx:&mut PluginContext<'_,$language>)->Result<()>{self.http_input.with_context(cx,|cx|{
                let sdk=cx.inputs.get::<NativeSdk>()?;
                let (source,report)=render(cx.api,sdk,self.max_operations)?;
                let package=cx.settings.package_name.clone().unwrap_or_else(||format!("{}-sdk",kebab_case(&cx.api.name)));
                let project=format!("{}.csproj",pascal_case(&package));
                cx.files.emit(GeneratedFile::new("tests/OperationTests/Program.cs",source)?)?;
                cx.files.emit(GeneratedFile::new("tests/OperationTests/OperationTests.csproj",format!("<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><OutputType>Exe</OutputType><Nullable>enable</Nullable><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><ProjectReference Include=\"../../{project}\" /></ItemGroup></Project>\n"))?)?;
                cx.files.emit(GeneratedFile::new("operation-test-report.json",serde_json::to_string_pretty(&report)?)?)?;
                cx.files.emit(GeneratedFile::new("OPERATION_TESTS.md","# Generated operation smoke tests\n\nAll HTTP is fake and in memory. Run:\n\n~~~sh\ndotnet run --project tests/OperationTests/OperationTests.csproj\n~~~\n\noperation-test-report.json records supported cases and exclusions. Samples strip source examples, defaults and annotations. Dedicated fixtures remain necessary for constraints, recursion/composition, auth, pagination, retries and streaming.\n")?)?;
                Ok(())
            })}
        }
    }
}
plugin!(CSharp);
pub(crate) fn finalize(tree: &mut GeneratedTree) -> Result<()> {
    if tree.get("tests/OperationTests/Program.cs").is_none() {
        return Ok(());
    }
    let (path, project) = tree
        .iter()
        .find(|(path, _)| {
            path.components().count() == 1
                && path
                    .extension()
                    .is_some_and(|extension| extension == "csproj")
        })
        .map(|(path, source)| (path.to_owned(), source.to_owned()))
        .ok_or_else(|| anyhow::anyhow!("operation tests require generated C# project"))?;
    anyhow::ensure!(
        project.contains("</Project>"),
        "operation tests cannot modify unknown C# project"
    );
    tree.replace(GeneratedFile::new(
        path,
        project.replace(
            "</Project>",
            "  <ItemGroup><Compile Remove=\"tests/**/*.cs\" /></ItemGroup>\n</Project>",
        ),
    )?)
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
                    .name("Poolster.OperationTest")
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
        let source = tree.get("sdk/tests/OperationTests/Program.cs").unwrap();
        assert!(source.contains("client.EchoContactAsync("));
        assert!(source.contains("ReadAsStringAsync(token)"));
        assert!(
            tree.get("sdk/PoolsterOperationTest.csproj")
                .unwrap()
                .contains("Compile Remove=\"tests/**/*.cs\"")
        );
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
    #[ignore = "requires .NET8; executes generated public operation calls through in-memory native HTTP driver"]
    fn native_generated_operation_tests_execute() {
        let tree = generate();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let result = std::process::Command::new("dotnet")
            .args([
                "run",
                "--project",
                "tests/OperationTests/OperationTests.csproj",
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

#[path = "operation_tests_input.rs"]
mod http_input;
