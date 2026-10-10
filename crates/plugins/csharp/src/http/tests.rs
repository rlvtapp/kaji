use std::collections::BTreeMap;

use poolster_core::{
    Field, HttpMethod, OperationMediaType, OperationParameter, OperationRequestBody, Schema,
};

use super::*;

#[test]
fn required_nullable_and_optional_omission_are_distinct() {
    let mut nullable = SchemaValue::new(SchemaKind::String);
    nullable.nullable = true;
    let schema = Schema::new(
        "WireInput",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![
                Field {
                    name: "enabled".into(),
                    value: SchemaValue::new(SchemaKind::Boolean),
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "count".into(),
                    value: SchemaValue::new(SchemaKind::Integer),
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "note".into(),
                    value: nullable,
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "missing".into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: false,
                    annotations: Default::default(),
                },
            ],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    );
    let model = render_model(&schema, "Example");
    assert!(model.contains(
        "[JsonPropertyName(\"note\")]\n    [JsonIgnore(Condition = JsonIgnoreCondition.Never)]"
    ));
    assert!(model.contains("[JsonPropertyName(\"missing\")]\n    public string? Missing"));
}

#[test]
fn normalized_page_and_offset_helpers_preserve_arguments_and_bounds() {
    let mut source = api();
    let operation = &mut source.operations[0];
    operation.parameters.push(OperationParameter {
        name: "page".into(),
        location: "query".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::Integer)),
        description: None,
        annotations: BTreeMap::new(),
    });
    operation.responses = vec![OperationResponse::json(
        "200",
        SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::new(SchemaKind::String)),
        }),
    )];
    operation.annotations.insert("x-poolster-pagination".into(), serde_json::json!({"type":"page", "inputs":[{"name":"page","type":"page"}], "outputs":{"results":"$"}}));
    let rendered = render_operation_chunk(&source, &source.operations[..1], "Example");
    assert!(rendered.contains("GetContactPagesAsync("));
    assert!(rendered.contains("long currentValue = page ?? 1"));
    assert!(
        rendered.contains(
            "GetContactAsync(contactId, includeDeleted, currentValue, cancellationToken)"
        )
    );
    assert!(rendered.contains("count == 0"));
    assert!(rendered.contains("long.MaxValue - 1"));
    assert!(rendered.contains("pageIndex < 10000"));
    assert!(rendered.contains("cancellationToken.ThrowIfCancellationRequested()"));
    source.operations[0]
        .parameters
        .last_mut()
        .unwrap()
        .schema
        .as_mut()
        .unwrap()
        .format = Some("int32".into());
    let rendered = render_operation_chunk(&source, &source.operations[..1], "Example");
    assert!(rendered.contains(
        "GetContactAsync(contactId, includeDeleted, checked((int)currentValue), cancellationToken)"
    ));
    assert!(rendered.contains("currentValue > int.MaxValue - 1"));
    assert!(rendered.contains("int? page = default"));
    let mut rejected = source.clone();
    rejected.operations[0]
        .parameters
        .last_mut()
        .unwrap()
        .location = "header".into();
    let error = render_test_sdk(&rejected, "csharp", None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("supports query inputs only"), "{error}");
    rejected = source.clone();
    rejected.operations[0]
        .parameters
        .last_mut()
        .unwrap()
        .required = true;
    rejected.operations[0]
        .parameters
        .last_mut()
        .unwrap()
        .schema
        .as_mut()
        .unwrap()
        .nullable = true;
    let error = render_test_sdk(&rejected, "csharp", None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("required and nullable/optional"), "{error}");
    rejected = source.clone();
    rejected.schemas.push(Schema::new(
        "PageNumber",
        SchemaValue::new(SchemaKind::Integer),
    ));
    rejected.operations[0].parameters.last_mut().unwrap().schema =
        Some(SchemaValue::reference("#/components/schemas/PageNumber"));
    let error = render_test_sdk(&rejected, "csharp", None)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("direct int32/int64 integer schema"),
        "{error}"
    );
    source.operations[0]
        .parameters
        .last_mut()
        .unwrap()
        .schema
        .as_mut()
        .unwrap()
        .format = None;
    source.operations[0].parameters.last_mut().unwrap().name = "offset".into();
    source.operations[0].parameters.push(OperationParameter {
        name: "limit".into(),
        location: "query".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::Integer)),
        description: None,
        annotations: BTreeMap::new(),
    });
    source.operations[0].annotations.insert("x-poolster-pagination".into(), serde_json::json!({"type":"offsetLimit", "inputs":[{"name":"offset","type":"offset"},{"name":"limit","type":"limit"}], "outputs":{"results":"$"}}));
    let rendered = render_operation_chunk(&source, &source.operations[..1], "Example");
    assert!(rendered.contains("long currentValue = offset ?? 0"));
    assert!(rendered.contains("long? pageLimit = limit"));
    assert!(rendered.contains("currentValue += count"));
    source.operations[0].responses = vec![OperationResponse::json(
        "200",
        SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::String)),
            })),
        }),
    )];
    source.operations[0]
        .annotations
        .get_mut("x-poolster-pagination")
        .unwrap()["outputs"]["results"] = serde_json::json!("/0");
    let rendered = render_operation_chunk(&source, &source.operations[..1], "Example");
    assert!(rendered.contains("results = results[0]"));
    source.operations[0]
        .annotations
        .get_mut("x-poolster-pagination")
        .unwrap()["outputs"]["results"] = serde_json::json!("$.missing");
    assert!(
        !render_operation_chunk(&source, &source.operations[..1], "Example")
            .contains("PagesAsync(")
    );
}

