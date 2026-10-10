use super::*;
use poolster_core::{Field, HttpMethod, OperationParameter};

fn api() -> Api {
    Api {
        name: "Example API".into(),
        version: "2026-09-19".into(),
        schemas: vec![Schema::new(
            "Contact",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![
                    Field {
                        name: "id".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: true,
                        annotations: Default::default(),
                    },
                    Field {
                        name: "display-name".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: false,
                        annotations: Default::default(),
                    },
                ],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        )],
        operations: vec![Operation {
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
                    annotations: Default::default(),
                },
                OperationParameter {
                    name: "includeDeleted".into(),
                    location: "query".into(),
                    required: false,
                    schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                    description: None,
                    annotations: Default::default(),
                },
            ],
            request_body: None,
            responses: vec![],
            security: vec![],
            annotations: Default::default(),
        }],
        annotations: Default::default(),
    }
}

#[test]
#[ignore = "requires elixir toolchain; dependency-free collision and alias wire probe"]
fn native_collision_models_preserve_wire_and_alias_decoding() {
    let schema = Schema::new(
        "Probe",
        SchemaValue::new(SchemaKind::Object {
            fields: ["+1", "-1", "x-axis", "x_axis"]
                .iter()
                .map(|name| poolster_core::Field {
                    name: (*name).into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: true,
                    annotations: Default::default(),
                })
                .collect(),
            additional_properties: AdditionalProperties::Forbidden,
        }),
    );
    let alias = Schema::new(
        "Alias",
        SchemaValue::reference("#/components/schemas/Probe"),
    );
    let mut script =
        "defmodule Collision.JSON do\n def to_wire(value), do: value\nend\n".to_owned();
    script.push_str(&render_model("Collision", &schema));
    script.push_str(&render_model("Collision", &alias));
    let page = Schema::new(
        "Page",
        SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::reference("#/components/schemas/Probe")),
        }),
    );
    script.push_str(&render_model("Collision", &page));
    script.push_str(
        r#"
wire=%{"+1"=>"positive","-1"=>"negative","x-axis"=>"dash","x_axis"=>"underscore"}
model=Collision.Models.Alias.from_map(wire)
if Collision.Models.Probe.to_map(model)!=wire, do: raise("collision alias wire roundtrip")
page=Collision.Models.Page.from_map([wire])
if Collision.Models.Probe.to_map(hd(page))!=wire, do: raise("array alias decoder")
if Collision.Models.Page.from_map(nil)!=nil, do: raise("nullable array alias")
"#,
    );
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("probe.exs"), script).unwrap();
    let result = std::process::Command::new("elixir")
        .arg(root.path().join("probe.exs"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
#[test]
fn required_json_content_nullability_controls_omission_guards() {
    let mut parameter = poolster_core::OperationParameter {
        name: "data".into(),
        location: "query".into(),
        required: true,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: [(
            "poolster.parameter_content".into(),
            serde_json::json!([{"content_type":"application/json"}]),
        )]
        .into(),
    };
    assert!(is_json_parameter_content(&parameter));
    assert!(!json_content_allows_null(&parameter));
    parameter.schema.as_mut().unwrap().nullable = true;
    assert!(json_content_allows_null(&parameter));
    parameter.schema = Some(SchemaValue::new(SchemaKind::Any));
    assert!(json_content_allows_null(&parameter));
}

#[test]
fn content_parameters_sequences_and_positional_metadata_reach_operations() {
    let mut operation = Operation {
        id: "probe".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/{id}".into(),
        parameters: [
            ("id", "path"),
            ("data", "query"),
            ("X-Data", "header"),
            ("session", "cookie"),
        ]
        .iter()
        .map(|(name, location)| poolster_core::OperationParameter {
            name: (*name).into(),
            location: (*location).into(),
            required: true,
            schema: Some(SchemaValue::new(SchemaKind::Any)),
            description: None,
            annotations: [(
                "poolster.parameter_content".into(),
                serde_json::json!([{"content_type":"application/json"}]),
            )]
            .into(),
        })
        .collect(),
        responses: vec![poolster_core::OperationResponse {
            status: "200".into(),
            description: None,
            media_types: vec![poolster_core::OperationMediaType {
                content_type: "application/x-ndjson".into(),
                schema: Some(SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Any)),
                })),
            }],
        }],
        ..Default::default()
    };
    let rendered = render_operation("Probe", &Api::default(), &operation);
    assert_eq!(rendered.matches("Client.parameter_content(").count(), 4);
    assert!(rendered.to_ascii_lowercase().contains("cookie"));
    operation.parameters = vec![poolster_core::OperationParameter {
        name: "filters".into(),
        location: "querystring".into(),
        required: true,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: [(
            "poolster.parameter_content".into(),
            serde_json::json!([{"content_type":"text/plain"}]),
        )]
        .into(),
    }];
    assert!(
        (render_operation("Probe", &Api::default(), &operation)).contains("Client.whole_query(")
    );
    operation.parameters.clear();
    operation.request_body = Some(poolster_core::OperationRequestBody {
        required: true,
        description: None,
        media_types: vec![poolster_core::OperationMediaType {
            content_type: "multipart/mixed".into(),
            schema: Some(SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::String)),
            })),
        }],
    });
    operation.annotations.insert("poolster.request_content".into(), serde_json::json!([{"content_type":"multipart/mixed","prefix_encoding":[{"contentType":"application/json"}],"item_encoding":{"contentType":"text/plain"}}]));
    let rendered = render_operation("Probe", &Api::default(), &operation);
    assert!(rendered.contains("MultipartBody.with_encoding("));
    assert!(rendered.contains("prefix_encoding"));
}

