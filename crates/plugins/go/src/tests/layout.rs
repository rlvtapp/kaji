use super::*;

#[test]
fn generated_client_exposes_declared_offset_pagers() {
    let mut api = contact_api();
    let operation = &mut api.operations[0];
    operation.id = "listContacts".into();
    operation.path = "/v1/contacts".into();
    operation.parameters = vec![
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
    operation.annotations.insert(
        "x-kaji-pagination".into(),
        serde_json::json!({
            "type": "offsetLimit",
            "inputs": [
                { "name": "offset", "in": "parameters", "type": "offset" },
                { "name": "limit", "in": "parameters", "type": "limit" }
            ],
            "outputs": { "results": "$.items" }
        }),
    );
    let root = tempfile::tempdir().unwrap();
    let tree = render_test_sdk(&api, "sdk", Some("email")).unwrap();
    let client = all_source(&tree);
    assert!(client.contains("type ListContactsPager struct"));
    assert!(client.contains(
        "func (client *Client) ListContactsPages(input *ListContactsRequest) *ListContactsPager"
    ));
    assert!(client.contains("poolsterPaginationArrayLen(response, \"$.items\")"));
    tree.write_to(root.path()).unwrap();
    let status = Command::new("go")
        .args(["test", "./..."])
        .current_dir(root.path().join("sdk"))
        .env("GOCACHE", root.path().join("go-cache"))
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn emits_void_operations_without_a_result_value() {
    let mut api = contact_api();
    api.operations[0].id = "deleteContact".into();
    api.operations[0].responses.clear();
    api.operations[0].request_body = None;
    let tree = render_test_sdk(&api, "go", None).unwrap();
    let client = all_source(&tree);
    assert!(client.contains("func (client *Client) DeleteContact(ctx context.Context, input *DeleteContactRequest) error"));
    assert!(client.contains("return err\n\t}\n\treturn nil"));
}

#[test]
fn output_directory_is_validated_and_names_are_deterministic() {
    assert!(render_test_sdk(&contact_api(), "../escape", None).is_err());
    assert_eq!(go_package_name("Go API 2"), "goapi2");
    assert_eq!(go_type_name("x-request_id"), "XRequestID");
    assert_eq!(go_module_name("Poolster Email API"), "poolster-email-api");
}

#[test]
fn imports_json_when_a_model_uses_a_composed_schema() {
    let mut api = contact_api();
    api.schemas.push(Schema::new(
        "Event",
        SchemaValue::new(SchemaKind::OneOf {
            variants: vec![SchemaValue::reference("#/components/schemas/Contact")],
        }),
    ));
    let tree = render_test_sdk(&api, "go", None).unwrap();
    let models = all_source(&tree);
    assert!(models.contains("\"encoding/json\""));
    assert!(models.contains("type Event = json.RawMessage"));
}

#[test]
fn avoids_runtime_model_collisions_and_maps_unknown_operations_to_any() {
    let mut api = contact_api();
    api.schemas.push(Schema::new(
        "APIError",
        SchemaValue::new(SchemaKind::Object {
            fields: Vec::new(),
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    api.operations[0].responses = vec![OperationResponse::json("200", SchemaValue::unknown())];
    let tree = render_test_sdk(&api, "go", None).unwrap();
    let models = all_source(&tree);
    let client = all_source(&tree);
    assert!(models.contains("type APIError struct"));
    assert!(client.contains("type poolsterAPIError struct"));
    assert!(
        client.contains("GetContact(ctx context.Context, input *GetContactRequest) (*any, error)")
    );
    assert!(!client.contains("*Unknown"));
}

#[test]
fn namespaced_style_initializes_resource_facades_and_documents_both_styles() {
    let tree = render_sdk(&contact_api(), "go", None, SdkClientStyle::Namespaced, 0).unwrap();
    let client = all_source(&tree);
    assert!(client.contains("Contacts *ContactsService"));
    assert!(client.contains("client.Contacts = &ContactsService{client: client}"));
    assert!(client.contains("func (service *ContactsService) Get(ctx context.Context, input *GetContactRequest) (*Contact, error)"));
    assert!(client.contains("return service.client.GetContact(ctx, input)"));
    assert!(
        tree.get("go/STYLE_GUIDE.md")
            .unwrap()
            .contains("SdkClientStyle::Flat")
    );
    assert!(
        tree.get("go/README.md")
            .unwrap()
            .contains("client.Contacts.Get")
    );
}

#[test]
fn namespaced_style_prefers_openapi_tags_over_path_segments() {
    let mut api = contact_api();
    api.operations[0]
        .annotations
        .insert("tags".into(), serde_json::json!(["Recipients"]));
    let tree = render_sdk(&api, "go", None, SdkClientStyle::Namespaced, 0).unwrap();
    let client = all_source(&tree);
    assert!(client.contains("Recipients *RecipientsService"));
    assert!(!client.contains("Contacts *ContactsService"));
}

#[test]
fn namespaced_style_disambiguates_resource_fields_from_direct_methods() {
    let mut api = contact_api();
    api.operations[0].id = "authCheck".into();
    api.operations[0].path = "/v1/email/auth-check".into();
    let tree = render_sdk(&api, "go", None, SdkClientStyle::Namespaced, 0).unwrap();
    let client = all_source(&tree);
    assert!(client.contains("AuthCheckResource *AuthCheckResourceService"));
    assert!(client.contains("func (client *Client) AuthCheck("));
    assert!(client.contains("func (service *AuthCheckResourceService) AuthCheck("));
}
#[test]
fn middleware_rewrites_recovers_and_short_circuits_native_requests() {
    let documentation = render_readme(&contact_api(), SdkClientStyle::Flat);
    assert!(documentation.contains("Middleware: []PoolsterMiddleware{addHeader}"));
    assert!(documentation.contains("request.Clone(request.Context())"));
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&contact_api(), "sdk", Some("email"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(root.path().join("sdk/middleware_test.go"), r#"package email
import ("context"; "errors"; "io"; "net/http"; "strings"; "testing")
func TestMiddleware(t *testing.T) {
    seen := []string{}
    ctx, cancel := context.WithCancel(context.Background()); defer cancel()
    transport := PoolsterHTTPClientFunc(func(request *http.Request) (*http.Response, error) {
        if request.Context() != ctx || request.Header.Get("X-Custom") != "yes" { t.Fatal("request rewrite/context lost") }
        return nil, errors.New("offline")
    })
    outer := func(next PoolsterHTTPClient) PoolsterHTTPClient { return PoolsterHTTPClientFunc(func(request *http.Request) (*http.Response, error) {
        seen = append(seen, "before"); request.Header.Set("X-Custom", "yes")
        response, err := next.Do(request); seen = append(seen, "after"); return response, err
    }) }
    recover := func(next PoolsterHTTPClient) PoolsterHTTPClient { return PoolsterHTTPClientFunc(func(request *http.Request) (*http.Response, error) {
        _, err := next.Do(request); if err == nil { t.Fatal("expected transport error") }
        return &http.Response{StatusCode: 200, Header: http.Header{}, Body: io.NopCloser(strings.NewReader(`{"recovered":true}`))}, nil
    }) }
    client, err := NewClient(ClientConfig{BaseURL: "https://example.test", HTTPClient: transport, Middleware: []PoolsterMiddleware{outer, recover}})
    if err != nil { t.Fatal(err) }
    request, _ := client.newRequest(ctx, "GET", "/", nil, nil, nil)
    var body map[string]bool
    if err := client.doWithRetry(request, &body); err != nil || !body["recovered"] { t.Fatalf("%v %#v", err, body) }
    if strings.Join(seen, ",") != "before,after" { t.Fatal(seen) }
    short := func(PoolsterHTTPClient) PoolsterHTTPClient { return PoolsterHTTPClientFunc(func(request *http.Request) (*http.Response, error) {
        return &http.Response{StatusCode: 204, Header: http.Header{}, Body: io.NopCloser(strings.NewReader(""))}, nil
    }) }
    client, err = NewClient(ClientConfig{BaseURL: "https://example.test", Middleware: []PoolsterMiddleware{short}})
    if err != nil { t.Fatal(err) }
    if err := client.doWithRetry(request, nil); err != nil { t.Fatal(err) }
    _, err = NewClient(ClientConfig{BaseURL: "https://example.test", Middleware: []PoolsterMiddleware{nil}})
    if err == nil { t.Fatal("nil middleware accepted") }
}
"#).unwrap();
    assert!(
        Command::new("go")
            .args(["test", "./..."])
            .current_dir(root.path().join("sdk"))
            .env("GOCACHE", root.path().join("go-cache"))
            .status()
            .unwrap()
            .success()
    );
}