fn api() -> Api {
    Api {
        name: "Example API".into(),
        version: "2026-09-19".into(),
        schemas: vec![Schema::new(
            "Contact",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![
                    Field {
                        name: "email".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: true,
                        annotations: BTreeMap::new(),
                    },
                    Field {
                        name: "display-name".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: false,
                        annotations: BTreeMap::new(),
                    },
                ],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        )],
        operations: vec![
            Operation {
                id: "getContact".into(),
                method: HttpMethod::Get,
                path: "/contacts/{contactId}".into(),
                parameters: vec![
                    OperationParameter {
                        name: "contactId".into(),
                        location: "path".into(),
                        required: true,
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                        description: None,
                        annotations: BTreeMap::new(),
                    },
                    OperationParameter {
                        name: "includeDeleted".into(),
                        location: "query".into(),
                        required: false,
                        schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                        description: None,
                        annotations: BTreeMap::new(),
                    },
                ],
                request_body: None,
                responses: vec![poolster_core::OperationResponse::json(
                    "200",
                    SchemaValue::reference("#/components/schemas/Contact"),
                )],
                security: vec![],
                annotations: BTreeMap::new(),
            },
            Operation {
                id: "createContact".into(),
                method: HttpMethod::Post,
                path: "/contacts".into(),
                parameters: vec![OperationParameter {
                    name: "dryRun".into(),
                    location: "query".into(),
                    required: false,
                    schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                    description: None,
                    annotations: BTreeMap::new(),
                }],
                request_body: Some(OperationRequestBody {
                    required: true,
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "application/json".into(),
                        schema: Some(SchemaValue::reference("#/components/schemas/Contact")),
                    }],
                }),
                responses: vec![poolster_core::OperationResponse::json(
                    "200",
                    SchemaValue::reference("#/components/schemas/Contact"),
                )],
                security: vec![],
                annotations: BTreeMap::new(),
            },
        ],
        annotations: BTreeMap::new(),
    }
}

#[test]
fn idempotency_generation_uses_allocated_header_argument() {
    let mut operation = Operation {
        id: "createItem".into(),
        method: HttpMethod::Post,
        path: "/items".into(),
        ..Default::default()
    };
    operation.parameters.push(OperationParameter {
        name: "x_once".into(),
        location: "query".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: BTreeMap::new(),
    });
    operation.annotations.insert(
        "x-poolster-idempotency".into(),
        serde_json::json!({"header":"Query", "auto_generate":true}),
    );
    let api = Api {
        operations: vec![operation],
        ..api()
    };
    let api = poolster_core::idempotency::prepare_api(&api, &Default::default()).unwrap();
    let tree = render_test_sdk(&api, "sdk", Some("example-api-sdk")).unwrap();
    let operations = tree
        .get("sdk/Operations/PoolsterClientOperations000.cs")
        .unwrap();
    assert!(operations.contains(
        "if (query2 is null) request.Headers.TryAddWithoutValidation(\"Query\", Guid.NewGuid()"
    ));
    assert!(!operations.contains(
        "if (query is null) request.Headers.TryAddWithoutValidation(\"Query\", Guid.NewGuid()"
    ));
}

