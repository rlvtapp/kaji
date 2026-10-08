use super::*;

#[test]
fn emits_cursor_paginators_only_for_declared_operation_inputs() {
    let mut source = api();
    source.operations[0].id = "listPets".into();
    source.operations[0].path = "/pets".into();
    source.operations[0].parameters = vec![OperationParameter {
        name: "cursor".into(),
        location: "query".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: BTreeMap::new(),
    }];
    source.operations[0].annotations.insert(
        "x-speakeasy-pagination".into(),
        serde_json::json!({
            "type": "cursor",
            "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
            "outputs": { "nextCursor": "$.page.cursors[-1]" }
        }),
    );

    let tree = render_sdk(
        &source,
        "php",
        Some("acme/pet-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let client = tree.get("php/src/Client.php").unwrap();
    let operations = tree.get("php/src/ClientOperations000.php").unwrap();
    let resource = tree
        .get("php/src/Resources/PetsResourceOperations000.php")
        .unwrap();
    assert!(
        operations.contains("public function listPetsPages(?string $cursor = null): \\Generator")
    );
    assert!(operations.contains("$response = $this->listPets($poolsterCursor);"));
    assert!(operations.contains("self::poolsterJsonPath($response, '$.page.cursors[-1]')"));
    assert!(
        client.contains(
            "private static function poolsterJsonPath(mixed $value, string $path): mixed"
        )
    );
    assert!(
        resource.contains("public function listPages(?string $cursor = null): \\Generator"),
        "{resource}"
    );

    source.operations[0].annotations.insert(
        "x-speakeasy-pagination".into(),
        serde_json::json!({
            "type": "cursor",
            "inputs": [{ "name": "missing", "in": "parameters", "type": "cursor" }],
            "outputs": { "nextCursor": "$.next" }
        }),
    );
    let without_input = render_test_sdk(&source, "php", Some("acme/pet-sdk")).unwrap();
    assert!(
        !without_input
            .get("php/src/ClientOperations000.php")
            .unwrap()
            .contains("public function listPetsPages")
    );
}

#[test]
fn emits_offset_paginators_only_for_declared_integer_inputs() {
    let mut source = api();
    source.operations[0].id = "listPets".into();
    source.operations[0].path = "/pets".into();
    source.operations[0].parameters = vec![
        OperationParameter {
            name: "offset".into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::Integer)),
            description: None,
            annotations: BTreeMap::new(),
        },
        OperationParameter {
            name: "limit".into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::Integer)),
            description: None,
            annotations: BTreeMap::new(),
        },
    ];
    source.operations[0].annotations.insert(
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
    let tree = render_test_sdk(&source, "php", Some("acme/pet-sdk")).unwrap();
    let operations = tree.get("php/src/ClientOperations000.php").unwrap();
    assert!(operations.contains(
        "public function listPetsPages(?int $offset = null, ?int $limit = null): \\Generator"
    ));
    assert!(operations.contains("$poolsterOffset = $offset ?? 0;"));
    assert!(operations.contains("self::poolsterJsonPath($response, '$.items')"));
}

#[test]
fn emits_safe_body_cursor_and_url_paginators() {
    let mut source = api();
    source.operations[0].id = "searchPets".into();
    source.operations[0].method = HttpMethod::Post;
    source.operations[0].path = "/pets/search".into();
    source.operations[0].parameters.clear();
    source.operations[0].request_body = Some(OperationRequestBody {
        required: true,
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(SchemaValue::new(SchemaKind::Object {
                fields: vec![Field {
                    name: "cursor-token".into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: false,
                    annotations: BTreeMap::new(),
                }],
                additional_properties: AdditionalProperties::Forbidden,
            })),
        }],
    });
    source.operations[0].annotations.insert(
        "x-poolster-pagination".into(),
        serde_json::json!({
            "type": "cursor",
            "inputs": [{ "name": "cursor-token", "in": "requestBody", "type": "cursor" }],
            "outputs": { "nextCursor": "$.next" }
        }),
    );
    let tree = render_test_sdk(&source, "php", Some("acme/pet-sdk")).unwrap();
    let operations = tree.get("php/src/ClientOperations000.php").unwrap();
    assert!(operations.contains("public function searchPetsPages(array $body): \\Generator"));
    assert!(operations.contains("$response = $this->searchPets($poolsterBody);"));
    assert!(
        operations
            .contains("self::poolsterWithBodyValue($poolsterBody, 'cursor-token', $nextCursor)")
    );

    source.operations[0].request_body.as_mut().unwrap().required = false;
    let omitted = render_test_sdk(&source, "php", Some("acme/pet-sdk")).unwrap();
    assert!(
        !omitted
            .get("php/src/ClientOperations000.php")
            .unwrap()
            .contains("public function searchPetsPages")
    );

    source.operations[0].request_body = None;
    source.operations[0].method = HttpMethod::Get;
    source.operations[0].annotations.insert(
        "x-poolster-pagination".into(),
        serde_json::json!({
            "type": "url",
            "outputs": { "nextUrl": "$.links.next" }
        }),
    );
    let tree = render_sdk(
        &source,
        "php",
        Some("acme/pet-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let client = tree.get("php/src/Client.php").unwrap();
    let operations = tree.get("php/src/ClientOperations000.php").unwrap();
    let resource = tree
        .get("php/src/Resources/PetsResourceOperations000.php")
        .unwrap();
    assert!(
        operations
            .contains("public function searchPets(?string $_poolsterPaginationUrl = null): Pet")
    );
    assert!(operations.contains("$this->searchPets(_poolsterPaginationUrl: $poolsterUrl)"));
    assert!(client.contains("bool $retryable = false, ?string $paginationUrl = null, ?string $idempotencyHeader = null): string"));
    assert!(client.contains("private function resolvePaginationUrl(string $nextUrl): string"));
    assert!(client.contains("pagination URL must remain on the configured API origin"));
    assert!(resource.contains("public function searchPages(): \\Generator"));
}
