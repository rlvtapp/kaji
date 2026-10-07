#[test]
#[ignore = "requires cached generated Rust dependencies"]
fn native_multipart_buffered_parts_preserve_bytes_and_safe_retries() {
    let mut api = kaji_core::Api {
        name: "Multipart".into(),
        ..Default::default()
    };
    for (id, method) in [
        ("putUpload", kaji_core::HttpMethod::Put),
        ("postUpload", kaji_core::HttpMethod::Post),
    ] {
        api.operations.push(kaji_core::Operation {
            id: id.into(),
            method,
            path: "/upload".into(),
            request_body: Some(kaji_core::OperationRequestBody {
                required: true,
                description: None,
                media_types: vec![kaji_core::OperationMediaType {
                    content_type: "multipart/form-data".into(),
                    schema: None,
                }],
            }),
            ..Default::default()
        });
    }
    let mut mixed = api.operations[0].clone();
    mixed.id = "mixedUpload".into();
    mixed
        .request_body
        .as_mut()
        .unwrap()
        .media_types
        .push(kaji_core::OperationMediaType {
            content_type: "application/json".into(),
            schema: None,
        });
    api.operations.push(mixed);
    api.operations.push(kaji_core::Operation {
        id: "mixedUploadMultipart".into(),
        method: kaji_core::HttpMethod::Get,
        path: "/collision".into(),
        ..Default::default()
    });
    let array = kaji_core::SchemaValue::new(kaji_core::SchemaKind::Array {
        items: Box::new(kaji_core::SchemaValue::new(kaji_core::SchemaKind::Any)),
    });
    api.operations.push(kaji_core::Operation {
        id: "sequence".into(),
        method: kaji_core::HttpMethod::Post,
        path: "/sequence".into(),
        parameters: vec![kaji_core::OperationParameter {
            name: "whole_query".into(),
            location: "querystring".into(),
            required: true,
            schema: Some(kaji_core::SchemaValue::new(kaji_core::SchemaKind::Object {
                fields: vec![],
                additional_properties: kaji_core::AdditionalProperties::Any,
            })),
            description: None,
            annotations: Default::default(),
        }],
        request_body: Some(kaji_core::OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![kaji_core::OperationMediaType {
                content_type: "application/x-ndjson".into(),
                schema: Some(array.clone()),
            }],
        }),
        responses: vec![kaji_core::OperationResponse {
            status: "200".into(),
            description: None,
            media_types: vec![kaji_core::OperationMediaType {
                content_type: "application/x-ndjson".into(),
                schema: Some(array),
            }],
        }],
        ..Default::default()
    });
    let mut params = kaji_core::Operation {
        id: "jsonParameters".into(),
        method: kaji_core::HttpMethod::Get,
        path: "/params/{path}".into(),
        ..Default::default()
    };
    for (name, location) in [
        ("path", "path"),
        ("filter", "query"),
        ("x-json", "header"),
        ("cookie", "cookie"),
    ] {
        let mut parameter = kaji_core::OperationParameter {
            name: name.into(),
            location: location.into(),
            required: true,
            schema: Some(kaji_core::SchemaValue::new(kaji_core::SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        };
        parameter.annotations.insert("kaji.parameter_content".into(),serde_json::json!([{"content_type":"application/json","schema_definition":{"type":"string"}}]));
        params.parameters.push(parameter);
    }
    api.operations.push(params);
    let tree = kaji_core::engine::Packages::new()
        .package(
            crate::package("sdk")
                .with(crate::sdk().namespaced())
                .with(crate::operation_tests()),
        )
        .generate(&api, None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    let path = dir.path().join("sdk/src/client/mod.rs");
    let source = std::fs::read_to_string(&path).unwrap() + include_str!("multipart_probe.rs.txt");
    std::fs::write(path, source).unwrap();
    let result = crate::native_cargo()
        .args(["test", "--quiet"])
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