#[test]
fn model_names_cannot_shadow_generated_resource_types() {
    for name in ["LinksResource", "HttpStatusCode", "Encoding"] {
        let mut source = api();
        source.schemas[0].name = name.to_owned();
        let prepared = prepare_api(&source);
        assert_eq!(prepared.schemas[0].name, format!("{name}Model"));
        assert_eq!(source.schemas[0].name, name);
        let repeated = prepare_api(&prepared);
        assert_eq!(repeated.schemas[0].name, format!("{name}Model"));
    }
}

#[test]
fn generates_a_deterministic_csharp_package() {
    let first = render_test_sdk(&api(), "sdk/dotnet", Some("example-api-sdk")).unwrap();
    let second = render_test_sdk(&api(), "sdk/dotnet", Some("example-api-sdk")).unwrap();
    assert_eq!(first, second);
    assert!(
        first
            .get("sdk/dotnet/ExampleApiSdk.csproj")
            .unwrap()
            .contains("<TargetFramework>net8.0</TargetFramework>")
    );
    assert!(first.iter().any(|(path, source)| {
        path.to_string_lossy().contains("Models/Contact_")
            && source.contains("public sealed record Contact")
    }));
    let operations = first
        .get("sdk/dotnet/Operations/PoolsterClientOperations000.cs")
        .unwrap();
    assert!(operations.contains(
        "Task<Contact> GetContactAsync(string contactId, bool? includeDeleted = default"
    ));
    assert!(operations.contains("Uri.EscapeDataString(ParameterString(contactId))"));
    assert!(operations.contains("JsonContent.Create(body, options: JsonOptions)"));
    assert!(operations.contains("CreateContactAsync(Contact body, bool? dryRun = default"));
    // Operations without declared status-specific errors must close their
    // method directly. A conditional catch block is emitted only when an
    // error mapper exists; emitting its closing brace unconditionally
    // produces invalid C#.
    assert!(operations.contains(
        "return await SendWithRetryAsync<Contact>(request, cancellationToken).ConfigureAwait(false);\n    }\n"
    ));
    assert!(!operations.contains(
        "return await SendWithRetryAsync<Contact>(request, cancellationToken).ConfigureAwait(false);\n        }\n    }\n"
    ));
    let runtime = first.get("sdk/dotnet/PoolsterClient.cs").unwrap();
    assert!(runtime.contains("ApiKeyHeader"));
    assert!(runtime.contains("PoolsterRetryOptions"));
    assert!(runtime.contains("SendWithRetryAsync"));
    assert!(runtime.contains("Idempotency-Key"));
    assert!(runtime.contains("IPoolsterClientHooks"));
    assert!(runtime.contains("using var _request = request;"));
    assert!(!runtime.contains("using (request)\n        using var response"));
}

#[test]
fn rejects_empty_or_unsafe_output_directory() {
    assert!(render_test_sdk(&api(), "/", None).is_err());
    assert!(render_test_sdk(&api(), "sdk/../dotnet", None).is_err());
}

