#[test]
#[ignore = "requires cached generated Rust dependencies"]
fn native_multipart_buffered_parts_preserve_bytes_and_safe_retries() {
    let mut api = poolster_core::Api {
        name: "Multipart".into(),
        ..Default::default()
    };
    for (id, method) in [
        ("putUpload", poolster_core::HttpMethod::Put),
        ("postUpload", poolster_core::HttpMethod::Post),
    ] {
        api.operations.push(poolster_core::Operation {
            id: id.into(),
            method,
            path: "/upload".into(),
            request_body: Some(poolster_core::OperationRequestBody {
                required: true,
                description: None,
                media_types: vec![poolster_core::OperationMediaType {
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
        .push(poolster_core::OperationMediaType {
            content_type: "application/json".into(),
            schema: None,
        });
    api.operations.push(mixed);
    api.operations.push(poolster_core::Operation {
        id: "mixedUploadMultipart".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/collision".into(),
        ..Default::default()
    });
    let array = poolster_core::SchemaValue::new(poolster_core::SchemaKind::Array {
        items: Box::new(poolster_core::SchemaValue::new(
            poolster_core::SchemaKind::Any,
        )),
    });
    api.operations.push(poolster_core::Operation {
        id: "sequence".into(),
        method: poolster_core::HttpMethod::Post,
        path: "/sequence".into(),
        parameters: vec![poolster_core::OperationParameter {
            name: "whole_query".into(),
            location: "querystring".into(),
            required: true,
            schema: Some(poolster_core::SchemaValue::new(
                poolster_core::SchemaKind::Object {
                    fields: vec![],
                    additional_properties: poolster_core::AdditionalProperties::Any,
                },
            )),
            description: None,
            annotations: Default::default(),
        }],
        request_body: Some(poolster_core::OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![poolster_core::OperationMediaType {
                content_type: "application/x-ndjson".into(),
                schema: Some(array.clone()),
            }],
        }),
        responses: vec![poolster_core::OperationResponse {
            status: "200".into(),
            description: None,
            media_types: vec![poolster_core::OperationMediaType {
                content_type: "application/x-ndjson".into(),
                schema: Some(array),
            }],
        }],
        ..Default::default()
    });
    let mut params = poolster_core::Operation {
        id: "jsonParameters".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/params/{path}".into(),
        ..Default::default()
    };
    for (name, location) in [
        ("path", "path"),
        ("filter", "query"),
        ("x-json", "header"),
        ("cookie", "cookie"),
    ] {
        let mut parameter = poolster_core::OperationParameter {
            name: name.into(),
            location: location.into(),
            required: true,
            schema: Some(poolster_core::SchemaValue::new(
                poolster_core::SchemaKind::String,
            )),
            description: None,
            annotations: Default::default(),
        };
        parameter.annotations.insert("poolster.parameter_content".into(),serde_json::json!([{"content_type":"application/json","schema_definition":{"type":"string"}}]));
        params.parameters.push(parameter);
    }
    api.operations.push(params);
    let tree = poolster_core::engine::Packages::new()
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
