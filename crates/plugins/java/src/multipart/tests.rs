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
fn emits_native_typed_multipart_in_all_response_drivers() {
    let tree = poolster_core::engine::Packages::new()
        .package(
            crate::package("sdk")
                .name("io.poolster.multipart")
                .with(crate::sdk()),
        )
        .generate(&api(), None)
        .unwrap();
    let client = tree
        .get("sdk/src/main/java/io/poolster/multipart/ClientBase.java")
        .unwrap();
    assert_eq!(
        client
            .matches("body instanceof MultipartBody multipart")
            .count(),
        4
    );
    let dto = tree
        .get("sdk/src/main/java/io/poolster/multipart/UploadThingMultipartBody.java")
        .unwrap();
    assert!(dto.contains("MultipartBody.FilePart file"));
    assert!(dto.contains("MultipartBody.scalar(flag)"));
    let operation = tree
        .iter()
        .find(|(_, source)| source.contains("public record UploadThingRequest"))
        .unwrap()
        .1;
    assert!(operation.contains("UploadThingMultipartBody body"));
}
#[test]
#[ignore = "requires JDK17+Maven; parses native HTTP multipart request bytes without network"]
fn native_multipart_http_bytes_preserve_unicode_falsy_and_binary() {
    let tree = poolster_core::engine::Packages::new()
        .package(
            crate::package("sdk")
                .name("io.poolster.multipart")
                .with(crate::sdk())
                .with(crate::operation_tests()),
        )
        .generate(&api(), None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    let mut source = include_str!("../../tests/fixtures/operation_driver.java")
        .replace("__PACKAGE__", "io.poolster.multipart")
        .replace(
            "__CASES__",
            include_str!("../../tests/fixtures/multipart_probe_main.java"),
        );
    let start = source.find("        void assertRequest(").unwrap();
    let end = source[start..]
        .find("        public <T> HttpResponse<T> send(")
        .unwrap()
        + start;
    source.replace_range(
        start..end,
        include_str!("../../tests/fixtures/multipart_probe_assert.java"),
    );
    std::fs::write(
        dir.path()
            .join("sdk/src/test/java/io/poolster/multipart/PoolsterOperationTests.java"),
        source,
    )
    .unwrap();
    let output = std::process::Command::new("mvn")
        .args([
            "-q",
            "test-compile",
            "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
            "-Dexec.mainClass=io.poolster.multipart.PoolsterOperationTests",
            "-Dexec.classpathScope=test",
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
#[ignore = "requires JDK17+Maven; native complex multipart wire probe"]
fn native_multipart_complex_json_and_repeated_arrays() {
    let tree = poolster_core::engine::Packages::new()
        .package(
            crate::package("sdk")
                .name("io.poolster.multipart")
                .with(crate::sdk())
                .with(crate::operation_tests()),
        )
        .generate(&complex_api(), None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    let main=include_str!("../../tests/fixtures/multipart_probe_main.java").replace("file,null);","file,null,MAPPER.valueToTree(Map.of(\"type\",\"server_vad\",\"label\",\"café雪\",\"threshold\",0.5)),List.of(\"word\",\"segment\"),List.of(file,file),Map.of(\"extra\",Map.of(\"snow\",\"雪\")));");
    let mut source = include_str!("../../tests/fixtures/operation_driver.java")
        .replace("__PACKAGE__", "io.poolster.multipart")
        .replace("__CASES__", &main);
    let start = source.find("        void assertRequest(").unwrap();
    let end = start
        + source[start..]
            .find("        public <T> HttpResponse<T> send(")
            .unwrap();
    source.replace_range(
        start..end,
        include_str!("../../tests/fixtures/multipart_complex_assert.java"),
    );
    std::fs::write(
        dir.path()
            .join("sdk/src/test/java/io/poolster/multipart/PoolsterOperationTests.java"),
        source,
    )
    .unwrap();
    let output = std::process::Command::new("mvn")
        .args([
            "-q",
            "test-compile",
            "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
            "-Dexec.mainClass=io.poolster.multipart.PoolsterOperationTests",
            "-Dexec.classpathScope=test",
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