#[test]
fn namespaced_style_exports_typed_resource_facades() {
    let mut tagged = api();
    tagged.operations[0]
        .annotations
        .insert("tags".into(), serde_json::json!(["People"]));
    let tree = render_sdk(
        &tagged,
        "sdk/dotnet",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();

    let client = tree.get("sdk/dotnet/PoolsterClient.cs").unwrap();
    assert!(client.contains("public PeopleResource People { get; }"));
    assert!(client.contains("People = new PeopleResource(this);"));
    assert!(client.contains("public ContactsResource Contacts { get; }"));

    let resources = tree
        .get("sdk/dotnet/Resources/PeopleResource000.cs")
        .unwrap();
    assert!(resources.contains("public sealed partial class PeopleResource"));
    assert!(resources.contains("Task<Contact> GetContactAsync(string contactId"));
    assert!(
        resources.contains("_client.GetContactAsync(contactId, includeDeleted, cancellationToken)")
    );
    assert!(
        tree.get("sdk/dotnet/STYLE_GUIDE.md")
            .unwrap()
            .contains("## Namespaced")
    );

    let flat = render_test_sdk(&api(), "sdk/dotnet", None).unwrap();
    assert!(
        flat.get("sdk/dotnet/Resources/ContactsResource000.cs")
            .is_none()
    );
}

#[test]
fn emits_binary_upload_download_and_sse_surfaces() {
    let mut source = api();
    source.operations[0].id = "downloadContact".into();
    source.operations[0].request_body = Some(OperationRequestBody {
        required: true,
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/octet-stream".into(),
            schema: Some(SchemaValue::new(SchemaKind::String)),
        }],
    });
    source.operations[0].responses = vec![poolster_core::OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/pdf".into(),
            schema: None,
        }],
    }];
    let mut stream = source.operations[0].clone();
    stream.id = "watchContacts".into();
    stream.request_body = None;
    stream.responses[0].media_types[0].content_type = "text/event-stream".into();
    source.operations.push(stream);
    let tree = render_test_sdk(&source, "sdk/dotnet", None).unwrap();
    let client = tree
        .get("sdk/dotnet/Operations/PoolsterClientOperations000.cs")
        .unwrap();
    assert!(client.contains("Task<byte[]> DownloadContactAsync"));
    assert!(client.contains("ByteArrayContent(body)"));
    assert!(client.contains("SendBytesAsync"));
    assert!(client.contains("IAsyncEnumerable<string> WatchContactsAsync"));
    assert!(client.contains("StreamSseAsync"));
    source.operations[0]
        .request_body
        .as_mut()
        .unwrap()
        .media_types[0]
        .content_type = "multipart/form-data".into();
    let multipart = render_test_sdk(&source, "sdk/dotnet", None).unwrap();
    assert!(
        multipart
            .get("sdk/dotnet/DownloadContactMultipartBody.cs")
            .unwrap()
            .contains("IReadOnlyList<OrderedMultipartPart>")
    );
}

#[test]
fn emits_operation_status_errors_with_declared_bodies() {
    let mut source = api();
    source.operations[0]
        .responses
        .push(poolster_core::OperationResponse {
            status: "404".into(),
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(SchemaValue::reference("#/components/schemas/Contact")),
            }],
        });
    let tree = render_test_sdk(&source, "sdk/dotnet", None).unwrap();
    let errors = tree.get("sdk/dotnet/Errors/DeclaredErrors000.cs").unwrap();
    let client = tree
        .get("sdk/dotnet/Operations/PoolsterClientOperations000.cs")
        .unwrap();
    assert!(errors.contains("class GetContactStatus404Exception : ApiException"));
    assert!(errors.contains("Contact? Body"));
    assert!(client.contains("throw MapGetContactError(error)"));
    assert!(client.contains("TryDeserializeError<Contact>(error.ResponseBody)"));
}

#[test]
fn retry_headers_preserve_zero_precedence_and_bounds() {
    let client = render_client(&api(), "Example", SdkClientStyle::Flat);
    assert!(client.contains("DelayForRetryAsync(attempt, response.Headers, cancellationToken)"));
    assert!(client.contains("TryGetValues(\"retry-after-ms\""));
    assert!(client.contains("double.IsFinite(milliseconds) && milliseconds >= 0"));
    assert!(
        client.contains("RetryAfterMilliseconds(headers, DateTimeOffset.UtcNow)\n            ??")
    );
    assert!(client.contains("Math.Min(milliseconds, cap)"));
    assert!(client.contains("values.Any(value => !string.IsNullOrWhiteSpace(value))"));
    assert!(client.contains("HasNonEmptyHeader(request, header)"));
    assert!(client.contains("uint.MaxValue - 1d"));
    assert!(!client.contains("if (delay <= TimeSpan.Zero)"));
}

