use poolster_core::openapi32::{parameter_content, request_content, response_content};
use poolster_core::{SchemaKind, SecuritySchemeKind};
use poolster_input_openapi::openapi_sidecar::load_operations;

#[test]
fn typed_content_preserves_query_stream_and_nested_encoding() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path();
    std::fs::create_dir(dir.join("operations")).unwrap();
    std::fs::write(
        dir.join("operations.json"),
        r#"{"GET /events":"events.json"}"#,
    )
    .unwrap();
    std::fs::write(dir.join("operations-order.json"), r#"["GET /events"]"#).unwrap();
    std::fs::write(dir.join("schemas.json"), r#"{"schemas":[]}"#).unwrap();
    std::fs::write(dir.join("security-schemes.json"),r#"{"schemes":[{"name":"device","type":"oauth2","oauth_flows":[{"type":"deviceAuthorization","device_authorization_url":"https://auth.example/device","token_url":"https://auth.example/token"}]}]}"#).unwrap();
    std::fs::write(dir.join("api-metadata.json"),r#"{"self":"https://example.test/api.json","tags":[{"name":"events","parent":"api","summary":"Events","kind":"resource"}]}"#).unwrap();
    std::fs::write(dir.join("operations/events.json"),r#"{"path":"/events","method":"GET","parameters":[{"name":"filter","in":"querystring","required":true,"content":[{"content_type":"application/json","schema_definition":{"type":"object"}}]}],"request_body":{"media_types":[{"content_type":"multipart/mixed","item_schema_definition":{"type":"string"},"prefix_encoding":[{"contentType":"multipart/mixed","prefixEncoding":[{"contentType":"image/png"}],"itemEncoding":{"contentType":"text/plain"}}],"item_encoding":{"contentType":"application/json"}}]},"responses":[{"code":"200","content_type":"application/json-seq","item_schema_definition":{"type":"integer"}},{"code":"200","content_type":"multipart/mixed; version=1","item_schema_definition":{"type":"string"}}]}"#).unwrap();
    let api = load_operations(dir, "API".into(), "1".into()).unwrap();
    let op = &api.operations[0];
    assert_eq!(op.parameters[0].location, "querystring");
    assert!(op.parameters[0].schema.is_some());
    assert_eq!(
        parameter_content(&op.parameters[0]).unwrap()[0].content_type,
        "application/json"
    );
    assert!(matches!(
        op.success_schema().unwrap().kind,
        SchemaKind::Array { .. }
    ));
    let responses = response_content(op).unwrap();
    assert_eq!(responses[0].status, "200");
    assert!(matches!(
        responses[0].content.item_schema().unwrap().kind,
        SchemaKind::Integer
    ));
    let binary = &op.responses[0].media_types[1].schema.as_ref().unwrap();
    assert!(matches!(binary.kind, SchemaKind::String));
    assert_eq!(binary.format.as_deref(), Some("binary"));
    assert!(matches!(
        response_content(op).unwrap()[1]
            .content
            .item_schema()
            .unwrap()
            .kind,
        SchemaKind::String
    ));
    let semantics = poolster_core::semantics::analyze_operation(op, None);
    assert_eq!(
        semantics.streaming,
        Some(poolster_core::semantics::StreamingKind::Binary)
    );
    assert_eq!(
        semantics.request_body,
        Some(poolster_core::semantics::RequestBodyKind::Multipart)
    );
    let body = request_content(op).unwrap();
    assert_eq!(
        body[0].prefix_encoding[0].prefix_encoding[0]
            .content_type
            .as_deref(),
        Some("image/png")
    );
    assert_eq!(
        body[0].prefix_encoding[0]
            .item_encoding
            .as_ref()
            .unwrap()
            .content_type
            .as_deref(),
        Some("text/plain")
    );
    assert_eq!(
        api.annotations["poolster.openapi.metadata"]["tags"][0]["parent"],
        "api"
    );
    let operation_path = dir.join("operations/events.json");
    let mut raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&operation_path).unwrap()).unwrap();
    raw["responses"][0]["schema_definition"] = serde_json::json!({"type":"string"});
    std::fs::write(&operation_path, serde_json::to_vec(&raw).unwrap()).unwrap();
    let combined = load_operations(dir, "API".into(), "1".into()).unwrap();
    assert!(matches!(
        combined.operations[0].success_schema().unwrap().kind,
        SchemaKind::Array { .. }
    ));
    raw["responses"][0]["content_type"] = "text/event-stream".into();
    std::fs::write(&operation_path, serde_json::to_vec(&raw).unwrap()).unwrap();
    let streamed = load_operations(dir, "API".into(), "1".into()).unwrap();
    assert!(matches!(
        streamed.operations[0].success_schema().unwrap().kind,
        SchemaKind::Integer
    ));
    let security = poolster_input_openapi::openapi_sidecar::OpenApiSidecar::new(dir, "API", "1");
    let catalog = poolster_core::Adapter::adapt(&security)
        .unwrap()
        .security_schemes;
    let SecuritySchemeKind::OAuth2 { flows, .. } = &catalog.schemes[0].kind else {
        panic!("oauth metadata lost")
    };
    assert_eq!(
        flows[0].device_authorization_url.as_deref(),
        Some("https://auth.example/device")
    );
}
