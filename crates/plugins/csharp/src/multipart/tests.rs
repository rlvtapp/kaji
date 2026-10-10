use super::*;

fn api() -> Api {
    let mut file = SchemaValue::new(SchemaKind::String);
    file.format = Some("binary".into());
    let field = |name: &str, value, required| poolster_core::Field {
        name: name.into(),
        value,
        required,
        annotations: Default::default(),
    };
    Api {
        name: "Multipart".into(),
        schemas: vec![Schema::new(
            "Upload",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![
                    field("title", SchemaValue::new(SchemaKind::String), true),
                    field("flag", SchemaValue::new(SchemaKind::Boolean), true),
                    field("count", SchemaValue::new(SchemaKind::Integer), true),
                    field("file", file, true),
                    field("missing", SchemaValue::new(SchemaKind::String), false),
                ],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        )],
        operations: vec![Operation {
            id: "uploadThing".into(),
            method: poolster_core::HttpMethod::Post,
            path: "/upload".into(),
            request_body: Some(poolster_core::OperationRequestBody {
                required: true,
                description: None,
                media_types: vec![poolster_core::OperationMediaType {
                    content_type: "multipart/form-data".into(),
                    schema: Some(SchemaValue::reference("#/components/schemas/Upload")),
                }],
            }),
            responses: vec![poolster_core::OperationResponse {
                status: "204".into(),
                description: None,
                media_types: vec![],
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}
#[test]
fn rejects_unsupported_shapes_before_emission() {
    let mut source = api();
    let SchemaKind::Object { fields, .. } = &mut source.schemas[0].value.kind else {
        unreachable!()
    };
    fields[0].value = SchemaValue::new(SchemaKind::Array {
        items: Box::new(SchemaValue::new(SchemaKind::String)),
    });
    validate(&source).unwrap();
    let mut source = api();
    source.operations[0]
        .request_body
        .as_mut()
        .unwrap()
        .media_types
        .push(poolster_core::OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(SchemaValue::new(SchemaKind::String)),
        });
    validate(&source).unwrap();
}
#[test]
fn emits_native_typed_multipart_in_public_and_resource_calls() {
    let tree = poolster_core::engine::Packages::new()
        .package(
            crate::package("sdk")
                .name("Poolster.Multipart")
                .with(crate::sdk()),
        )
        .generate(&api(), None)
        .unwrap();
    let dto = tree.get("sdk/UploadThingMultipartBody.cs").unwrap();
    assert!(dto.contains("required MultipartFile File"));
    assert!(dto.contains("new MultipartFormDataContent()"));
    let client = tree
        .get("sdk/Operations/PoolsterClientOperations000.cs")
        .unwrap();
    assert!(client.contains("UploadThingMultipartBody body"));
    assert!(client.contains("request.Content = body.ToContent()"));
    assert!(!client.contains("UploadThingAsync(byte[]"));
}
#[test]
#[ignore = "requires .NET8; parses native HTTP multipart request bytes without network"]
fn native_multipart_http_bytes_preserve_unicode_falsy_and_binary() {
    let tree = poolster_core::engine::Packages::new()
        .package(
            crate::package("sdk")
                .name("Poolster.Multipart")
                .with(crate::sdk())
                .with(crate::operation_tests()),
        )
        .generate(&api(), None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("sdk/tests/OperationTests/Program.cs"),
        include_str!("../../tests/fixtures/multipart_probe.cs"),
    )
    .unwrap();
    let output = std::process::Command::new("dotnet")
        .args([
            "run",
            "--project",
            "tests/OperationTests/OperationTests.csproj",
        ])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn complex_api() -> Api {
    let mut source = api();
    let SchemaKind::Object {
        fields,
        additional_properties,
    } = &mut source.schemas[0].value.kind
    else {
        unreachable!()
    };
    *additional_properties = AdditionalProperties::Any;
    fields.push(poolster_core::Field {
        name: "chunking_strategy".into(),
        value: SchemaValue::new(SchemaKind::OneOf {
            variants: vec![
                SchemaValue::new(SchemaKind::String),
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![],
                    additional_properties: AdditionalProperties::Any,
                }),
            ],
        }),
        required: false,
        annotations: Default::default(),
    });
    fields.push(poolster_core::Field {
        name: "timestamp_granularities[]".into(),
        value: SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::new(SchemaKind::String)),
        }),
        required: false,
        annotations: Default::default(),
    });
    let mut binary = SchemaValue::new(SchemaKind::String);
    binary.format = Some("binary".into());
    fields.push(poolster_core::Field {
        name: "files".into(),
        value: SchemaValue::new(SchemaKind::Array {
            items: Box::new(binary),
        }),
        required: false,
        annotations: Default::default(),
    });
    source.operations[0].annotations.insert("poolster.request_body_encodings".into(),serde_json::json!({"multipart/form-data":{"chunking_strategy":{"contentType":"application/json"}}}));
    source
}

#[test]
#[ignore = "requires .NET8; native complex multipart wire probe"]
fn native_multipart_complex_json_and_repeated_arrays() {
    let tree = poolster_core::engine::Packages::new()
        .package(
            crate::package("sdk")
                .name("Poolster.Multipart")
                .with(crate::sdk())
                .with(crate::operation_tests()),
        )
        .generate(&complex_api(), None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("sdk/tests/OperationTests/Program.cs"),
        include_str!("../../tests/fixtures/multipart_complex_probe.cs"),
    )
    .unwrap();
    let output = std::process::Command::new("dotnet")
        .args([
            "run",
            "--project",
            "tests/OperationTests/OperationTests.csproj",
        ])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
