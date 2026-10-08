use super::*;

#[test]
fn emits_offset_pagers_and_decoded_sse_iterators() {
    let mut source = api();
    let operation = &mut source.operations[0];
    operation.id = "listContacts".into();
    operation.path = "/contacts".into();
    operation.parameters = vec![
        OperationParameter {
            name: "offset".into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::Integer)),
            description: None,
            annotations: Default::default(),
        },
        OperationParameter {
            name: "limit".into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::Integer)),
            description: None,
            annotations: Default::default(),
        },
    ];
    operation.annotations.insert(
        "x-poolster-pagination".into(),
        serde_json::json!({
            "type": "offsetLimit",
            "inputs": [
                { "name": "offset", "in": "parameters", "type": "offset" },
                { "name": "limit", "in": "parameters", "type": "limit" }
            ],
            "outputs": { "results": "$.items" }
        }),
    );
    source.operations[1].id = "watchEvents".into();
    source.operations[1].responses = vec![OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "text/event-stream".into(),
            schema: None,
        }],
    }];
    let tree = render_test_sdk(&source, "sdk/python", Some("example-api-sdk")).unwrap();
    let client = rendered_python(&tree);
    assert!(client.contains("def list_contacts_pages(self, *, offset: int | None = None, limit: int | None = None) -> Iterator[Contact]:"));
    assert!(client.contains("results = _poolster_json_path(response, \"$.items\")"));
    assert!(client.contains("def watch_events(self) -> Iterator[Any]:"));
    assert!(client.contains("result = self._event_stream(\"GET\""));
    assert!(client.contains("Open an SSE response and yield decoded"));
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let status = Command::new("python3")
        .args(["-m", "compileall", "-q"])
        .env("PYTHONPYCACHEPREFIX", root.path().join("python-cache"))
        .arg(root.path().join("sdk/python/src"))
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn emits_body_cursor_paginators_only_for_required_declared_json_fields() {
    let mut source = api();
    {
        let operation = &mut source.operations[0];
        operation.id = "searchContacts".into();
        operation.method = HttpMethod::Post;
        operation.path = "/contacts/search".into();
        operation.parameters.clear();
        operation.request_body = Some(OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(SchemaValue::new(SchemaKind::Object {
                    fields: vec![Field {
                        name: "cursor-token".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: false,
                        annotations: Default::default(),
                    }],
                    additional_properties: AdditionalProperties::Forbidden,
                })),
            }],
        });
        operation.annotations.insert(
            "x-poolster-pagination".into(),
            serde_json::json!({
                "type": "cursor",
                "inputs": [{ "name": "cursor-token", "in": "requestBody", "type": "cursor" }],
                "outputs": { "nextCursor": "$.next" }
            }),
        );
    }
    let tree = render_test_sdk(&source, "sdk", Some("example-api-sdk")).unwrap();
    let client = rendered_python(&tree);
    assert!(client.contains(
        "def search_contacts_pages(self, *, body: dict[str, Any]) -> Iterator[Contact]:"
    ));
    assert!(client.contains("poolster_body = body"));
    assert!(client.contains("response = self.search_contacts(body=poolster_body)"));
    assert!(
        client.contains("_poolster_with_body_value(poolster_body, \"cursor-token\", next_cursor)")
    );

    source.operations[0].request_body.as_mut().unwrap().required = false;
    let omitted = render_test_sdk(&source, "sdk", Some("example-api-sdk")).unwrap();
    assert!(!rendered_python(&omitted).contains("def search_contacts_pages"));
}
