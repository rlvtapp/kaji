use super::*;
use poolster_core::ast::{HttpMethod, SecurityRequirement, SecurityScheme, SecuritySchemeKind};

fn operation(id: &str, method: HttpMethod, path: &str) -> Operation {
    Operation {
        id: id.into(),
        method,
        path: path.into(),
        responses: vec![poolster_core::OperationResponse {
            status: "200".into(),
            description: None,
            media_types: vec![poolster_core::OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(poolster_core::SchemaValue::reference(
                    "#/components/schemas/Pet",
                )),
            }],
        }],
        annotations: Default::default(),
        ..Default::default()
    }
}

#[test]
fn renders_poolster_path_parameter_docs_and_source() {
    let source = render_operation(
        &operation("getPetById", HttpMethod::Get, "/pet/{petId}"),
        true,
        None,
    );
    assert!(source.contains("{@link /pet/:petId}"));
    assert!(source.contains("method: 'GET'"));
    assert!(source.contains("GetPetByIdOptions"));
}

#[test]
fn security_catalog_preserves_http_api_key_locations_and_oauth() {
    let mut operation = operation("getSecure", HttpMethod::Get, "/secure");
    operation.security = ["bearer", "header_key", "query_key", "oauth"]
        .into_iter()
        .map(|name| SecurityRequirement {
            schemes: [(name.into(), Vec::new())].into_iter().collect(),
        })
        .collect();
    let catalog = SecuritySchemeCatalog {
        schemes: vec![
            SecurityScheme {
                name: "bearer".into(),
                description: None,
                kind: SecuritySchemeKind::Http {
                    scheme: Some("bearer".into()),
                    bearer_format: Some("JWT".into()),
                },
            },
            SecurityScheme {
                name: "header_key".into(),
                description: None,
                kind: SecuritySchemeKind::ApiKey {
                    name: Some("X-API-Key".into()),
                    location: Some("header".into()),
                },
            },
            SecurityScheme {
                name: "query_key".into(),
                description: None,
                kind: SecuritySchemeKind::ApiKey {
                    name: Some("api_key".into()),
                    location: Some("query".into()),
                },
            },
            SecurityScheme {
                name: "oauth".into(),
                description: None,
                kind: SecuritySchemeKind::OAuth2 {
                    flows: Vec::new(),
                    metadata_url: None,
                },
            },
        ],
    };

    let source = render_operation(&operation, true, Some(&catalog));
    // SDK generation keeps OpenAPI's OR-of-AND alternatives rather than
    // flattening every scheme into one ambiguous list. The component key
    // is retained as the runtime credential lookup key.
    assert!(source.contains("security: [[{ id: 'bearer', type: 'http', scheme: 'bearer'"));
    assert!(
        source.contains("[{ id: 'header_key', type: 'apiKey', name: 'X-API-Key', in: 'header'")
    );
    assert!(source.contains("[{ id: 'query_key', type: 'apiKey', name: 'api_key', in: 'query'"));
    assert!(source.contains("id: 'oauth', type: 'oauth2'"));
}

#[test]
fn encoding_omission_and_required_sse_options_match_native_types() {
    let mut operation = operation("streamProbe", HttpMethod::Get, "/probe/{id}");
    operation
        .parameters
        .push(poolster_core::OperationParameter {
            name: "id".into(),
            location: "path".into(),
            required: true,
            schema: None,
            description: None,
            annotations: Default::default(),
        });
    operation.annotations.insert("poolster.request_body_encodings".into(), serde_json::json!({"multipart/form-data": {"payload": {"style":null,"explode":null,"contentType":null,"allowReserved":null}}}));
    assert_eq!(
        render_form_encodings(&operation).unwrap(),
        "{\"multipart/form-data\":{\"payload\":{}}}"
    );
    let source = render_event_stream_operation(&operation, false, None);
    assert!(source.contains("options: Options<StreamProbeOptions, ThrowOnError>,"));
    assert!(!source.contains("Options<StreamProbeOptions, ThrowOnError> = {}"));
    operation.parameters.clear();
    let source = render_event_stream_operation(&operation, false, None);
    assert!(source.contains("Options<StreamProbeOptions, ThrowOnError> = {}"));
}

#[test]
fn style_metadata_preserves_unsafe_wire_keys_as_string_properties() {
    let mut operation = operation("headerProbe", HttpMethod::Get, "/probe");
    for name in ["openai-beta", "x'quoted", "1st", "雪"] {
        operation
            .parameters
            .push(poolster_core::OperationParameter {
                name: name.into(),
                location: "header".into(),
                required: false,
                schema: None,
                description: None,
                annotations: std::collections::BTreeMap::from([(
                    "style".into(),
                    serde_json::json!("simple"),
                )]),
            });
    }
    let styles = render_parameter_styles(&operation).unwrap();
    for name in ["openai-beta", "x'quoted", "1st", "雪"] {
        assert!(styles.contains(&format!(
            "{}: {{ style: 'simple' }}",
            serde_json::to_string(name).unwrap()
        )));
    }
    assert_eq!(style_property_name("petId"), "petId");
}

#[test]
fn operation_composes_media_styles_and_security_metadata() {
    let mut operation = operation("updatePet", HttpMethod::Post, "/pets/{petId}");
    operation
        .parameters
        .push(poolster_core::OperationParameter {
            name: "petId".into(),
            location: "path".into(),
            required: true,
            schema: None,
            description: None,
            annotations: std::collections::BTreeMap::from([(
                "style".into(),
                serde_json::json!("matrix"),
            )]),
        });
    operation.request_body = Some(poolster_core::OperationRequestBody {
        required: true,
        description: None,
        media_types: vec![poolster_core::OperationMediaType {
            content_type: "multipart/form-data".into(),
            schema: None,
        }],
    });
    operation.annotations.insert(
        "poolster.request_body_encodings".into(),
        serde_json::json!({
            "multipart/form-data": {
                "metadata": {
                    "contentType": "application/json",
                    "explode": false,
                    "headers": {
                        "X-Part-Id": {
                            "required": true,
                            "style": "simple",
                            "schema_definition": { "type": "string" }
                        }
                    }
                }
            }
        }),
    );
    operation.security = vec![SecurityRequirement {
        schemes: [("cookie".into(), Vec::new())].into_iter().collect(),
    }];
    let catalog = SecuritySchemeCatalog {
        schemes: vec![SecurityScheme {
            name: "cookie".into(),
            description: None,
            kind: SecuritySchemeKind::ApiKey {
                name: Some("session".into()),
                location: Some("cookie".into()),
            },
        }],
    };
    let source = render_operation(&operation, true, Some(&catalog));
    assert!(source.contains("contentType: { request: 'multipart/form-data' }"));
    assert!(source.contains("formEncodings: {\"multipart/form-data\":{\"metadata\":{\"contentType\":\"application/json\",\"explode\":false,\"headers\":{\"X-Part-Id\":{\"required\":true,\"schema_definition\":{\"type\":\"string\"},\"style\":\"simple\"}}}}}"));
    assert!(source.contains("styles: { path: { petId: { style: 'matrix' } } }"));
    assert!(
        source.contains(
            "security: [[{ id: 'cookie', type: 'apiKey', name: 'session', in: 'cookie' }]]"
        )
    );
}
