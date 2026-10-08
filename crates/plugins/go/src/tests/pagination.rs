use super::*;

#[test]
fn generated_client_exposes_declared_cursor_pagers() {
    let mut api = contact_api();
    api.schemas.push(Schema::new(
        "ContactPage",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "next_cursor".into(),
                value: string_schema(),
                required: false,
                annotations: BTreeMap::new(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    let operation = &mut api.operations[0];
    operation.id = "listContacts".into();
    operation.path = "/v1/contacts".into();
    operation.request_body = None;
    operation.parameters = vec![OperationParameter {
        name: "cursor".into(),
        location: "query".into(),
        required: false,
        schema: Some(string_schema()),
        description: None,
        annotations: BTreeMap::new(),
    }];
    operation.responses = vec![OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(SchemaValue::reference("#/components/schemas/ContactPage")),
        }],
    }];
    operation.annotations.insert(
        "x-poolster-pagination".into(),
        serde_json::json!({
            "type": "cursor",
            "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
            "outputs": { "nextCursor": "$.next_cursor" }
        }),
    );
    let root = tempfile::tempdir().unwrap();
    let tree = render_test_sdk(&api, "sdk", Some("email")).unwrap();
    let client = all_source(&tree);
    assert!(client.contains("type ListContactsPager struct"));
    assert!(client.contains(
        "func (client *Client) ListContactsPages(input *ListContactsRequest) *ListContactsPager"
    ));
    tree.write_to(root.path()).unwrap();
    fs::write(
            root.path().join("sdk/pager_test.go"),
            r#"package email

import (
    "context"
    "io"
    "net/http"
    "strings"
    "testing"
)

type pagerTransport struct { requests int }
func (transport *pagerTransport) RoundTrip(request *http.Request) (*http.Response, error) {
    transport.requests++
    body := `{}`
    if transport.requests == 1 { body = `{"next_cursor":"cursor_2"}` }
    return &http.Response{StatusCode: http.StatusOK, Header: http.Header{}, Body: io.NopCloser(strings.NewReader(body)), Request: request}, nil
}

func TestPoolsterCursorPager(t *testing.T) {
    transport := &pagerTransport{}
    client, err := NewClient(ClientConfig{BaseURL: "https://example.test", HTTPClient: &http.Client{Transport: transport}, Retry: &RetryConfig{MaxAttempts: 1}})
    if err != nil { t.Fatal(err) }
    pager := client.ListContactsPages(&ListContactsRequest{})
    first, err := pager.Next(context.Background())
    if err != nil || first.NextCursor == nil || *first.NextCursor != "cursor_2" { t.Fatalf("first = %#v, err = %v", first, err) }
    _, err = pager.Next(context.Background())
    if err != nil { t.Fatal(err) }
    _, err = pager.Next(context.Background())
    if err != io.EOF { t.Fatalf("err = %v, want io.EOF", err) }
    if transport.requests != 2 { t.Fatalf("requests = %d", transport.requests) }
}
"#,
        )
        .unwrap();
    let status = Command::new("go")
        .args(["test", "./..."])
        .current_dir(root.path().join("sdk"))
        .env("GOCACHE", root.path().join("go-cache"))
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn generated_client_preserves_typed_body_cursor_pagination() {
    let mut api = contact_api();
    api.schemas.push(Schema::new(
        "ListContactsBody",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "cursor".into(),
                value: string_schema(),
                required: false,
                annotations: BTreeMap::new(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    api.schemas.push(Schema::new(
        "ContactPage",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "next_cursor".into(),
                value: string_schema(),
                required: false,
                annotations: BTreeMap::new(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    let operation = &mut api.operations[0];
    operation.id = "listContacts".into();
    operation.path = "/v1/contacts/search".into();
    operation.parameters.clear();
    operation.request_body = Some(OperationRequestBody {
        required: false,
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(SchemaValue::reference(
                "#/components/schemas/ListContactsBody",
            )),
        }],
    });
    operation.responses = vec![OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(SchemaValue::reference("#/components/schemas/ContactPage")),
        }],
    }];
    operation.annotations.insert(
        "x-poolster-pagination".into(),
        serde_json::json!({
            "type": "cursor",
            "inputs": [{ "name": "cursor", "in": "requestBody", "type": "cursor" }],
            "outputs": { "nextCursor": "$.next_cursor" }
        }),
    );
    let root = tempfile::tempdir().unwrap();
    let tree = render_test_sdk(&api, "sdk", Some("email")).unwrap();
    let client = all_source(&tree);
    assert!(client.contains("bodyCopy := *pager.input.Body"));
    assert!(client.contains("bodyCopy.Cursor = &pager.cursor"));
    tree.write_to(root.path()).unwrap();
    fs::write(
            root.path().join("sdk/body_pager_test.go"),
            r#"package email

import (
    "context"
    "io"
    "net/http"
    "strings"
    "testing"
)

type bodyPagerTransport struct { requests int; bodies []string }
func (transport *bodyPagerTransport) RoundTrip(request *http.Request) (*http.Response, error) {
    transport.requests++
    body, _ := io.ReadAll(request.Body)
    transport.bodies = append(transport.bodies, string(body))
    response := `{}`
    if transport.requests == 1 { response = `{"next_cursor":"cursor_2"}` }
    return &http.Response{StatusCode: http.StatusOK, Header: http.Header{}, Body: io.NopCloser(strings.NewReader(response)), Request: request}, nil
}

func TestPoolsterBodyCursorPager(t *testing.T) {
    transport := &bodyPagerTransport{}
    client, err := NewClient(ClientConfig{BaseURL: "https://example.test", HTTPClient: &http.Client{Transport: transport}, Retry: &RetryConfig{MaxAttempts: 1}})
    if err != nil { t.Fatal(err) }
    body := &ListContactsBody{}
    pager := client.ListContactsPages(&ListContactsRequest{Body: body})
    if _, err = pager.Next(context.Background()); err != nil { t.Fatal(err) }
    if _, err = pager.Next(context.Background()); err != nil { t.Fatal(err) }
    if body.Cursor != nil { t.Fatalf("caller body cursor was mutated: %q", *body.Cursor) }
    if len(transport.bodies) != 2 || transport.bodies[0] != `{}` || transport.bodies[1] != `{"cursor":"cursor_2"}` {
        t.Fatalf("bodies = %#v", transport.bodies)
    }
}
"#,
        )
        .unwrap();
    let status = Command::new("go")
        .args(["test", "./..."])
        .current_dir(root.path().join("sdk"))
        .env("GOCACHE", root.path().join("go-cache"))
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn generated_client_follows_only_same_origin_url_pagination() {
    let mut api = contact_api();
    api.schemas.push(Schema::new(
        "ContactPage",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "next".into(),
                value: string_schema(),
                required: false,
                annotations: BTreeMap::new(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    let operation = &mut api.operations[0];
    operation.id = "listContacts".into();
    operation.path = "/v1/contacts".into();
    operation.request_body = None;
    operation.responses = vec![OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(SchemaValue::reference("#/components/schemas/ContactPage")),
        }],
    }];
    operation.annotations.insert(
        "x-speakeasy-pagination".into(),
        serde_json::json!({
            "type": "url",
            "outputs": { "nextUrl": "$.next" }
        }),
    );
    let root = tempfile::tempdir().unwrap();
    let tree = render_test_sdk(&api, "sdk", Some("email")).unwrap();
    let client = all_source(&tree);
    assert!(client.contains("func (client *Client) newPaginationRequest"));
    assert!(client.contains("pagination URL must remain on the configured API origin"));
    assert!(client.contains("newPaginationRequest(ctx, \"GET\", pager.nextURL"));
    tree.write_to(root.path()).unwrap();
    fs::write(
            root.path().join("sdk/url_pager_test.go"),
            r#"package email

import (
    "context"
    "io"
    "net/http"
    "strings"
    "testing"
)

type urlPagerTransport struct { requests int; auth string; requestID string; query string }
func (transport *urlPagerTransport) RoundTrip(request *http.Request) (*http.Response, error) {
    transport.requests++
    if transport.requests == 2 { transport.auth, transport.requestID, transport.query = request.Header.Get("Authorization"), request.Header.Get("X-Request-ID"), request.URL.RawQuery }
    response := `{}`
    if transport.requests == 1 { response = `{"next":"/v1/contacts?cursor=cursor_2"}` }
    return &http.Response{StatusCode: http.StatusOK, Header: http.Header{}, Body: io.NopCloser(strings.NewReader(response)), Request: request}, nil
}

func TestPoolsterURLPagerPreservesAuthAndServerQuery(t *testing.T) {
    transport := &urlPagerTransport{}
    client, err := NewClient(ClientConfig{BaseURL: "https://example.test", APIKey: "secret", APIKeyPrefix: "Bearer", HTTPClient: &http.Client{Transport: transport}, Retry: &RetryConfig{MaxAttempts: 1}})
    if err != nil { t.Fatal(err) }
    requestID := "request_123"
    pager := client.ListContactsPages(&ListContactsRequest{XRequestID: &requestID})
    if _, err = pager.Next(context.Background()); err != nil { t.Fatal(err) }
    if _, err = pager.Next(context.Background()); err != nil { t.Fatal(err) }
    if transport.requests != 2 || transport.auth != "Bearer secret" || transport.requestID != "request_123" || transport.query != "cursor=cursor_2" { t.Fatalf("requests=%d auth=%q requestID=%q query=%q", transport.requests, transport.auth, transport.requestID, transport.query) }
}

type unsafeURLPagerTransport struct { requests int }
func (transport *unsafeURLPagerTransport) RoundTrip(request *http.Request) (*http.Response, error) {
    transport.requests++
    return &http.Response{StatusCode: http.StatusOK, Header: http.Header{}, Body: io.NopCloser(strings.NewReader(`{"next":"https://untrusted.test/contacts"}`)), Request: request}, nil
}

func TestPoolsterURLPagerRejectsCrossOriginContinuation(t *testing.T) {
    transport := &unsafeURLPagerTransport{}
    client, err := NewClient(ClientConfig{BaseURL: "https://example.test", HTTPClient: &http.Client{Transport: transport}, Retry: &RetryConfig{MaxAttempts: 1}})
    if err != nil { t.Fatal(err) }
    pager := client.ListContactsPages(&ListContactsRequest{})
    if _, err = pager.Next(context.Background()); err != nil { t.Fatal(err) }
    if _, err = pager.Next(context.Background()); err == nil || !strings.Contains(err.Error(), "configured API origin") { t.Fatalf("err = %v", err) }
    if transport.requests != 1 { t.Fatalf("requests = %d", transport.requests) }
}
"#,
        )
        .unwrap();
    let status = Command::new("go")
        .args(["test", "./..."])
        .current_dir(root.path().join("sdk"))
        .env("GOCACHE", root.path().join("go-cache"))
        .status()
        .unwrap();
    assert!(status.success());
}
