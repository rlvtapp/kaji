use super::*;

#[test]
fn generated_retry_client_compiles_with_go() {
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&contact_api(), "sdk", Some("email"))
        .unwrap()
        .write_to(root.path())
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
fn generated_client_retries_safe_requests_and_protects_post_without_a_key() {
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&contact_api(), "sdk", Some("email"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(
            root.path().join("sdk/retry_test.go"),
            r#"package email

import (
    "context"
    "io"
    "net/http"
    "strings"
    "testing"
    "time"
)

type retryTransport struct { attempts int }

type lifecycleHooks struct { before, after, errors int }
func (hooks *lifecycleHooks) BeforeRequest(PoolsterRequestInfo) { hooks.before++ }
func (hooks *lifecycleHooks) AfterResponse(PoolsterResponseInfo) { hooks.after++ }
func (hooks *lifecycleHooks) OnError(PoolsterRequestInfo, error) { hooks.errors++ }

func (transport *retryTransport) RoundTrip(request *http.Request) (*http.Response, error) {
    transport.attempts++
    status := http.StatusTooManyRequests
    if transport.attempts == 2 { status = http.StatusOK }
    return &http.Response{
        StatusCode: status,
        Header: http.Header{},
        Body: io.NopCloser(strings.NewReader(`{"email":"hello@example.com"}`)),
        Request: request,
    }, nil
}

func TestPoolsterRetriesSafeRequest(t *testing.T) {
    transport := &retryTransport{}

    hooks := &lifecycleHooks{}
    client, err := NewClient(ClientConfig{
        BaseURL: "https://example.test",
        HTTPClient: &http.Client{Transport: transport},
        Retry: &RetryConfig{MaxAttempts: 2, InitialDelay: time.Nanosecond},
        Hooks: hooks,
    })
    if err != nil { t.Fatal(err) }
    _, err = client.GetContact(context.Background(), &GetContactRequest{ContactID: "contact_123"})
    if err != nil { t.Fatal(err) }
    if transport.attempts != 2 { t.Fatalf("attempts = %d, want 2", transport.attempts) }
    if hooks.before != 1 || hooks.after != 1 || hooks.errors != 0 { t.Fatalf("hooks = before %d after %d errors %d", hooks.before, hooks.after, hooks.errors) }
}

func TestPoolsterDoesNotRetryPostWithoutIdempotencyKey(t *testing.T) {
    request, err := http.NewRequest(http.MethodPost, "https://example.test", nil)
    if err != nil { t.Fatal(err) }
    if canRetry(request) { t.Fatal("unsafe POST is retryable") }
    request.Header.Set("Idempotency-Key", "safe_123")
    if !canRetry(request) { t.Fatal("idempotent POST is not retryable") }
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
fn generated_client_handles_declared_errors_binary_text_and_sse() {
    let mut api = contact_api();
    api.schemas.push(Schema::new(
        "Problem",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "message".into(),
                value: string_schema(),
                required: true,
                annotations: BTreeMap::new(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    api.operations[0].responses.push(OperationResponse {
        status: "404".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(SchemaValue::reference("#/components/schemas/Problem")),
        }],
    });
    let mut binary = SchemaValue::new(SchemaKind::String);
    binary.format = Some("binary".into());
    api.operations.extend([
        Operation {
            id: "downloadExport".into(),
            method: HttpMethod::Get,
            path: "/v1/export".into(),
            parameters: Vec::new(),
            request_body: None,
            responses: vec![OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "application/octet-stream".into(),
                    schema: Some(binary),
                }],
            }],
            security: Vec::new(),
            annotations: BTreeMap::new(),
        },
        Operation {
            id: "watchEvents".into(),
            method: HttpMethod::Get,
            path: "/v1/events".into(),
            parameters: Vec::new(),
            request_body: None,
            responses: vec![OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "text/event-stream".into(),
                    schema: None,
                }],
            }],
            security: Vec::new(),
            annotations: BTreeMap::new(),
        },
        Operation {
            id: "healthText".into(),
            method: HttpMethod::Get,
            path: "/health".into(),
            parameters: Vec::new(),
            request_body: None,
            responses: vec![OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "text/plain".into(),
                    schema: Some(string_schema()),
                }],
            }],
            security: Vec::new(),
            annotations: BTreeMap::new(),
        },
    ]);
    let tree = render_test_sdk(&api, "sdk", Some("email")).unwrap();
    let client = all_source(&tree);
    assert!(client.contains("type GetContactError404 struct"));
    assert!(client.contains("func decodeGetContactError(requestError error) error"));
    assert!(
        client
            .contains("func (client *Client) DownloadExport(ctx context.Context) ([]byte, error)")
    );
    assert!(
        client.contains(
            "func (client *Client) WatchEvents(ctx context.Context) (io.ReadCloser, error)"
        )
    );
    assert!(
        client.contains("func (client *Client) HealthText(ctx context.Context) (string, error)")
    );
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    fs::write(
            root.path().join("sdk/media_test.go"),
            r#"package email

import (
    "context"
    "errors"
    "io"
    "net/http"
    "strings"
    "testing"
)

type mediaTransport struct{}

func (mediaTransport) RoundTrip(request *http.Request) (*http.Response, error) {
    status, body, contentType := http.StatusOK, "", "application/json"
    switch request.URL.Path {
    case "/v1/contacts/contact_123":
        status, body = http.StatusNotFound, `{"message":"missing"}`
    case "/v1/export":
        body, contentType = "zip bytes", "application/octet-stream"
    case "/v1/events":
        body, contentType = "data: hello\n\n", "text/event-stream"
    case "/health":
        body, contentType = "ok", "text/plain"
    }
    return &http.Response{StatusCode: status, Header: http.Header{"Content-Type": []string{contentType}}, Body: io.NopCloser(strings.NewReader(body)), Request: request}, nil
}

func TestPoolsterResponseMediaAndDeclaredErrors(t *testing.T) {
    client, err := NewClient(ClientConfig{BaseURL: "https://example.test", HTTPClient: &http.Client{Transport: mediaTransport{}}, Retry: &RetryConfig{MaxAttempts: 1}})
    if err != nil { t.Fatal(err) }
    _, err = client.GetContact(context.Background(), &GetContactRequest{ContactID: "contact_123"})
    var declared *GetContactError404
    if !errors.As(err, &declared) { t.Fatalf("error = %T, want *GetContactError404", err) }
    if declared.Body.Message != "missing" { t.Fatalf("message = %q", declared.Body.Message) }
    binary, err := client.DownloadExport(context.Background())
    if err != nil || string(binary) != "zip bytes" { t.Fatalf("binary = %q, err = %v", binary, err) }
    text, err := client.HealthText(context.Background())
    if err != nil || text != "ok" { t.Fatalf("text = %q, err = %v", text, err) }
    stream, err := client.WatchEvents(context.Background())
    if err != nil { t.Fatal(err) }
    defer stream.Close()
    data, err := io.ReadAll(stream)
    if err != nil || string(data) != "data: hello\n\n" { t.Fatalf("stream = %q, err = %v", data, err) }
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
