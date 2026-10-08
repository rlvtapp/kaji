use super::*;

#[test]
fn emits_cursor_pagination_through_the_normal_operation() {
    let mut source = contact_api();
    source.operations[0].id = "listContacts".into();
    source.operations[0].path = "/v1/contacts".into();
    source.operations[0].request_body = None;
    source.operations[0].parameters = vec![OperationParameter {
        name: "cursor".into(),
        location: "query".into(),
        required: false,
        schema: Some(string()),
        description: None,
        annotations: BTreeMap::new(),
    }];
    source.operations[0].annotations.insert(
        "x-kaji-pagination".into(),
        serde_json::json!({
            "type": "cursor",
            "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
            "outputs": { "nextCursor": "$.data[-1].nextCursor" }
        }),
    );
    let tree = render_test_sdk(&source, "java", None).unwrap();
    let client = rendered_java(&tree);
    assert!(client.contains(
        "public java.lang.Iterable<Contact> listContactsPages(ListContactsRequest input)"
    ));
    assert!(client.contains("var page = listContacts(current);"));
    assert!(client.contains("current = ListContactsRequestWithCursor(current, cursor.asText());"));
    assert!(client.contains("protected static JsonNode kajiJsonPath"));
    assert!(client.contains("$.data[-1].nextCursor"));
    assert!(client.contains("Integer.parseInt"));

    let namespaced =
        render_sdk(&source, "java-namespaced", None, SdkClientStyle::Namespaced).unwrap();
    let resource = namespaced
        .get("java-namespaced/src/main/java/io/poolster/poolsteremailapi/ContactsResource.java")
        .unwrap();
    assert!(resource.contains("extends ContactsResourcePart000"));
    let sources = rendered_java(&namespaced);
    assert!(sources.contains(
        "public java.lang.Iterable<Contact> listPages(Client.ListContactsRequest input)"
    ));
    assert!(sources.contains("return client.listContactsPages(input);"));
}

#[test]
fn declared_page_results_have_defaults_bounds_and_validated_selectors() {
    let mut source = contact_api();
    let operation = &mut source.operations[0];
    operation.request_body = None;
    operation.parameters = vec![OperationParameter {
        name: "page".into(),
        location: "query".into(),
        required: false,
        schema: Some(integer()),
        description: None,
        annotations: BTreeMap::new(),
    }];
    operation.responses = vec![poolster_core::OperationResponse::json(
        "200",
        SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::new(SchemaKind::String)),
        }),
    )];
    operation.annotations.insert("x-kaji-pagination".into(), serde_json::json!({"type":"page", "inputs":[{"name":"page","type":"page"}], "outputs":{"results":"$"}}));
    let rendered = rendered_java(&render_test_sdk(&source, "java", None).unwrap());
    assert!(rendered.contains("input.page() == null ?"));
    assert!(rendered.contains("(input, 1L) : input"));
    assert!(rendered.contains("results.size() == 0"));
    assert!(rendered.contains("current.page() == Long.MAX_VALUE"));
    assert!(rendered.contains("++pageCount >= 10000"));
    source.operations[0]
        .annotations
        .get_mut("x-kaji-pagination")
        .unwrap()["outputs"]["results"] = serde_json::json!("$.missing");
    assert!(render_test_sdk(&source, "java", None).is_err());
}