#[test]
#[ignore = "requires Elixir Mix project with Finch/Jason; POOLSTER_ELIXIR_NATIVE_PROJECT optional"]
fn native_multipart_binary_json_limits_and_retry_bytes() {
    let root = tempfile::tempdir().unwrap();
    let tree = render_sdk(
        &Api::default(),
        "sdk",
        Some("multipart-probe"),
        SdkClientStyle::Flat,
    )
    .unwrap();
    tree.write_to(root.path()).unwrap();
    let files = ["multipart_body", "api_error", "json", "client"];
    let mut script = String::new();
    for file in files {
        let path = root
            .path()
            .join(format!("sdk/lib/multipart_probe/{file}.ex"));
        use std::fmt::Write as _;
        writeln!(
            script,
            "Code.compile_file({})",
            serde_json::to_string(&path.to_string_lossy()).unwrap()
        )
        .unwrap();
    }
    script.push_str(include_str!("../../tests/fixtures/multipart_probe.exs"));
    let path = root.path().join("probe.exs");
    std::fs::write(&path, script).unwrap();
    let project = std::env::var_os("POOLSTER_ELIXIR_NATIVE_PROJECT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.path().join("sdk"));
    if std::env::var_os("POOLSTER_ELIXIR_NATIVE_PROJECT").is_none() {
        let deps = std::process::Command::new("mix")
            .arg("deps.get")
            .current_dir(&project)
            .output()
            .unwrap();
        assert!(
            deps.status.success(),
            "{}",
            String::from_utf8_lossy(&deps.stderr)
        );
    }
    let output = std::process::Command::new("mix")
        .args(["run", "--no-start"])
        .arg(path)
        .current_dir(project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Elixir; dependency-free model required-null probe"]
fn native_string_enum_typespecs_preserve_wire_strings() {
    let mut value = SchemaValue::new(SchemaKind::String);
    value.enum_values = vec![serde_json::json!("temperature_2m"), serde_json::json!("λ")];
    value.nullable = true;
    let array = SchemaValue::new(SchemaKind::Array {
        items: Box::new(value.clone()),
    });
    let source = format!(
        "defmodule EnumProbe do\n @type wire :: {}\n @type list_wire :: {}\n @spec echo(wire()) :: wire()\n def echo(value), do: value\nend\nfor value <- [\"temperature_2m\", \"λ\", \"future\", nil] do\n if EnumProbe.echo(value) != value, do: raise(\"wire value changed\")\nend\n",
        elixir_type(&value, "EnumProbe"),
        elixir_type(&array, "EnumProbe")
    );
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("probe.exs");
    std::fs::write(&path, source).unwrap();
    let output = std::process::Command::new("elixir")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn open_models_emit_collision_safe_presence_and_extension_storage() {
    let schema = forward_model_schema();
    let source = render_model("Probe", &schema);
    assert!(source.contains("poolster_present_fields_: Map.keys(map)"));
    assert!(source.contains("additional_properties_: Map.drop(map"));
    assert!(source.contains("Map.merge(Map.drop(JSON.to_wire(model.additional_properties_)"));
}

fn forward_model_schema() -> Schema {
    Schema::new(
        "FutureModel",
        SchemaValue::new(SchemaKind::Object {
            fields: ["note", "additional_properties", "poolster_present_fields"]
                .iter()
                .map(|name| Field {
                    name: (*name).into(),
                    value: SchemaValue::new(SchemaKind::Any),
                    required: false,
                    annotations: Default::default(),
                })
                .collect(),
            additional_properties: AdditionalProperties::Any,
        }),
    )
}

#[test]
#[ignore = "requires Elixir; dependency-free forward model roundtrip"]
fn native_forward_models_preserve_unknown_properties_and_presence() {
    let script = String::from("defmodule Probe.JSON do\n def to_wire(value), do: value\nend\n")
        + &render_model("Probe", &forward_model_schema())
        + r#"
model = struct(Probe.Models.FutureModel)
manual = Probe.Models.FutureModel.with_present_fields(model, ["note"])
if Probe.Models.FutureModel.to_map(manual) != %{"note" => nil}, do: raise("explicit null helper failed")
if Probe.Models.FutureModel.to_map(model) != %{}, do: raise("presence helper mutated original")
try do
  Probe.Models.FutureModel.with_present_fields(model, ["missing"])
  raise "unknown presence field accepted"
rescue ArgumentError -> :ok end
for wire <- [%{}, %{"note" => nil}, %{"future" => %{"enum" => "new", "union" => [false, 0, nil]}}, %{"additional_properties" => false, "poolster_present_fields" => 0}] do
  restored = Probe.Models.FutureModel.from_map(wire)
  if Probe.Models.FutureModel.to_map(restored) != wire, do: raise("wire presence or extension changed")
end
model = Probe.Models.FutureModel.from_map(%{"note" => "known"})
model = %{model | additional_properties_: %{"note" => "override", "future" => nil}}
if Probe.Models.FutureModel.to_map(model) != %{"note" => "known", "future" => nil}, do: raise("known fields must win")
"#;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("probe.exs");
    std::fs::write(&path, script).unwrap();
    let result = std::process::Command::new("elixir")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "requires Elixir; dependency-free model required-null probe"]
fn native_required_null_and_optional_omission() {
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
    let model = render_model("Probe", &schema);
    let script = String::from("defmodule Probe.JSON do\n def to_wire(value), do: value\nend\n")
        + &model
        + r#"
model=struct!(Probe.Models.WireInput,enabled: false,count: 0,note: nil)
expected=%{"enabled"=>false,"count"=>0,"note"=>nil}
if Probe.Models.WireInput.to_map(model)!=expected, do: raise("required-null serialization assertion")
restored=Probe.Models.WireInput.from_map(expected)
if Probe.Models.WireInput.to_map(restored)!=expected, do: raise("roundtrip presence assertion")
"#;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("probe.exs");
    std::fs::write(&path, script).unwrap();
    let result = std::process::Command::new("elixir")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

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
    let model = render_model("Probe", &schema);
    assert!(model.contains("is_nil(value) and key not in [\"enabled\", \"count\", \"note\"]"));
    assert!(model.contains("{\"missing\", model.missing}"));
}

#[test]
fn generates_a_deterministic_typed_mix_package() {
    let first = render_test_sdk(&api(), "sdk/elixir", Some("example-api-sdk")).unwrap();
    let second = render_test_sdk(&api(), "sdk/elixir", Some("example-api-sdk")).unwrap();
    assert_eq!(first, second);
    assert!(
        first
            .get("sdk/elixir/mix.exs")
            .unwrap()
            .contains("app: :example_api_sdk")
    );
    assert!(
        first
            .get("sdk/elixir/mix.exs")
            .unwrap()
            .contains("version: \"2026.9.19\"")
    );
    let model = first
        .get("sdk/elixir/lib/example_api_sdk/models/contact.ex")
        .unwrap();
    assert!(model.contains("defstruct [:id, :display_name, poolster_present_fields: nil]"));
    assert!(model.contains("{\"display-name\", model.display_name}"));
    let client = first
        .get("sdk/elixir/lib/example_api_sdk/client.ex")
        .unwrap();
    assert!(client.contains("Finch.request(request, client.finch"));
    assert!(client.contains("module.exception(status: status"));
    assert!(client.contains("max_retries: 2"));
    assert!(client.contains("retryable?(method, headers, idempotency_header)"));
    assert!(client.contains("transient_status?(status)"));
    assert!(client.contains("before_request"));
    let operations = first
        .get("sdk/elixir/lib/example_api_sdk/api/operations_0001.ex")
        .unwrap();
    assert!(operations.contains("def get_contact(client, options \\\\ [])"));
    assert!(operations.contains("Client.required(options, :contact_id)"));
    assert!(operations.contains("URI.encode(to_string(contact_id)"));
}

#[test]
fn rejects_empty_output_directory() {
    assert!(render_test_sdk(&api(), "/", None).is_err());
}

#[test]
fn namespaced_style_exports_resource_modules() {
    let mut tagged = api();
    tagged.operations[0]
        .annotations
        .insert("tags".into(), serde_json::json!(["People"]));
    let tree = render_sdk(
        &tagged,
        "sdk/elixir",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();

    let resources = tree
        .get("sdk/elixir/lib/example_api_sdk/resources/people/chunk_0001.ex")
        .unwrap();
    assert!(resources.contains("defmodule ExampleApiSdk.Resources.People"));
    assert!(resources.contains(
        "def get_contact(client, options \\\\ []), do: API.get_contact(client, options)"
    ));
    assert!(
        tree.get("sdk/elixir/STYLE_GUIDE.md")
            .unwrap()
            .contains("## Namespaced")
    );

    let flat = render_test_sdk(&api(), "sdk/elixir", None).unwrap();
    assert!(
        flat.get("sdk/elixir/lib/example_api_sdk/resources/contacts.ex")
            .is_none()
    );
}

#[test]
fn splits_large_api_and_resource_surfaces_into_bounded_modules() {
    let mut source = api();
    source.operations = (0..51)
        .map(|index| Operation {
            id: format!("listContacts{index}"),
            method: HttpMethod::Get,
            path: format!("/contacts/{index}"),
            ..Operation::default()
        })
        .collect();
    source.schemas = (0..2)
        .map(|index| {
            Schema::new(
                format!("Long-Model-{index}"),
                SchemaValue::new(SchemaKind::String),
            )
        })
        .collect();

    let tree = render_sdk(
        &source,
        "sdk/elixir",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    assert!(
        tree.get("sdk/elixir/lib/example_api_sdk/api/operations_0003.ex")
            .is_some()
    );
    assert!(
        tree.get("sdk/elixir/lib/example_api_sdk/resources/contacts/chunk_0003.ex")
            .is_some()
    );
    assert!(
        tree.get("sdk/elixir/lib/example_api_sdk/api.ex")
            .unwrap()
            .contains("Typed API operations")
    );
}

#[test]
fn emits_mix_runtime_retries_hooks_media_and_declared_errors() {
    use poolster_core::{OperationMediaType, OperationResponse};

    let mut source = api();
    source.operations[0].method = HttpMethod::Post;
    source.operations[0].parameters.push(OperationParameter {
        name: "Idempotency-Key".into(),
        location: "header".into(),
        required: true,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: Default::default(),
    });
    source.operations[0].responses = vec![
        OperationResponse {
            status: "200".into(),
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/octet-stream".into(),
                schema: None,
            }],
        },
        OperationResponse {
            status: "404".into(),
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(SchemaValue::reference("Contact")),
            }],
        },
    ];

    let tree = render_test_sdk(&source, "sdk/elixir", Some("example-api-sdk")).unwrap();
    let client = tree
        .get("sdk/elixir/lib/example_api_sdk/client.ex")
        .unwrap();
    let errors = tree
        .get("sdk/elixir/lib/example_api_sdk/errors/chunk_0001.ex")
        .unwrap();
    let operations = tree
        .get("sdk/elixir/lib/example_api_sdk/api/operations_0001.ex")
        .unwrap();

    assert!(client.contains("Only idempotent methods, or POST requests with Idempotency-Key"));
    assert!(client.contains("status in [408, 429, 500, 502, 503, 504]"));
    assert!(client.contains("headers |> header(\"retry-after\") |> parse_retry_after()"));
    assert!(client.contains("before_request: Map.get(hooks"));
    assert!(client.contains("encode_body(body, :binary) when is_binary(body)"));
    assert!(errors.contains("Errors.GetContactStatus404Error"));
    assert!(
        operations
            .contains(":json, :binary, %{404 => ExampleApiSdk.Errors.GetContactStatus404Error}")
    );
    assert!(operations.contains("ExampleApiSdk.Models.Contact.from_map(map)"));
}

#[test]
fn emits_explicit_cursor_pagers_and_lazy_sse_streams() {
    use poolster_core::OperationResponse;

    let mut source = api();
    let mut pages = source.operations[0].clone();
    pages.id = "listContacts".into();
    pages.path = "/contacts".into();
    pages
        .parameters
        .retain(|parameter| parameter.location != "path");
    pages.parameters.push(OperationParameter {
        name: "cursor".into(),
        location: "query".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: Default::default(),
    });
    pages.annotations.insert(
        "x-poolster-pagination".into(),
        serde_json::json!({
            "type": "cursor",
            "inputs": [{"name": "cursor", "in": "parameters", "type": "cursor"}],
            "outputs": {"nextCursor": "$.meta.nextCursor"}
        }),
    );

    let mut events = pages.clone();
    events.id = "watchContacts".into();
    events.path = "/contacts/events".into();
    events.parameters.clear();
    events.annotations.clear();
    events.responses = vec![OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![poolster_core::OperationMediaType {
            content_type: "text/event-stream".into(),
            schema: None,
        }],
    }];
    source.operations = vec![pages, events];

    let tree = render_test_sdk(&source, "sdk/elixir", Some("example-api-sdk")).unwrap();
    let client = tree
        .get("sdk/elixir/lib/example_api_sdk/client.ex")
        .unwrap();
    let operations = tree
        .get("sdk/elixir/lib/example_api_sdk/api/operations_0001.ex")
        .unwrap();
    let mix = tree.get("sdk/elixir/mix.exs").unwrap();

    let normalized = client.split_whitespace().collect::<String>();
    let contains =
        |source: &str| normalized.contains(&source.split_whitespace().collect::<String>());
    assert!(operations.contains("def list_contacts_pages(client, options \\\\ [])"));
    assert!(operations.contains("Client.json_path(response, \"$.meta.nextCursor\")"));
    assert!(operations.contains("Keyword.put(current, :cursor, cursor)"));
    assert!(operations.contains("def watch_contacts(client, options \\\\ [])"));
    assert!(operations.contains("Client.event_stream(client, :get, path"));
    assert!(contains("def event_stream(client, method, path"));
    assert!(contains("Task.start(fn ->"));
    assert!(contains(
        "start_sse_request(client, request, self(), make_ref())"
    ));
    assert!(contains("Finch.stream(request, client.finch"));
    assert!(!contains("{:cont, acc}"));
    assert!(contains(":stream -> \"text/event-stream\""));
    assert!(contains("defp decode_sse_data"));
    assert!(contains("defp parse_retry_after_http_date"));
    assert!(mix.contains("extra_applications: [:logger, :inets, :crypto]"));
}
#[test]
fn emits_composable_customer_transport() {
    let client = render_client("Probe");
    assert!(client.contains("transport: Keyword.get(options, :transport)"));
    assert!(client.contains("Enum.reduce(Enum.reverse(client.middleware), terminal"));
    assert!(client.contains("layer.(request, next)"));
    assert!(client.contains("case execute_transport(client, request) do"));
    assert!(client.contains(
        "client.stream_transport.(request, [receive_timeout: client.timeout], nil, callback)"
    ));
}

#[test]
#[ignore = "requires an Elixir toolchain; dependency-free transport probe"]
fn elixir_customer_middleware_executes() {
    let mut source = api();
    source.operations.truncate(1);
    let op = &mut source.operations[0];
    op.id = "createItem".into();
    op.path = "/items".into();
    op.method = poolster_core::HttpMethod::Patch;
    op.parameters = vec![OperationParameter {
        name: "X-Request-Key".into(),
        location: "header".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: Default::default(),
    }];
    op.responses = vec![poolster_core::OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![poolster_core::OperationMediaType {
            content_type: "text/plain".into(),
            schema: Some(SchemaValue::new(SchemaKind::String)),
        }],
    }];
    op.annotations.insert("x-poolster-idempotency-resolved".into(),serde_json::json!({"header":"X-Request-Key","parameter_name":"X-Request-Key","auto_generate":true}));
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("multipart_body.ex"),
        include_str!("../../templates/multipart.ex.tmpl").replace("__POOLSTER_MODULE__", "Probe"),
    )
    .unwrap();
    std::fs::write(
        root.path().join("client.ex"),
        render_client("Probe").replace(
            "Process.sleep(milliseconds)",
            "send(self(), {:delay, milliseconds})",
        ),
    )
    .unwrap();
    std::fs::write(
        root.path().join("operations.ex"),
        render_operation_chunk("Probe", &source, &source.operations, 0),
    )
    .unwrap();
    std::fs::write(root.path().join("probe.exs"), r#"
defmodule Finch.Request do
  defstruct [:method, :url, :body, headers: []]
  @type t :: %__MODULE__{}
end
defmodule Finch.Response do
  defstruct status: 200, headers: [], body: ""
end
defmodule Finch do
  def build(method, url, headers, body), do: struct(Finch.Request, method: method, url: url, headers: headers, body: body)
  def request(_, _, _), do: raise("default transport unexpectedly called")
  def stream(_, _, _, _, _), do: raise("default stream unexpectedly called")
end
defmodule Probe.ApiError do
  defexception [:status, :body, :headers]
end
defmodule Probe.JSON do
  def decode(value), do: {:ok, value}
  def to_wire(value), do: value
end
Code.compile_file("multipart_body.ex")
Code.compile_file("client.ex")
Code.compile_file("operations.ex")
ExUnit.start()
defmodule TransportProbe do
  use ExUnit.Case
  test "request mutation, error recovery, response transformation and ordering" do
terminal = fn request, options ->
  assert options[:receive_timeout] == 30_000
  assert {"x-customer", "yes"} in request.headers
  send(self(), :terminal)
  {:error, :expected}
end
inner = fn request, next ->
  send(self(), :inner_request)
  request = %{request | headers: [{"x-customer", "yes"} | request.headers]}
  assert {:error, :expected} = next.(request)
  send(self(), :inner_error)
  {:ok, struct(Finch.Response, body: "recovered")}
end
outer = fn request, next ->
  send(self(), :outer_request)
  {:ok, response} = next.(request)
  send(self(), :outer_response)
  {:ok, %{response | body: "transformed"}}
end
{:ok, client} = Probe.Client.new(base_url: "https://unused.example", transport: terminal, middleware: [outer, inner], max_retries: 0)
assert {:ok, "transformed"} = Probe.Client.request(client, :get, "/label", [], [], nil, :json, :text)
for event <- [:outer_request, :inner_request, :terminal, :inner_error, :outer_response], do: assert_receive(^event)
shortcut = fn _, _ -> {:ok, struct(Finch.Response, body: "cached")} end
{:ok, client} = Probe.Client.new(base_url: "https://unused.example", transport: terminal, middleware: [shortcut])
assert {:ok, "cached"} = Probe.Client.request(client, :get, "/label", [], [], nil, :json, :text)
refute_receive(:terminal)
  end
  test "transport errors reach customer hook" do
{:ok, client} = Probe.Client.new(base_url: "https://unused.example", transport: fn _, _ -> {:error, :expected} end, max_retries: 0, on_error: fn reason -> send(self(), {:observed, reason}) end)
assert {:error, :expected} = Probe.Client.request(client, :get, "/label")
assert_receive({:observed, :expected})
  end
  test "custom idempotency retry eligibility is operation scoped and UUID v4" do
key = Probe.Client.idempotency_key()
assert key =~ ~r/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/
refute key == Probe.Client.idempotency_key()
transport = fn request, _ -> send(self(), {:key, request.headers}); {:ok, struct(Finch.Response, status: 503)} end
{:ok, client} = Probe.Client.new(base_url: "https://unused.example", transport: transport, max_retries: 1, retry_initial_delay_ms: 0, retry_max_delay_ms: 0)
Probe.Client.request(client, :patch, "/items", [], [{"X-Request-Key", key}], nil, :json, :text, %{}, "X-Request-Key")
assert_receive({:key, first});assert_receive({:key, second});assert first == second
Probe.API.Operations0000.create_item(client)
assert_receive({:key, auto_first});assert_receive({:key, auto_second});assert auto_first == auto_second
auto_key = auto_first |> Enum.find_value(fn {name, value} -> if name == "X-Request-Key", do: value end)
assert auto_key =~ ~r/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/
Probe.API.Operations0000.create_item(client, x_request_key: "provided")
assert_receive({:key, provided_first});assert_receive({:key, provided_second});assert provided_first == provided_second
assert {"X-Request-Key", "provided"} in provided_first
Probe.API.Operations0000.create_item(client, x_request_key: "")
assert_receive({:key, empty});assert {"X-Request-Key", ""} in empty;refute_receive({:key, _})
Probe.API.Operations0000.create_item(client, x_request_key: "   ")
assert_receive({:key, blank});assert {"X-Request-Key", "   "} in blank;refute_receive({:key, _})
Probe.Client.request(client, :post, "/items", [], [], nil, :json, :text)
assert_receive({:key, _});refute_receive({:key, _})
Probe.Client.request(client, :post, "/items", [], [{"Idempotency-Key", "provided"}], nil, :json, :text)
assert_receive({:key, standard_first});assert_receive({:key, standard_second});assert standard_first == standard_second
Probe.Client.request(client, :post, "/items", [], [{"Idempotency-Key", " "}], nil, :json, :text)
assert_receive({:key, _});refute_receive({:key, _})

Probe.Client.request(client, :patch, "/items", [], [{"X-Request-Key", key}], nil, :json, :text)
assert_receive({:key, _});refute_receive({:key, _})
  end
  test "millisecond retry delay precedes seconds and respects cap" do
for {ms, expected} <- [{"250", 250}, {"99999", 400}, {"invalid", 400}, {"0", 0}] do
  transport = fn _, _ -> {:ok, struct(Finch.Response, status: 503, headers: [{"Retry-After", "2"}, {"retry-after-ms", ms}])} end
  {:ok, client} = Probe.Client.new(base_url: "https://unused.example", transport: transport, max_retries: 1, retry_max_delay_ms: 400)
  Probe.Client.request(client, :get, "/items", [], [], nil, :json, :text)
  if expected > 0, do: assert_receive({:delay, ^expected}), else: refute_receive({:delay, _})
end
  end
  test "stream transport can synthesize streaming responses" do
stream = fn _, _, acc, callback ->
  acc = callback.({:status, 200}, acc)
  acc = callback.({:data, "data: hello\n\n"}, acc)
  {:ok, acc}
end
{:ok, client} = Probe.Client.new(base_url: "https://unused.example", stream_transport: stream)
{:ok, events} = Probe.Client.event_stream(client, :get, "/events")
assert Enum.to_list(events) == [{:ok, "hello"}]
  end
end
"#).unwrap();
    let output = std::process::Command::new("elixir")
        .arg("probe.exs")
        .current_dir(root.path())
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
fn auto_idempotency_is_generated_before_operation_scoped_retries() {
    let mut api = api();
    let op = &mut api.operations[0];
    op.parameters.clear();
    op.path = "/contacts".into();
    op.method = poolster_core::HttpMethod::Post;
    op.parameters.push(OperationParameter {
        name: "X-Request-Key".into(),
        location: "header".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: Default::default(),
    });
    op.annotations.insert("x-poolster-idempotency-resolved".into(),serde_json::json!({"header":"X-Request-Key","parameter_name":"X-Request-Key","auto_generate":true}));
    let rendered = render_operation("Probe", &api, &api.operations[0]);
    assert!(rendered.contains("nil -> Client.idempotency_key(); provided -> provided"));
    assert!(rendered.contains(", \"X-Request-Key\") do"));
    let client = render_client("Probe");
    assert!(client.contains(":crypto.strong_rand_bytes(16)"));
    assert!(client.contains("context.idempotency_header"));
    assert!(!client.contains("[:get, :put, :patch, :delete]"));
}
fn byte_boundary_api() -> Api {
    let parameters = (0..80)
        .map(|index| poolster_core::OperationParameter {
            name: format!("queryParameter{index:03}{}", "LongName".repeat(20)),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        })
        .collect::<Vec<_>>();
    Api {
        name: "Byte Probe".into(),
        operations: (0..20)
            .map(|index| Operation {
                id: format!("getItem{index}"),
                path: format!("/items/{index}"),
                parameters: parameters.clone(),
                responses: vec![poolster_core::OperationResponse::json(
                    "200",
                    SchemaValue::new(SchemaKind::String),
                )],
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
#[ignore = "requires Elixir; native bounded delegation macro compilation"]
fn native_large_facades_use_bounded_delegation_macros() {
    let mut source = byte_boundary_api();
    source.operations = (0..600)
        .map(|index| Operation {
            id: format!("getItem{index}{}", "LongName".repeat(20)),
            path: format!("/items/{index}"),
            responses: vec![poolster_core::OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::String),
            )],
            ..Default::default()
        })
        .collect();
    let tree = render_sdk(
        &source,
        "elixir",
        Some("byte-probe"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    assert!(
        tree.get("elixir/lib/byte_probe/api.ex")
            .unwrap()
            .contains("use ByteProbe.API.Delegates")
    );
    assert!(
        tree.get("elixir/lib/byte_probe/resources/items.ex")
            .unwrap()
            .contains("use ByteProbe.Resources.Items.Delegates")
    );
    assert!(
        tree.iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "ex"))
            .all(|(_, value)| value.len() <= 128 * 1024)
    );
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let method = elixir_identifier(&source.operations.last().unwrap().id);
    let script = format!(
        r#"
defmodule ByteProbe.Client do
 @type t :: map()
 def request(client, _method, path, _query, _headers, _body, _body_kind, _response_kind, _errors, _idempotency), do: client.transport.(path)
end
defmodule ByteProbe.JSON do
 def to_wire(value), do: value
end
Path.wildcard("elixir/lib/byte_probe/api/*.ex") |> Enum.each(&Code.compile_file/1)
Code.compile_file("elixir/lib/byte_probe/api.ex")
Path.wildcard("elixir/lib/byte_probe/resources/items/*.ex") |> Enum.each(&Code.compile_file/1)
Code.compile_file("elixir/lib/byte_probe/resources/items.ex")
client=%{{transport: fn path -> {{:ok,path}} end}}
{{:ok,"/items/599"}}=ByteProbe.API.{method}(client)
{{:ok,"/items/599"}}=ByteProbe.Resources.Items.{method}(client)
"#
    );
    std::fs::write(root.path().join("probe.exs"), script).unwrap();
    let output = std::process::Command::new("elixir")
        .arg("probe.exs")
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Elixir; dependency-free compiled operation/resource forwarding probe"]
fn native_byte_bounded_operations_preserve_last_resource_and_transport() {
    let source = byte_boundary_api();
    let root = tempfile::tempdir().unwrap();
    let tree = render_sdk(
        &source,
        "elixir",
        Some("byte-probe"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let chunks = tree
        .iter()
        .filter(|(path, _)| path.to_string_lossy().contains("api/operations_"))
        .collect::<Vec<_>>();
    assert!(chunks.len() > 1);
    assert!(
        chunks
            .iter()
            .all(|(_, contents)| contents.len() <= 128 * 1024)
    );
    assert!(
        tree.iter()
            .filter(|(path, _)| path.to_string_lossy().contains("resources/"))
            .all(|(_, contents)| contents.len() <= 128 * 1024)
    );
    tree.write_to(root.path()).unwrap();
    let script = r#"
defmodule ByteProbe.Client do
 @type t :: map()
 def request(client, method, path, query, _headers, _body, _body_kind, _response_kind, _errors, _idempotency), do: client.transport.(method,path,query)
end
defmodule ByteProbe.JSON do
 def to_wire(value), do: value
end
Path.wildcard("elixir/lib/byte_probe/api/*.ex") |> Enum.each(&Code.compile_file/1)
Code.compile_file("elixir/lib/byte_probe/api.ex")
Path.wildcard("elixir/lib/byte_probe/resources/items/*.ex") |> Enum.each(&Code.compile_file/1)
Code.compile_file("elixir/lib/byte_probe/resources/items.ex")
client=%{transport: fn _method,path,_query -> send(self(),path); {:ok,"custom"} end}
{:ok,"custom"}=ByteProbe.API.get_item19(client)
{:ok,"custom"}=ByteProbe.Resources.Items.get_item19(client)
receive do "/items/19" -> :ok after 0 -> raise("transport not called") end
receive do "/items/19" -> :ok after 0 -> raise("resource not called") end
"#;
    std::fs::write(root.path().join("probe.exs"), script).unwrap();
    let output = std::process::Command::new("elixir")
        .arg("probe.exs")
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
