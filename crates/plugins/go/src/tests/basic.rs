use super::*;

#[test]
fn emits_an_isolated_typed_go_package() {
    let tree = render_test_sdk(&contact_api(), "sdks/go", Some("email")).unwrap();
    assert_eq!(
        tree.get("sdks/go/go.mod"),
        Some("module email\n\ngo 1.22\n")
    );
    let models = all_source(&tree);
    assert!(models.contains("package email"));
    assert!(models.contains("Email string `json:\"email\"`"));
    assert!(models.contains("DisplayName *string `json:\"display_name,omitempty\"`"));
    let client = all_source(&tree);
    assert!(client.contains("type ClientConfig struct"));
    assert!(client.contains("func NewClient(config ClientConfig) (*Client, error)"));
    assert!(client.contains("func (client *Client) GetContact(ctx context.Context, input *GetContactRequest) (*Contact, error)"));
    assert!(client.contains("url.PathEscape(fmt.Sprint(input.ContactID))"));
    assert!(client.contains("addQuery(query, \"expand\", input.Expand)"));
    assert!(client.contains("headers.Set(\"X-Request-ID\", fmt.Sprint(*input.XRequestID))"));
    assert!(client.contains("body := input.Body"));
}

#[test]
fn native_multipart_bytes_replay_stably_and_unsafe_uploads_do_not_retry() {
    let mut upload = Operation {
        id: "uploadFile".into(),
        method: HttpMethod::Put,
        path: "/upload".into(),
        request_body: Some(OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "multipart/form-data".into(),
                schema: None,
            }],
        }),
        responses: vec![OperationResponse {
            status: "204".into(),
            description: None,
            media_types: vec![],
        }],
        ..Default::default()
    };
    let mut create = upload.clone();
    create.id = "createFile".into();
    create.method = HttpMethod::Post;
    create.path = "/create".into();
    upload
        .request_body
        .as_mut()
        .unwrap()
        .media_types
        .push(OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(SchemaValue::new(SchemaKind::Object {
                fields: vec![],
                additional_properties: AdditionalProperties::Any,
            })),
        });
    let api = Api {
        name: "Upload".into(),
        operations: vec![upload, create],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api, "sdk", Some("upload"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(
        root.path().join("sdk/multipart_test.go"),
        include_str!("../../tests/fixtures/multipart_test.go"),
    )
    .unwrap();
    let output = Command::new("go")
        .args(["test", "-race", "./..."])
        .current_dir(root.path().join("sdk"))
        .env("GOCACHE", std::env::temp_dir().join("poolster-go-cache"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_scoped_call_headers_and_deadlines_preserve_client_defaults() {
    let api = Api {
        name: "Call".into(),
        operations: vec![Operation {
            id: "getThing".into(),
            method: HttpMethod::Get,
            path: "/thing".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api, "sdk", Some("call"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(
        root.path().join("sdk/call_options_test.go"),
        include_str!("../../tests/fixtures/call_options_test.go"),
    )
    .unwrap();
    let output = Command::new("go")
        .args(["test", "-race", "./..."])
        .current_dir(root.path().join("sdk"))
        .env("GOCACHE", std::env::temp_dir().join("poolster-go-cache"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_custom_method_preserves_wire_and_is_not_replayed() {
    let api = Api {
        name: "Custom".into(),
        operations: vec![
            Operation {
                id: "copyThing".into(),
                method: HttpMethod::Custom("COPY".into()),
                path: "/thing".into(),
                ..Default::default()
            },
            Operation {
                id: "quoteThing".into(),
                method: HttpMethod::Custom("X'CHECK`TEST".into()),
                path: "/quote".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api, "sdk", Some("custom"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(root.path().join("sdk/custom_method_test.go"), r#"package custom
import("context";"net/http";"net/http/httptest";"testing";"sync/atomic")
func TestCustomMethod(t *testing.T){
 var calls atomic.Int32
 server:=httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter,r *http.Request){calls.Add(1);expected:="COPY";if r.URL.Path=="/quote"{expected="X'CHECK`TEST"};if r.Method!=expected{t.Errorf("method %s",r.Method)};w.WriteHeader(503)}));defer server.Close()
 client,err:=NewClient(ClientConfig{BaseURL:server.URL,HTTPClient:server.Client(),Retry:&RetryConfig{MaxAttempts:3}});if err!=nil{t.Fatal(err)}
 if err:=client.CopyThing(context.Background());err==nil{t.Fatal("expected HTTP failure")};if calls.Load()!=1{t.Fatal("unsafe custom method replayed",calls.Load())}
 if err:=client.QuoteThing(context.Background());err==nil{t.Fatal("expected HTTP failure")};if calls.Load()!=2{t.Fatal("unsafe punctuation method replayed",calls.Load())}
}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "-race", "./..."])
        .current_dir(root.path().join("sdk"))
        .env("GOCACHE", std::env::temp_dir().join("poolster-go-cache"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn emits_safe_retry_runtime_without_changing_operation_signatures() {
    let tree = render_test_sdk(&contact_api(), "go", None).unwrap();
    let client = all_source(&tree);
    assert!(client.contains("Retry        *RetryConfig"));
    assert!(client.contains("retry: normalizeRetry(config.Retry)"));
    assert!(client.contains("Hooks        PoolsterClientHooks"));
    assert!(client.contains("type PoolsterClientHooks interface"));
    assert!(client.contains("client.beforeRequest(requestInfo)"));
    assert!(client.contains("client.afterResponse(requestInfo, response)"));
    assert!(client.contains("client.onError(requestInfo, result)"));
    assert!(client.contains("client.doWithRetry(request, &response)"));
    assert!(client.contains("func canRetry(request *http.Request) bool"));
    assert!(client.contains("case http.MethodGet, http.MethodHead, http.MethodOptions, http.MethodTrace, \"QUERY\", http.MethodPut, http.MethodDelete:"));
    assert!(client.contains("case http.MethodPost, http.MethodPatch:"));
    assert!(client.contains("strings.TrimSpace(request.Header.Get(\"Idempotency-Key\")) != \"\""));
    assert!(client.contains("http.StatusTooManyRequests"));
    assert!(client.contains("func replayRequest(request *http.Request)"));
    assert!(client.contains("func parseRetryAfter(value string)"));
}

#[test]
fn native_operation_inputs_do_not_shadow_models_or_reserved_input_names() {
    let mut api = contact_api();
    let original_name = operation_request_name(&api, &api.operations[0]);
    assert_eq!(original_name, "GetContactRequest");
    api.schemas
        .push(Schema::new("GetContactRequest", string_schema()));
    api.schemas
        .push(Schema::new("GetContactOperationRequest", string_schema()));
    assert_eq!(
        operation_request_name(&api, &api.operations[0]),
        "GetContactOperationRequest2"
    );
    api.schemas.push(Schema::new(
        "Reaction",
        SchemaValue::new(SchemaKind::Object {
            fields: ["+1", "-1", "v12"]
                .into_iter()
                .map(|name| Field {
                    name: name.into(),
                    value: SchemaValue::new(SchemaKind::Integer),
                    required: true,
                    annotations: BTreeMap::new(),
                })
                .collect(),
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    let root = tempfile::tempdir().unwrap();
    let tree = render_sdk(
        &api,
        "sdk",
        Some("collision"),
        SdkClientStyle::Namespaced,
        0,
    )
    .unwrap();
    tree.write_to(root.path()).unwrap();
    fs::write(root.path().join("sdk/collision_test.go"), r#"package collision
import("context";"testing";"encoding/json")
func TestDistinctInputAndModelTypes(t *testing.T){
  var wireModel GetContactRequest = "wire model"
  var otherModel GetContactOperationRequest = "another wire model"
  input := &GetContactOperationRequest2{ContactID:"contact"}
  if string(wireModel)==string(otherModel){t.Fatal("models conflated")}
  var direct func(context.Context,*GetContactOperationRequest2)(*Contact,error) = (*Client)(nil).GetContact
  var facade func(context.Context,*GetContactOperationRequest2)(*Contact,error) = (*ContactsService)(nil).Get
  _=input;_=direct;_=facade
  var reaction Reaction
  if err:=json.Unmarshal([]byte(`{"+1":2,"-1":3,"v12":4}`), &reaction); err!=nil {t.Fatal(err)}
  if reaction.V1!=2 || reaction.V13!=3 || reaction.V12!=4 {t.Fatal("wire fields conflated")}
  encoded,err:=json.Marshal(reaction);if err!=nil {t.Fatal(err)}
  var wire map[string]int64;if err=json.Unmarshal(encoded,&wire);err!=nil {t.Fatal(err)}
  if wire["+1"]!=2 || wire["-1"]!=3 || wire["v12"]!=4 {t.Fatal("wire fields changed")}
}
"#).unwrap();
    let result = Command::new("go")
        .args(["test", "./..."])
        .current_dir(root.path().join("sdk"))
        .env("GOCACHE", std::env::temp_dir().join("poolster-go-cache"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