#[test]
#[ignore = "requires .NET 8; executes emitted SSE stream with a mock HTTP handler"]
fn generated_sse_framing_executes_with_dotnet() {
    let client = render_client(&api(), "Example", SdkClientStyle::Flat);
    let start = client
        .find("    private async IAsyncEnumerable<string> StreamSseAsync(")
        .unwrap();
    let end = client[start..].find("\n    }\n").unwrap() + start + "\n    }\n".len();
    let method = &client[start..end];
    let probe = format!(
        r#"
using System;
using System.IO;
using System.Text;
using System.Net;
using System.Net.Http;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
class ApiException : Exception {{ public ApiException(int status, string body) {{ }} }}
class Hook {{ public void OnError(Exception error) {{ }} public void AfterResponse(HttpResponseMessage response) {{ }} }}
class Handler : HttpMessageHandler {{
public int Calls;
public Content? Body;
protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken token) {{
    Calls++; Body = new Content(": comment\nevent: change\nid: 4\ndata: first\ndata:  second\n\nretry: 10\ndata:\n\ndata:last");
    return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) {{Content = Body}});
}}
}}
class Content : StringContent {{
public bool Closed;
public Content(string value) : base(value, Encoding.UTF8, "text/event-stream") {{ }}
protected override void Dispose(bool disposing) {{ Closed = true; base.Dispose(disposing); }}
}}
class Probe {{
private readonly HttpClient _httpClient;
private Hook? _hooks = null;
private TimeSpan? _callTimeout=null;
private Dictionary<string,string>? _callHeaders=null;
Probe(HttpClient client) {{ _httpClient = client; }}
{method}
static async Task Main() {{
    var handler = new Handler();
    using var http = new HttpClient(handler);
    var probe = new Probe(http);
    var sequence = probe.StreamSseAsync(new HttpRequestMessage(HttpMethod.Get,"https://example.invalid"), default);
    if (handler.Calls != 0) throw new Exception("not lazy");
    var values = new List<string>();
    await foreach(var value in sequence) values.Add(value);
    if (string.Join("|",values) != "first\n second||last") throw new Exception("bad SSE data framing");
    if (handler.Calls != 1 || handler.Body?.Closed != true) throw new Exception("not disposed");
    using var cancel = new CancellationTokenSource(); cancel.Cancel();
    try {{
        await foreach(var value in probe.StreamSseAsync(new HttpRequestMessage(HttpMethod.Get,"https://example.invalid"), cancel.Token)) {{ }}
        throw new Exception("cancellation ignored");
    }} catch(OperationCanceledException) {{ }}
}}
}}
"#
    );
    let output = tempfile::tempdir().unwrap();
    std::fs::write(output.path().join("Program.cs"), probe).unwrap();
    std::fs::write(output.path().join("Probe.csproj"), "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><OutputType>Exe</OutputType><Nullable>enable</Nullable></PropertyGroup></Project>").unwrap();
    let result = std::process::Command::new("dotnet")
        .args(["run", "--project", "Probe.csproj", "--verbosity", "quiet"])
        .current_dir(output.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "requires .NET 8; executes emitted retry parser without external dependencies"]
