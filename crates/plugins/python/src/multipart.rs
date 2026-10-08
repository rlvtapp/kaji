#[cfg(test)]
mod tests {
    use super::super::*;
    use poolster_core::{
        Field, HttpMethod, OperationMediaType, OperationParameter, OperationRequestBody,
    };
    use std::{fs, process::Command};
    #[test]
    #[ignore = "needs Python HTTPX and a native loopback HTTP server"]
    fn native_multipart_sync_async_preserve_mime_and_retry_bytes() {
        let mut api = Api {
            name: "Multipart".into(),
            version: "1.0.0".into(),
            schemas: vec![Schema::new(
                "UploadRequest",
                SchemaValue::new(SchemaKind::OneOf {
                    variants: vec![
                        SchemaValue::new(SchemaKind::String),
                        SchemaValue::new(SchemaKind::Object {
                            fields: vec![],
                            additional_properties: AdditionalProperties::Any,
                        }),
                    ],
                }),
            )],
            operations: vec![Operation {
                id: "createTranscription".into(),
                method: HttpMethod::Post,
                path: "/upload".into(),
                security: vec![],
                parameters: vec![OperationParameter {
                    name: "Idempotency-Key".into(),
                    location: "header".into(),
                    required: false,
                    schema: Some(SchemaValue::new(SchemaKind::String)),
                    description: None,
                    annotations: Default::default(),
                }],
                request_body: Some(OperationRequestBody {
                    required: true,
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "multipart/form-data".into(),
                        schema: Some(SchemaValue::reference("#/components/schemas/UploadRequest")),
                    }],
                }),
                responses: vec![],
                annotations: BTreeMap::from([(
                    "x-kaji-idempotency-resolved".into(),
                    serde_json::json!({"header":"Idempotency-Key","parameter_name":"Idempotency-Key","auto_generate":false}),
                )]),
            }],
            ..Default::default()
        };
        api.schemas.push(Schema::new(
            "VoiceOptions",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![
                    Field {
                        name: "normal".into(),
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
                ],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        ));
        let mut mixed = api.operations[0].clone();
        mixed.id = "createVoice".into();
        mixed.path = "/mixed".into();
        mixed.request_body.as_mut().unwrap().media_types.insert(
            0,
            OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(SchemaValue::reference("#/components/schemas/VoiceOptions")),
            },
        );
        let mut json_only = mixed.clone();
        json_only.id = "jsonOnly".into();
        json_only.path = "/json".into();
        json_only
            .request_body
            .as_mut()
            .unwrap()
            .media_types
            .truncate(1);
        api.operations.extend([mixed, json_only]);
        let root = tempfile::tempdir().unwrap();
        render_sdk_with_async(
            &api,
            "sdk",
            Some("multipart-sdk"),
            SdkClientStyle::Namespaced,
            true,
        )
        .unwrap()
        .write_to(root.path())
        .unwrap();
        fs::write(
            root.path().join("sdk/probe.py"),
            include_str!("multipart_probe.py"),
        )
        .unwrap();
        let python = std::env::var_os("KAJI_TEST_PYTHON").unwrap_or_else(|| "python3".into());
        let mut paths = vec![root.path().join("sdk/src")];
        if let Some(path) = std::env::var_os("PYTHONPATH") {
            paths.extend(std::env::split_paths(&path));
        }
        let result = Command::new(python)
            .arg("probe.py")
            .current_dir(root.path().join("sdk"))
            .env("PYTHONPATH", std::env::join_paths(paths).unwrap())
            .env("PYTHONDONTWRITEBYTECODE", "1")
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
