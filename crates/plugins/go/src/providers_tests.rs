use super::*;
use poolster_core::{
    Api, HttpMethod, Operation, OperationResponse, Schema, SchemaKind, SchemaValue,
    engine::Packages,
};
struct CustomTransport {
    meta: Meta,
}
impl Plugin<Go> for CustomTransport {
    fn kind(&self) -> &'static str {
        "custom-http"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Transport>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        cx.files.emit(GeneratedFile::new("custom_http.go",r#"package demo
import("io";"net/http";"strings")
type customHTTP struct{}
func(customHTTP) Do(request *http.Request)(*http.Response,error){return &http.Response{StatusCode:200,Header:http.Header{"Content-Type":[]string{"application/json"}},Body:io.NopCloser(strings.NewReader(`"custom"`)),Request:request},nil}
"#)?)?;
        cx.publish(Transport {
            constructor: "customHTTP{}".into(),
        })
    }
}
#[test]
fn custom_provider_and_generated_roundtrips_execute_without_network() {
    let mut integer = SchemaValue::new(SchemaKind::Integer);
    integer.format = Some("int64".into());
    let api = Api {
        name: "demo".into(),
        version: "1.0.0".into(),
        schemas: vec![
            Schema::new(
                "Details",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![poolster_core::Field {
                        name: "note".into(),
                        value: {
                            let mut value = SchemaValue::new(SchemaKind::String);
                            value.nullable = true;
                            value
                        },
                        required: false,
                        annotations: Default::default(),
                    }],
                    additional_properties: poolster_core::AdditionalProperties::Any,
                }),
            ),
            Schema::new("Counter", integer),
            Schema::new("Label", SchemaValue::new(SchemaKind::String)),
        ],
        operations: vec![Operation {
            id: "getLabel".into(),
            method: HttpMethod::Get,
            path: "/label".into(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::String),
            )],
            ..Default::default()
        }],
        ..Default::default()
    };
    let model = models();
    let model_handle = model.models_handle();
    let custom = CustomTransport { meta: Meta::new() };
    let transport_handle = custom.meta.handle::<Transport>();
    let operation = operations()
        .using_models(model_handle)
        .using_transport(transport_handle);
    let operation_handle = operation.operations_handle();
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .with(client().using_operations(operation_handle))
                .with(operation)
                .with(custom)
                .with(model)
                .with(roundtrip_tests().using_models(model_handle)),
        )
        .generate(&api, None)
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    std::fs::write(root.path().join("sdk/custom_test.go"),r#"package demo
import("context";"testing")
func TestCustomTransport(t *testing.T){client,err:=NewClient(ClientConfig{BaseURL:"https://unused.example"});if err!=nil{t.Fatal(err)};result,err:=client.GetLabel(context.Background());if err!=nil{t.Fatal(err)};if result==nil || *result!="custom"{t.Fatal(result)}}
"#).unwrap();
    let output = std::process::Command::new("go")
        .args(["test", "./..."])
        .env("GOCACHE", root.path().join("go-cache"))
        .current_dir(root.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