fn generated_retry_parser_executes_with_dotnet() {
    use std::process::Command;
    let client = render_client(&api(), "Example", SdkClientStyle::Flat);
    let start = client
        .find("    private static double? RetryAfterMilliseconds(")
        .unwrap();
    let end = client
        .find("    private async Task DelayForRetryAsync(")
        .unwrap();
    let eligibility_start = client
        .find("    private static bool IsRetryAllowed(")
        .unwrap();
    let parser = format!(
        "{}\n{}",
        &client[eligibility_start..start],
        &client[start..end]
    );
    let probe = format!(
        "using System;\nusing System.Globalization;\nusing System.Linq;\nusing System.Net;\nusing System.Net.Http;\nusing System.Net.Http.Headers;\nclass ParserProbe {{\n{parser}\n{}\n}}",
        r#"
static void Check(bool value) { if (!value) throw new Exception("parser assertion"); }
static void Main() {
    using var request = new HttpRequestMessage(HttpMethod.Post, "https://example.invalid");
    request.Headers.TryAddWithoutValidation("Idempotency-Key", "");
    Check(!IsRetryAllowed(request));
    request.Headers.Remove("Idempotency-Key");
    request.Headers.TryAddWithoutValidation("Idempotency-Key", "provided");
    Check(IsRetryAllowed(request));
    request.Headers.Remove("Idempotency-Key");
    request.Method = HttpMethod.Patch;
    request.Options.Set(new HttpRequestOptionsKey<string>("Poolster.IdempotencyHeader"), "X-Key");
    request.Headers.TryAddWithoutValidation("X-Key", "");
    Check(!IsRetryAllowed(request));
    request.Headers.Remove("X-Key");
    request.Headers.TryAddWithoutValidation("X-Key", " ");
    Check(!IsRetryAllowed(request));
    request.Headers.Remove("X-Key");
    request.Headers.TryAddWithoutValidation("X-Key", "provided");
    Check(IsRetryAllowed(request));
    var now = DateTimeOffset.UtcNow;
    using var response = new HttpResponseMessage();
    response.Headers.RetryAfter = new RetryConditionHeaderValue(TimeSpan.FromSeconds(2));
    response.Headers.TryAddWithoutValidation("retry-after-ms", "0");
    Check(RetryAfterMilliseconds(response.Headers, now) == 0);
    foreach (var value in new[] {"-1", "NaN", "Infinity", "invalid"}) {
        response.Headers.Remove("retry-after-ms");
        response.Headers.TryAddWithoutValidation("retry-after-ms", value);
        Check(RetryAfterMilliseconds(response.Headers, now) == 2000);
    }
    response.Headers.Remove("retry-after-ms");
    response.Headers.TryAddWithoutValidation("retry-after-ms", "125.5");
    Check(RetryAfterMilliseconds(response.Headers, now) == 125.5);
    response.Headers.Remove("retry-after-ms");
    response.Headers.RetryAfter = new RetryConditionHeaderValue(now.AddSeconds(5));
    var future = RetryAfterMilliseconds(response.Headers, now);
    Check(future > 4000 && future <= 5000);
    response.Headers.RetryAfter = new RetryConditionHeaderValue(now.AddSeconds(-5));
    Check(RetryAfterMilliseconds(response.Headers, now) == 0);
    response.Headers.Remove("Retry-After");
    Check(RetryAfterMilliseconds(response.Headers, now) is null);
}
"#
    );
    let output = tempfile::tempdir().unwrap();
    std::fs::write(output.path().join("Program.cs"), probe).unwrap();
    std::fs::write(output.path().join("Probe.csproj"), "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><OutputType>Exe</OutputType><Nullable>enable</Nullable></PropertyGroup></Project>").unwrap();
    let result = Command::new("dotnet")
        .args(["run", "--project", "Probe.csproj", "--verbosity", "quiet"])
        .current_dir(output.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "requires the .NET 8 SDK; CI runs this generated-package smoke test"]
fn generated_csharp_project_builds_with_dotnet() {
    use std::process::Command;

    let mut source = api();
    let mut page = source.operations[0].clone();
    page.id = "listItemPages".into();
    page.path = "/items".into();
    let mut integer = SchemaValue::new(SchemaKind::Integer);
    integer.format = Some("int32".into());
    page.parameters = vec![OperationParameter {
        name: "page".into(),
        location: "query".into(),
        required: false,
        schema: Some(integer),
        description: None,
        annotations: BTreeMap::new(),
    }];
    page.responses = vec![OperationResponse::json(
        "200",
        SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::new(SchemaKind::String)),
        }),
    )];
    page.annotations.insert("x-poolster-pagination".into(),serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page"}],"outputs":{"results":"$"}}));
    source.operations.push(page);
    let mut keyed = source.operations[0].clone();
    keyed.id = "createKeyedItem".into();
    keyed.method = HttpMethod::Post;
    keyed.annotations.insert(
        "x-poolster-idempotency".into(),
        serde_json::json!({"header":"X-Request-Key","auto_generate":true}),
    );
    source.operations.push(keyed);
    let source = poolster_core::idempotency::prepare_api(&source, &Default::default()).unwrap();
    let output = tempfile::tempdir().unwrap();
    let tree = render_sdk(
        &source,
        "csharp",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    tree.write_to(output.path()).unwrap();
    let project = output.path().join("csharp/ExampleApiSdk.csproj");
    let result = Command::new("dotnet")
        .args(["build", "--nologo", "--verbosity", "minimal"])
        .arg(&project)
        .output()
        .expect("the .NET SDK must be available when this test is selected");
    assert!(
        result.status.success(),
        "generated C# project failed to build:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
    );
    generated_retry_parser_executes_with_dotnet();
}
