use super::*;
use poolster_core::{Field, HttpMethod, OperationParameter, OperationResponse};
use std::process::Command;

#[test]
fn native_large_packages_load_partitioned_models_and_operations() {
    let mut source = Api {
        name: "Split".into(),
        ..Default::default()
    };
    for index in 0..205 {
        source.schemas.push(Schema::new(
            format!("Record{index}"),
            SchemaValue::new(SchemaKind::Object {
                fields: (0..20)
                    .map(|field| poolster_core::Field {
                        name: format!("optional_wire_field_{field}"),
                        value: SchemaValue::new(SchemaKind::String),
                        required: false,
                        annotations: Default::default(),
                    })
                    .collect(),
                additional_properties: AdditionalProperties::Any,
            }),
        ));
        source.operations.push(Operation {
            id: format!("probe{index}"),
            method: poolster_core::HttpMethod::Get,
            path: format!("/resource{index}/probe"),
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security: vec![],
            annotations: Default::default(),
        });
    }
    let root = tempfile::tempdir().unwrap();
    let tree = render_sdk(
        &source,
        "sdk",
        Some("split-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    assert!(tree.get("sdk/lib/split_sdk/models/chunk_0002.rb").is_some());
    assert!(
        tree.get("sdk/lib/split_sdk/operations/chunk_0002.rb")
            .is_some()
    );
    assert!(
        tree.get("sdk/lib/split_sdk/response_shapes/refs_0001.rb")
            .is_some()
    );
    for (path, contents) in tree.iter() {
        if path.extension().is_some_and(|extension| extension == "rb") {
            assert!(
                contents.len() <= 128 * 1024,
                "{} exceeds chunk budget",
                path.display()
            );
        }
    }
    tree.write_to(root.path()).unwrap();
    let output = std::process::Command::new("ruby").args(["-Ilib", "-e", "require 'split_sdk'; raise unless SplitSdk::Models::Record204.from_hash({'extra'=>1}).to_h=={'extra'=>1}; raise unless SplitSdk::Client.instance_methods.include?(:probe204); raise unless SplitSdk::ResponseShapes['refs'].length==205; raise unless SplitSdk::Client.new(base_url: 'https://example.invalid').resource204.is_a?(SplitSdk::Resource204Resource)"]).current_dir(root.path().join("sdk")).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn native_normalized_names_preserve_distinct_wire_fields_and_arguments() {
    let mut source = Api {
        name: "Collision".into(),
        ..Default::default()
    };
    source.schemas.push(Schema::new(
        "Probe",
        SchemaValue::new(SchemaKind::Object {
            fields: ["+1", "-1", "field", "x-axis", "x_axis"]
                .iter()
                .map(|name| poolster_core::Field {
                    name: (*name).into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: false,
                    annotations: Default::default(),
                })
                .collect(),
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    source.schemas.push(Schema::new(
        "Nested/Model",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "name".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: true,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    source.schemas.push(Schema::new(
        "Holder",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "nested".into(),
                value: SchemaValue::reference("#/components/schemas/Nested~1Model"),
                required: true,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    source.operations.push(Operation {
        id: "probe".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/{id}".into(),
        parameters: ["path", "query", "header"]
            .iter()
            .map(|location| poolster_core::OperationParameter {
                name: "id".into(),
                location: (*location).into(),
                required: true,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: Default::default(),
            })
            .collect(),
        request_body: None,
        responses: vec![],
        security: vec![],
        annotations: Default::default(),
    });
    let root = tempfile::tempdir().unwrap();
    render_sdk(
        &source,
        "sdk",
        Some("collision-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    let script = r#"require 'collision_sdk'
wire={'+1'=>'positive','-1'=>'negative','field'=>'safe','x-axis'=>'dash','x_axis'=>'underscore'}
raise unless CollisionSdk::Models::Probe.from_hash(wire).to_h == wire
holder=CollisionSdk::Models::Holder.from_hash({'nested'=>{'name'=>'value'}})
raise unless holder.nested.is_a?(CollisionSdk::Models::NestedModel) && holder.to_h=={'nested'=>{'name'=>'value'}}
names=CollisionSdk::Client.instance_method(:probe).parameters.map{|kind, name| name}
raise unless names.include?(:id) && names.include?(:id_)
seen=[]
client=CollisionSdk::Client.new(base_url: 'https://example.invalid')
client.define_singleton_method(:request){|*args, **kwargs| seen << [args, kwargs]; nil}
client.probe(id: 'path value', id_: 'query value', id__: 'header value')
raise unless seen[0][0][1]=='/path%20value' && seen[0][1][:query]=={'id'=>'query value'} && seen[0][1][:headers]=={'id'=>'header value'}
"#;
    let output = std::process::Command::new("ruby")
        .args(["-Ilib", "-e", script])
        .current_dir(root.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
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
    let rendered = render_operation(&Api::default(), &operation);
    assert_eq!(rendered.matches("poolster_parameter_content(").count(), 4);
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
    assert!((render_operation(&Api::default(), &operation)).contains("poolster_whole_query("));
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
    let rendered = render_operation(&Api::default(), &operation);
    assert!(rendered.contains("with_encoding("));
    assert!(rendered.contains("prefix_encoding"));
}

#[test]
fn native_whole_query_and_sequential_json_preserve_wire() {
    let root = tempfile::tempdir().unwrap();
    let mut api = Api::default();
    api.schemas.push(Schema::new(
        "SequenceItem",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![
                poolster_core::Field {
                    name: "value".into(),
                    value: SchemaValue::new(SchemaKind::Integer),
                    required: true,
                    annotations: Default::default(),
                },
                poolster_core::Field {
                    name: "note".into(),
                    value: SchemaValue::new(SchemaKind::Any),
                    required: false,
                    annotations: Default::default(),
                },
            ],
            additional_properties: AdditionalProperties::Any,
        }),
    ));
    api.operations.push(Operation {
        id: "getItems".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/items".into(),
        parameters: vec![poolster_core::OperationParameter {
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
        }],
        responses: vec![poolster_core::OperationResponse {
            status: "200".into(),
            description: None,
            media_types: vec![poolster_core::OperationMediaType {
                content_type: "application/x-ndjson".into(),
                schema: Some(SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::reference("#/components/schemas/SequenceItem")),
                })),
            }],
        }],
        ..Default::default()
    });
    api.operations.push(Operation {
        id: "contentParameters".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/items/{id}".into(),
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
        responses: vec![poolster_core::OperationResponse::json(
            "200",
            SchemaValue::new(SchemaKind::Any),
        )],
        ..Default::default()
    });
    let mut strict = api.operations.last().unwrap().clone();
    strict.id = "strictParameters".into();
    for parameter in &mut strict.parameters {
        parameter.schema = Some(SchemaValue::new(SchemaKind::String));
    }
    api.operations.push(strict);
    render_sdk(&api, "sdk", Some("probe-sdk"), SdkClientStyle::Flat)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"require 'probe_sdk'
Response = Struct.new(:code, :body, :content_type)
body = "0\nfalse\nnull\n{\"future\":[0,false,null]}\n"
transport = ->(request) { raise 'query changed' unless request.uri.query == 'tag=a&tag=b&escaped=%26'; Response.new('200', body, 'application/x-ndjson') }
client = ProbeSdk::Client.new(base_url: 'https://example.test', transport: transport)
query = client.send(:poolster_whole_query, '?tag=a&tag=b&escaped=%26', 'text/plain')
result = client.send(:request, 'GET', '/', query: query, headers: {}, body: nil)
raise 'sequence changed' unless result == [0, false, nil, {'future'=>[0,false,nil]}]
body = "{\"value\":0,\"future\":false}\n"
items = client.get_items(filters: '?tag=a&tag=b&escaped=%26')
raise "typed sequence model lost: #{items.inspect}" unless items.first.is_a?(ProbeSdk::Models::SequenceItem) && items.first.to_h == {'value'=>0,'future'=>false}


manual = ProbeSdk::Models::SequenceItem.new(value: 0)
raise 'manual known changed' unless manual.to_h == {'value'=>0}
manual = manual.with_present_fields('note')
raise 'explicit null not serialized' unless manual.to_h == {'value'=>0, 'note'=>nil}
begin; manual.with_present_fields('missing'); raise 'unknown field accepted'; rescue ArgumentError; end
raise 'form changed' unless client.send(:poolster_whole_query, {'tag'=>['a','b'], 'zero'=>0, 'flag'=>false}, 'application/x-www-form-urlencoded') == 'tag=a&tag=b&zero=0&flag=false'
raise 'json changed' unless URI.decode_www_form_component(client.send(:poolster_whole_query, {'future'=>false}, 'application/json')) == '{"future":false}'
raise 'json seq changed' unless client.send(:poolster_sequential_json, "\x1e0\n\x1efalse\n", 'application/json-seq') == [0,false]
nested = ProbeSdk::MultipartBody.new.add_text('inner', 'nested')
plan = {'content_type'=>'multipart/mixed', 'prefix_encoding'=>[{'contentType'=>'application/json', 'headers'=>{'X-Position'=>{'required'=>true,'schema_definition'=>{'default'=>'first'}}}}, {'contentType'=>'multipart/mixed', 'prefixEncoding'=>[{'contentType'=>'text/custom'}]}], 'item_encoding'=>{'contentType'=>'application/custom'}}
ordered = ProbeSdk::MultipartBody.new.add_json('first', {'zero'=>0}).add_part('second', nested).add_file('third', "\x00\xff".b, filename: 'tail.bin').with_encoding(plan)
media, encoded = ordered.encode
raise 'top media type changed' unless media.start_with?('multipart/mixed; boundary=')
['Content-Type: application/json', 'X-Position: first', 'Content-Type: multipart/mixed', 'Content-Type: text/custom', 'Content-Type: application/custom', "\x00\xff".b].each { |wire| raise 'positional encoding changed' unless encoded.include?(wire) }
raise 'part order changed' unless encoded.index('name="first"') < encoded.index('name="second"')
expected_parameter_json = '{"flag":false}'
parameter_transport = ->(request) {
  raise 'JSON query content changed' unless URI.decode_www_form(request.uri.query).to_h['data'] == expected_parameter_json
  raise 'JSON path content changed' unless CGI.unescape(request.uri.path.split('/').last) == expected_parameter_json
  raise 'JSON header content changed' unless request['X-Data'] == expected_parameter_json
  raise 'JSON cookie content changed' unless CGI.unescape(request['Cookie'].split('=',2).last) == expected_parameter_json
  Response.new('200', '{}', 'application/json')
}
parameter_client = ProbeSdk::Client.new(base_url: 'https://example.test', transport: parameter_transport)
parameter_client.content_parameters(id: {'flag'=>false}, data: {'flag'=>false}, x_data: {'flag'=>false}, session: {'flag'=>false})
expected_parameter_json = 'null'
parameter_client.content_parameters(id: nil, data: nil, x_data: nil, session: nil)
begin
  parameter_client.strict_parameters(id: nil, data: nil, x_data: nil, session: nil)
  raise 'nonnullable JSON null accepted'
rescue ArgumentError
end

raise 'nullable JSON whole query lost' unless client.send(:poolster_whole_query, nil, 'application/json', true) == 'null'
raise 'optional JSON whole query changed' unless client.send(:poolster_whole_query, nil, 'application/json') == ''



['#bad', 'x=1?next', "x=1\nheader"].each do |raw|
  begin; client.send(:poolster_whole_query, raw, 'text/plain'); raise 'unsafe query accepted'; rescue ArgumentError; end
end
['prefix' + "\x1e{}", "\x1e", "\x1e{}\x1einvalid"].each do |raw|
  begin; client.send(:poolster_sequential_json, raw, 'application/json-seq'); raise 'invalid sequence accepted'; rescue JSON::ParserError; end
end
"#;
    let result = std::process::Command::new("ruby")
        .args(["-Ilib", "-e", script])
        .current_dir(root.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn native_multipart_binary_json_limits_and_retry_bytes() {
    let api = Api {
        name: "Example".into(),
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_sdk(&api, "ruby", Some("example-sdk"), SdkClientStyle::Flat)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = include_str!("../tests/fixtures/multipart_probe.rb.txt");
    let output = Command::new("ruby")
        .args(["-Ilib", "-e", script])
        .current_dir(root.path().join("ruby"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_per_call_headers_deadlines_cancellation_and_retries_are_isolated() {
    let api = Api {
        name: "Example".into(),
        operations: vec![poolster_core::Operation {
            id: "getProbe".into(),
            method: HttpMethod::Get,
            path: "/probe".into(),
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security: vec![],
            annotations: Default::default(),
        }],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_sdk(
        &api,
        "ruby",
        Some("example-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    std::fs::write(
        root.path().join("ruby/probe.rb"),
        include_str!("../tests/fixtures/request_options_probe.rb.txt"),
    )
    .unwrap();
    let output = Command::new("ruby")
        .args(["-Ilib", "probe.rb"])
        .current_dir(root.path().join("ruby"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_reserved_keyword_fields_preserve_wire_names() {
    let keys = [
        "next", "retry", "rescue", "yield", "super", "and", "or", "when", "break", "1st",
    ];
    let api = Api {
        name: "Keyword".into(),
        schemas: vec![Schema::new(
            "Probe",
            SchemaValue::new(SchemaKind::Object {
                fields: keys
                    .iter()
                    .map(|name| Field {
                        name: (*name).into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: true,
                        annotations: Default::default(),
                    })
                    .collect(),
                additional_properties: AdditionalProperties::Forbidden,
            }),
        )],
        ..Default::default()
    };
    let mut api = api;
    api.schemas.insert(
        0,
        Schema::new(
            "ForwardAlias",
            SchemaValue::reference("#/components/schemas/Probe"),
        ),
    );
    api.schemas.insert(
        0,
        Schema::new(
            "ForwardUnion",
            SchemaValue::new(SchemaKind::OneOf {
                variants: vec![
                    SchemaValue::reference("#/components/schemas/Probe"),
                    SchemaValue::new(SchemaKind::Boolean),
                ],
            }),
        ),
    );
    api.schemas.insert(
        0,
        Schema::new("Flag", SchemaValue::new(SchemaKind::Boolean)),
    );
    let root = tempfile::tempdir().unwrap();
    render_sdk(&api, "ruby", Some("keyword-sdk"), SdkClientStyle::Flat)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let output = Command::new("ruby")
            .args([
                "-Ilib",
                "-rkeyword_sdk",
                "-e",
                r#"
wire = %w[next retry rescue yield super and or when break 1st].to_h { |key| [key, "value:#{key}"] }
model = KeywordSdk::Models::Probe.from_hash(wire)
raise 'keyword model failed' unless model.next_ == 'value:next' && model.value_1st == 'value:1st'
raise 'wire names changed' unless model.to_h == wire
raise 'forward alias lost model decoding' unless KeywordSdk::Models::ForwardAlias.from_hash(wire).is_a?(KeywordSdk::Models::Probe)
raise 'forward alias constructor failed' unless KeywordSdk::Models::ForwardAlias.new(next_: 'works').next_ == 'works'
raise 'union wire value changed' unless KeywordSdk::Models::ForwardUnion.from_hash(wire) == wire && KeywordSdk::Models::ForwardUnion.from_hash(false) == false
raise 'union shape missing' unless KeywordSdk::Models::ForwardUnion.wire_shape[0] == 'union'
raise 'boolean alias failed' unless KeywordSdk::Models::Flag.from_hash(false) == false
"#,
            ])
            .current_dir(root.path().join("ruby"))
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn author_bundled_middleware_is_registered_without_customer_configuration() {
    use poolster_core::{customization::BundledMiddleware, engine::Packages};
    let api = Api {
        name: "Example".into(),
        ..Default::default()
    };
    let middleware = BundledMiddleware { path: "lib/example_sdk/customer.rb".into(), contents: "CustomerMiddleware = ->(request, following) { request['X-Bundled'] = 'yes'; following.call(request) }\n".into(), symbol: "CustomerMiddleware".into(), async_symbol: None };
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("example-sdk")
                .with(crate::sdk())
                .middleware(middleware.clone()),
        )
        .generate(&api, None)
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let script = r#"require 'example_sdk'
Response = Struct.new(:code, :body)
transport = ->(request) { raise 'middleware missing' unless request['X-Bundled'] == 'yes'; Response.new('200','{}') }
client = ExampleSdk::Client.new(base_url: 'https://example.test', transport: transport)
raise 'decode' unless client.send(:request, 'get', '/', query: {}, headers: {}, body: nil) == {}
"#;
    let output = Command::new("ruby")
        .args(["-Ilib", "-e", script])
        .current_dir(root.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut invalid = middleware;
    invalid.path = "lib/example_sdk/client.rb".into();
    assert!(
        Packages::new()
            .package(
                crate::package("sdk")
                    .name("example-sdk")
                    .with(crate::sdk())
                    .middleware(invalid)
            )
            .generate(&api, None)
            .is_err()
    );
}
#[test]
fn native_middleware_rewrites_recovers_and_short_circuits() {
    let api = Api {
        name: "Example".into(),
        ..Default::default()
    };
    let documentation = readme(&api, "example-sdk", "ExampleSdk", SdkClientStyle::Flat);
    assert!(documentation.contains("ExampleSdk::Client.new"));
    assert!(documentation.contains("middleware: [add_header]"));
    let root = tempfile::tempdir().unwrap();
    render_sdk(&api, "ruby", Some("example-sdk"), SdkClientStyle::Flat)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"require 'example_sdk'
Response = Struct.new(:code, :body)
seen = []
transport = ->(request) { raise 'header missing' unless request['X-Customer'] == 'acme'; raise IOError, 'offline' }
outer = ->(request, following) { seen << 'before'; request['X-Customer'] = 'acme'; response = following.call(request); response.body = '{"rewritten":true}'; seen << 'after'; response }
recover = ->(request, following) { begin; following.call(request); rescue IOError; Response.new('200', '{}'); end }
client = ExampleSdk::Client.new(base_url: 'https://example.test', transport: transport, middleware: [outer, recover])
result = client.send(:request, 'get', '/', query: {}, headers: {}, body: nil)
raise 'response/order' unless result == {'rewritten' => true} && seen == ['before', 'after']
short = ->(request, following) { Response.new('200', '{"cached":true}') }
client = ExampleSdk::Client.new(base_url: 'https://example.test', middleware: [short])
raise 'short circuit failed' unless client.send(:request, 'get', '/', query: {}, headers: {}, body: nil) == {'cached' => true}
client = ExampleSdk::Client.new(base_url: 'https://example.test', transport: transport, middleware: [outer])
begin
  client.send(:request, 'get', '/', query: {}, headers: {}, body: nil)
  raise 'transport error swallowed'
rescue IOError => error
  raise 'transport error changed' unless error.message == 'offline'
end
"#;
    let output = Command::new("ruby")
        .args(["-Ilib", "-e", script])
        .current_dir(root.path().join("ruby"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_roundtrip_consumer_preserves_nulls_and_unknown_fields() {
    use poolster_core::engine::Packages;
    let mut nullable = SchemaValue::new(SchemaKind::String);
    nullable.nullable = true;
    let api = Api {
        name: "Example".into(),
        schemas: vec![Schema::new(
            "Open",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![
                    Field {
                        name: "required".into(),
                        value: nullable.clone(),
                        required: true,
                        annotations: Default::default(),
                    },
                    Field {
                        name: "additional_properties".into(),
                        value: nullable,
                        required: false,
                        annotations: Default::default(),
                    },
                ],
                additional_properties: AdditionalProperties::Any,
            }),
        )],
        ..Default::default()
    };
    let sdk = crate::sdk();
    let tests = crate::roundtrips().models_from(&sdk);
    let tree = Packages::new()
        .package(
            crate::package("ruby")
                .name("example-sdk")
                .with(tests)
                .with(sdk),
        )
        .generate(&api, None)
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let output = Command::new("ruby")
        .args(["-Ilib", "test/model_roundtrips.rb"])
        .current_dir(root.path().join("ruby"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn emits_a_syntax_checked_namespaced_gem() {
    let api = Api {
        name: "Example API".into(),
        version: "1.2.3".into(),
        schemas: vec![Schema::new(
            "Contact",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![Field {
                    name: "id".into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: true,
                    annotations: Default::default(),
                }],
                additional_properties: AdditionalProperties::Any,
            }),
        )],
        operations: vec![Operation {
            id: "getContact".into(),
            method: HttpMethod::Get,
            path: "/contacts/{contactId}".into(),
            parameters: vec![OperationParameter {
                name: "contactId".into(),
                location: "path".into(),
                required: true,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: Default::default(),
            }],
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::reference("#/components/schemas/Contact"),
            )],
            ..Operation::default()
        }],
        ..Api::default()
    };
    let tree = render_sdk(
        &api,
        "sdk",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    assert!(
        tree.get("sdk/lib/example_api_sdk/client.rb")
            .unwrap()
            .contains("class ContactsResource")
    );
    assert!(
        tree.get("sdk/lib/example_api_sdk/client.rb")
            .unwrap()
            .contains("def get_contact(contact_id:, request_options: nil)")
    );
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    if Command::new("ruby").arg("--version").status().is_ok() {
        for path in [
            "sdk/lib/example_api_sdk.rb",
            "sdk/lib/example_api_sdk/client.rb",
            "sdk/lib/example_api_sdk/models.rb",
        ] {
            let status = Command::new("ruby")
                .args(["-c", path])
                .current_dir(root.path())
                .status()
                .unwrap();
            assert!(status.success(), "generated {path} must be valid Ruby");
        }
    }
}
#[test]
fn native_structural_checks_validate_cached_responses_before_model_conversion() {
    let field = |name: &str, value: SchemaValue, required: bool| Field {
        name: name.into(),
        value,
        required,
        annotations: Default::default(),
    };
    let mut secret = SchemaValue::new(SchemaKind::String);
    secret.write_only = true;
    let mut nullable = SchemaValue::new(SchemaKind::String);
    nullable.nullable = true;
    let mut open_enum = SchemaValue::new(SchemaKind::String);
    open_enum.enum_values = vec![serde_json::json!("known")];
    let object = SchemaValue::new(SchemaKind::Object {
        fields: vec![
            field("id", open_enum, true),
            field("count", SchemaValue::new(SchemaKind::Integer), true),
            field("secret", secret, true),
            field("label", nullable, false),
            field(
                "items",
                SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Boolean)),
                }),
                false,
            ),
        ],
        additional_properties: AdditionalProperties::Forbidden,
    });
    let mut api = Api {
        name: "Probe".into(),
        version: "1.0.0".into(),
        schemas: vec![Schema::new("Item", object)],
        operations: vec![Operation {
            id: "getItem".into(),
            method: HttpMethod::Get,
            path: "/item".into(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::reference("#/components/schemas/Item"),
            )],
            ..Default::default()
        }],
        ..Default::default()
    };
    let union = SchemaValue::new(SchemaKind::OneOf {
        variants: vec![
            SchemaValue::new(SchemaKind::String),
            SchemaValue::new(SchemaKind::Integer),
        ],
    });
    api.operations.push(Operation {
        id: "getVariant".into(),
        method: HttpMethod::Get,
        path: "/variant".into(),
        responses: vec![OperationResponse::json("2XX", union)],
        ..Default::default()
    });
    let branch = |name: &str| {
        SchemaValue::new(SchemaKind::Object {
            fields: vec![field(name, SchemaValue::new(SchemaKind::String), true)],
            additional_properties: AdditionalProperties::Any,
        })
    };
    api.operations.push(Operation {
        id: "getIntersection".into(),
        method: HttpMethod::Get,
        path: "/intersection".into(),
        responses: vec![OperationResponse::json(
            "200",
            SchemaValue::new(SchemaKind::AllOf {
                variants: vec![branch("a"), branch("b")],
            }),
        )],
        ..Default::default()
    });
    let root = tempfile::tempdir().unwrap();
    render_sdk(&api, "ruby", Some("probe-sdk"), SdkClientStyle::Flat)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"require 'probe_sdk'
class Response
  attr_reader :body, :code
  def initialize(body,code='200'); @body=body; @code=code; end
  def [](key); 'application/json; charset=utf-8'; end
end
calls=0
transport=->(request){calls+=1;raise 'must short circuit'}
wire={'id'=>'new-open-enum','count'=>2,'label'=>nil,'items'=>[true,false],'future'=>{'v'=>1}}
reply=Response.new(JSON.generate(wire))
cache=->(request,next_call){reply}
client=ProbeSdk::Client.new(base_url:'https://unused.example',transport:transport,middleware:[cache],validate_responses:true)
result=client.get_item
raise 'wire changed' unless result.to_h==wire
raise 'transport executed' unless calls==0
[{'id'=>'PAYLOAD_SECRET','count'=>true},{'id'=>'PAYLOAD_SECRET'},{'id'=>nil,'count'=>1},{'id'=>'x','count'=>1,'items'=>['PAYLOAD_SECRET']}].each do |invalid|
  reply=Response.new(JSON.generate(invalid))
  begin;client.get_item;raise 'accepted malformed shape';rescue ProbeSdk::ResponseDecodeError=>error
    raise 'payload leaked' if error.message.include?('PAYLOAD_SECRET')
    raise 'path missing' unless error.path.start_with?('$[')
  end
end
[7,'future-string'].each do |value|
  reply=Response.new(JSON.generate(value),'201');raise 'union valid rejected' unless client.get_variant==value
end
reply=Response.new('true','201')
begin;client.get_variant;raise 'union invalid accepted';rescue ProbeSdk::ResponseDecodeError;end
reply=Response.new('{"a":"x","b":"y"}')
raise 'intersection valid rejected' unless client.get_intersection=={'a'=>'x','b'=>'y'}
reply=Response.new('{"a":"x"}')
begin;client.get_intersection;raise 'intersection invalid accepted';rescue ProbeSdk::ResponseDecodeError;end
reply=Response.new('{not json PAYLOAD_SECRET')
begin;client.get_item;raise 'invalid JSON accepted';rescue ProbeSdk::ResponseDecodeError=>error;raise 'payload leaked' if error.message.include?('PAYLOAD_SECRET');end
reply=Response.new('"' + ('x'*(10*1024*1024)) + '"')
begin;client.get_item;raise 'oversized accepted';rescue ProbeSdk::ResponseDecodeError;end
reply=Response.new(('['*140)+'0'+(']'*140))
begin;client.get_item;raise 'deep accepted';rescue ProbeSdk::ResponseDecodeError;end
reply=Response.new('{"id":"x","count":true}')
permissive=ProbeSdk::Client.new(base_url:'https://unused.example',middleware:[cache])
raise 'default changed' unless permissive.get_item.count==true
reply=Response.new('{"server":"PAYLOAD_SECRET"}','400')
begin;client.get_item;raise 'error accepted';rescue ProbeSdk::ApiError=>error;raise 'wrong status' unless error.status==400;end
"#;
    let output = Command::new("ruby")
        .args(["-Ilib", "-e", script])
        .current_dir(root.path().join("ruby"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn native_auto_idempotency_generates_per_invocation_and_preserves_explicit_key() {
    let mut api = Api {
        name: "Probe".into(),
        ..Default::default()
    };
    let mut op = Operation {
        id: "createItem".into(),
        method: poolster_core::HttpMethod::Post,
        path: "/items".into(),
        ..Default::default()
    };
    op.parameters.push(poolster_core::OperationParameter {
        name: "X-Request-Key".into(),
        location: "header".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: Default::default(),
    });
    op.annotations.insert("x-poolster-idempotency-resolved".into(),serde_json::json!({"header":"X-Request-Key","parameter_name":"X-Request-Key","auto_generate":true}));
    api.operations.push(op);
    let mut collision = api.operations[0].parameters[0].clone();
    collision.name = "X_Request_Key".into();
    collision.location = "query".into();
    api.operations[0].parameters.insert(0, collision);
    let root = tempfile::tempdir().unwrap();
    render_sdk(&api, "ruby", Some("probe-sdk"), SdkClientStyle::Flat)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"require 'probe_sdk'
Response=Struct.new(:code,:body);seen=[]
client=ProbeSdk::Client.new(base_url:'https://example.invalid',transport:->(request){seen<<request['X-Request-Key'];Response.new('200','{}')})
client.create_item(x_request_key:'query-value');client.create_item;client.create_item(x_request_key_:'provided')
raise unless seen[0].match?(/\A[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\z/) && seen[0]!=seen[1] && seen[2]=='provided'
"#;
    let output = std::process::Command::new("ruby")
        .args(["-Ilib", "-e", script])
        .current_dir(root.path().join("ruby"))
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
fn native_malformed_json_is_explicitly_permissive_until_validation_enabled() {
    use poolster_core::{HttpMethod, OperationResponse};
    let api = Api {
        name: "Malformed".into(),
        operations: vec![Operation {
            id: "getPayload".into(),
            method: HttpMethod::Get,
            path: "/payload".into(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::String),
            )],
            ..Default::default()
        }],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_sdk(&api, "ruby", Some("probe-sdk"), SdkClientStyle::Flat)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"require 'probe_sdk'
class Response
 attr_reader :code,:body
 def initialize(code,body);@code=code;@body=body;end
 def [](key);'application/json';end
end
calls=0;reply=Response.new('200','{malformed SECRET')
transport=->(_){calls+=1;reply}
permissive=ProbeSdk::Client.new(base_url:'https://unused.example',transport:transport)
raise 'legacy decode behavior changed' unless permissive.get_payload=='{malformed SECRET' && calls==1
strict=ProbeSdk::Client.new(base_url:'https://unused.example',transport:transport,validate_responses:true)
begin;strict.get_payload;raise 'malformed JSON accepted';rescue ProbeSdk::ResponseDecodeError=>error;raise 'payload leaked' if error.message.include?('SECRET');end
raise 'strict decode retried' unless calls==2
reply=Response.new('503','{malformed SECRET')
begin;permissive.get_payload;raise 'status accepted';rescue ProbeSdk::ApiError=>error;raise unless error.status==503 && error.body=='{malformed SECRET';end
raise 'Ruby invented retry' unless calls==3
"#;
    let output = std::process::Command::new("ruby")
        .args(["-Ilib", "-e", script])
        .current_dir(root.path().join("ruby"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn native_nested_reference_arrays_decode_models_and_round_trip_wire() {
    let field = |name: &str, value: SchemaValue| poolster_core::Field {
        name: name.into(),
        value,
        required: true,
        annotations: Default::default(),
    };
    let api = Api {
        name: "Nested".into(),
        schemas: vec![
            Schema::new(
                "Contact",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![field("id", SchemaValue::new(SchemaKind::String))],
                    additional_properties: AdditionalProperties::Any,
                }),
            ),
            Schema::new(
                "ContactPage",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![field(
                        "items",
                        SchemaValue::new(SchemaKind::Array {
                            items: Box::new(SchemaValue::reference("#/components/schemas/Contact")),
                        }),
                    )],
                    additional_properties: AdditionalProperties::Forbidden,
                }),
            ),
        ],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_sdk(&api, "sdk", Some("probe-sdk"), SdkClientStyle::Flat)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"require 'probe_sdk'
wire={'items'=>[{'id'=>'one'},{'id'=>'two','future'=>nil}]}
page=ProbeSdk::Models::ContactPage.from_hash(wire)
raise unless page.items.all?{|item|item.is_a?(ProbeSdk::Models::Contact)} && page.items.map(&:id)==['one','two']
raise unless page.to_h==wire && JSON.parse(JSON.generate(page.to_h))==wire
"#;
    let output = std::process::Command::new("ruby")
        .args(["-Ilib", "-e", script])
        .current_dir(root.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
