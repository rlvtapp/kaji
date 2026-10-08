use super::*;

#[test]
fn emits_same_origin_url_pagination_without_bypassing_the_operation() {
    let mut source = api();
    source.operations[0].id = "listContacts".into();
    source.operations[0].path = "/v1/contacts".into();
    source.operations[0].parameters.clear();
    source.operations[0].annotations.insert(
        "x-kaji-pagination".into(),
        serde_json::json!({
            "type": "url",
            "outputs": { "nextUrl": "$.links.next" }
        }),
    );
    let tree = render_test_sdk(&source, "sdk", Some("example-api-sdk")).unwrap();
    let client = rendered_python(&tree);
    assert!(client.contains("def list_contacts_pages(self) -> Iterator[Contact]:"));
    assert!(client.contains("self.list_contacts(_poolster_pagination_url=poolster_url)"));
    assert!(client.contains("pagination URL must remain on the configured API origin"));
    assert!(client.contains("urljoin(url, pagination_url)"));
    assert!(client.contains("pagination_url=_poolster_pagination_url"));
}

#[test]
fn rejects_an_empty_output_directory() {
    assert!(render_test_sdk(&api(), "/", None).is_err());
}

#[test]
fn namespaced_style_exports_and_initializes_resource_facades() {
    let tree = render_sdk(
        &api(),
        "sdk/python",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let client = rendered_python(&tree);
    assert!(client.contains("self.contacts = ContactsResource(self)"));
    assert!(client.contains("class ContactsResource(ContactsResourcePart000):"));
    assert!(client.contains(
        "def get(self, *, contact_id: str, include_deleted: bool | None = None) -> Contact:"
    ));
    assert!(client.contains(
        "return self._client.get_contact(contact_id=contact_id, include_deleted=include_deleted)"
    ));
    let init = tree
        .get("sdk/python/src/example_api_sdk/__init__.py")
        .unwrap();
    assert!(init.contains("from .resources import *"));
    assert!(!init.contains("__all__"));
    assert!(
        tree.get("sdk/python/src/example_api_sdk/resources/chunks/exports_000.py")
            .unwrap()
            .contains("ContactsResource")
    );
    assert!(
        tree.get("sdk/python/STYLE_GUIDE.md")
            .unwrap()
            .contains("SdkClientStyle::Flat")
    );
    assert!(
        tree.get("sdk/python/README.md")
            .unwrap()
            .contains("client.contacts.get")
    );
}

#[test]
fn partitions_large_resource_facades_without_changing_the_namespace() {
    let mut source = api();
    let template = source.operations[0].clone();
    source.operations = (0..101)
        .map(|index| {
            let mut operation = template.clone();
            operation.id = format!("getContact{index}");
            operation
        })
        .collect();
    let tree = render_sdk(
        &source,
        "sdk",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    assert!(
        tree.iter()
            .any(|(path, _)| path.to_string_lossy().ends_with("_part_001.py"))
    );
    let facade = tree
        .iter()
        .find_map(|(path, contents)| {
            path.to_string_lossy()
                .ends_with("resources/contacts_17f458ee83d31930.py")
                .then_some(contents)
        })
        .unwrap();
    assert!(facade.contains("class ContactsResource(ContactsResourcePart001):"));
}

#[test]
fn namespaced_style_prefers_openapi_tags_over_path_segments() {
    let mut api = api();
    api.operations[0]
        .annotations
        .insert("tags".into(), serde_json::json!(["Recipients"]));
    let tree = render_sdk(
        &api,
        "sdk/python",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let client = rendered_python(&tree);
    assert!(client.contains("self.recipients = RecipientsResource(self)"));
    assert!(!client.contains("self.contacts = ContactsResource(self)"));
}

#[test]
fn namespaced_resource_attribute_never_shadows_a_flat_operation() {
    let mut api = api();
    api.operations[1].id = "authCheck".into();
    api.operations[1].path = "/v1/auth-check".into();
    let tree = render_sdk(
        &api,
        "sdk/python",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let client = rendered_python(&tree);
    assert!(client.contains("def auth_check(self) -> None:"));
    assert!(client.contains("self.auth_check_resource = AuthCheckResource(self)"));
    assert!(!client.contains("self.auth_check = AuthCheckResource(self)"));
}

#[test]
fn emits_declared_error_types_and_openapi_media_handling() {
    let mut api = api();
    api.schemas.push(Schema::new(
        "Problem",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "message".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: true,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    api.operations[0].responses = vec![
        OperationResponse {
            status: "200".into(),
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(SchemaValue::reference("#/components/schemas/Contact")),
            }],
        },
        OperationResponse {
            status: "404".into(),
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(SchemaValue::reference("#/components/schemas/Problem")),
            }],
        },
    ];
    api.operations[1].request_body = Some(OperationRequestBody {
        required: true,
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/x-www-form-urlencoded".into(),
            schema: Some(SchemaValue::new(SchemaKind::Object {
                fields: vec![],
                additional_properties: AdditionalProperties::Forbidden,
            })),
        }],
    });
    api.operations[1].responses = vec![OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/octet-stream".into(),
            schema: None,
        }],
    }];

    let tree = render_test_sdk(&api, "sdk/python", Some("example-api-sdk")).unwrap();
    let client = rendered_python(&tree);
    assert!(client.contains("class GetContactStatus404Error(ApiError):"));
    assert!(client.contains("body: Problem"));
    assert!(client.contains("error_types={404: (GetContactStatus404Error, Problem)}"));
    assert!(client.contains("dict(error.headers.items())"));
    assert!(client.contains("def health(self, *, body: dict[str, Any]) -> bytes:"));
    assert!(client.contains("body_kind=\"form\""));

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
fn emits_safe_retries_and_lifecycle_hooks() {
    let mut api = api();
    api.operations[1].method = HttpMethod::Post;
    api.operations[1].parameters.push(OperationParameter {
        name: "Idempotency-Key".into(),
        location: "header".into(),
        required: true,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: Default::default(),
    });
    let tree = render_test_sdk(&api, "sdk/python", Some("example-api-sdk")).unwrap();
    let client = rendered_python(&tree);
    assert!(client.contains("max_retries: int = 2"));
    assert!(client.contains("before_request: Callable[[dict[str, Any]], None] | None"));
    assert!(client.contains("status_code in {408, 429, 500, 502, 503, 504}"));
    assert!(client.contains("name.lower() == \"idempotency-key\""));
    assert!(client.contains("retryable=True"));
}