#[test]
fn emits_page_and_offset_pagination_only_for_safe_query_parameters() {
    let mut page = contact_api();
    page.operations[0].id = "listContactPages".into();
    page.operations[0].path = "/v1/contacts".into();
    page.operations[0].request_body = None;
    page.operations[0].parameters = vec![OperationParameter {
        name: "page".into(),
        location: "query".into(),
        required: false,
        schema: Some(integer()),
        description: None,
        annotations: BTreeMap::new(),
    }];
    page.operations[0].annotations.insert(
        "x-speakeasy-pagination".into(),
        serde_json::json!({
            "type": "offsetLimit",
            "inputs": [{ "name": "page", "in": "parameters", "type": "page" }],
            "outputs": { "numPages": "$.meta.pages" }
        }),
    );
    let tree = render_test_sdk(&page, "java", None).unwrap();
    let client = rendered_java(&tree);
    assert!(client.contains("listContactPagesPages(ListContactPagesRequest input)"));
    assert!(
        client.contains("var numPages = kajiJsonPath(mapper.valueToTree(page), \"$.meta.pages\")")
    );
    assert!(client.contains("current = ListContactPagesRequestWithPage(current, nextValue);"));

    let mut offset = page;
    offset.operations[0].id = "listContactOffsets".into();
    offset.operations[0].parameters = vec![
        OperationParameter {
            name: "offset".into(),
            location: "query".into(),
            required: false,
            schema: Some(integer()),
            description: None,
            annotations: BTreeMap::new(),
        },
        OperationParameter {
            name: "limit".into(),
            location: "query".into(),
            required: false,
            schema: Some(integer()),
            description: None,
            annotations: BTreeMap::new(),
        },
    ];
    offset.operations[0].annotations.insert(
        "x-speakeasy-pagination".into(),
        serde_json::json!({
            "type": "offsetLimit",
            "inputs": [
                { "name": "offset", "in": "parameters", "type": "offset" },
                { "name": "limit", "in": "parameters", "type": "limit" }
            ],
            "outputs": { "results": "$.contacts" }
        }),
    );
    let tree = render_test_sdk(&offset, "java", None).unwrap();
    let client = rendered_java(&tree);
    assert!(
        client.contains("var results = kajiJsonPath(mapper.valueToTree(page), \"$.contacts\")")
    );
    assert!(client.contains(
        "current = ListContactOffsetsRequestWithOffset(current, currentValue + resultCount);"
    ));
}

#[test]
fn follows_declared_url_pagination_on_the_generated_client_origin() {
    let mut source = contact_api();
    source.operations[0].id = "listContacts".into();
    source.operations[0].annotations.insert(
        "x-speakeasy-pagination".into(),
        serde_json::json!({
            "type": "url",
            "inputs": [],
            "outputs": { "nextUrl": "$.links.next" }
        }),
    );
    let tree = render_test_sdk(&source, "java", None).unwrap();
    let client = rendered_java(&tree);
    assert!(client.contains(
        "public java.lang.Iterable<Contact> listContactsPages(ListContactsRequest input)"
    ));
    assert!(client.contains("listContactsFromPaginationUrl(input, nextUrl)"));
    assert!(client.contains("requestPaginationUrlWithRetry"));
    assert!(client.contains("Poolster pagination URL must remain on the configured API origin"));
    assert!(client.contains("\"$.links.next\""));
}

#[test]
fn supports_safe_header_and_rejects_invalid_path_java_pagination_contracts() {
    let mut source = contact_api();
    source.operations[0].id = "listContacts".into();
    source.operations[0].request_body = None;
    source.operations[0].parameters = vec![OperationParameter {
        name: "cursor".into(),
        location: "header".into(),
        required: false,
        schema: Some(string()),
        description: None,
        annotations: BTreeMap::new(),
    }];
    source.operations[0].annotations.insert(
        "x-kaji-pagination".into(),
        serde_json::json!({
            "type": "cursor",
            "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
            "outputs": { "nextCursor": "$.next" }
        }),
    );
    let tree = render_test_sdk(&source, "java", None).unwrap();
    let client = rendered_java(&tree);
    assert!(client.contains("listContactsPages("));

    source.operations[0].parameters[0].location = "path".into();
    source.operations[0].parameters[0].required = false;
    source.operations[0].path = "/contacts/{cursor}".into();
    let tree = render_test_sdk(&source, "java", None).unwrap();
    let client = rendered_java(&tree);
    assert!(!client.contains("listContactsPages("));

    source.operations[0].parameters[0].location = "query".into();
    source.operations[0].annotations.insert(
        "x-kaji-pagination".into(),
        serde_json::json!({
            "type": "offsetLimit",
            "inputs": [{ "name": "cursor", "in": "parameters", "type": "offset" }],
            "outputs": { "results": "$.items" }
        }),
    );
    let tree = render_test_sdk(&source, "java", None).unwrap();
    let client = rendered_java(&tree);
    assert!(!client.contains("listContactsPages("));
}
