use super::*;

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
    let rendered = render_operation(&operation, "Probe", &NamedTypes::default());
    assert_eq!(rendered.matches("poolsterParameterContent(").count(), 4);
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
        (render_operation(&operation, "Probe", &NamedTypes::default()))
            .contains("poolsterWholeQuery(")
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
    let rendered = render_operation(&operation, "Probe", &NamedTypes::default());
    assert!(rendered.contains("withEncoding("));
    assert!(rendered.contains("prefix_encoding"));
}

#[test]
#[ignore = "requires PHP 8.2 or newer; native OpenAPI whole query and sequential JSON"]
fn native_whole_query_and_sequential_json_preserve_wire() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("MultipartBody.php"),
        include_str!("../../templates/multipart.php.tmpl").replace("__NAMESPACE__", "Probe"),
    )
    .unwrap();
    std::fs::write(
        root.path().join("Client.php"),
        render_client(&Api::default(), "Probe", SdkClientStyle::Flat),
    )
    .unwrap();
    let script = r#"<?php
require __DIR__.'/Client.php';
require __DIR__.'/MultipartBody.php';
$client=(new ReflectionClass(\Probe\Client::class))->newInstanceWithoutConstructor();
$query = new ReflectionMethod($client, 'poolsterWholeQuery');
$content = new ReflectionMethod($client, 'poolsterParameterContent');
if ($content->invoke($client, ['flag'=>false, 'zero'=>0, 'null'=>null], 'application/json') !== '{"flag":false,"zero":0,"null":null}') throw new Exception('parameter JSON changed');
if ($content->invoke($client, null, 'application/json') !== 'null') throw new Exception('required JSON null lost');
if ($query->invoke($client, null, 'application/json', true) !== 'null') throw new Exception('nullable JSON whole query lost');
if ($query->invoke($client, null, 'application/json') !== '') throw new Exception('optional whole query changed');

$sequence = new ReflectionMethod($client, 'poolsterSequentialJson');
if ($query->invoke($client, '?tag=a&tag=b&escaped=%26', 'text/plain') !== 'tag=a&tag=b&escaped=%26') throw new Exception('raw query changed');
if ($query->invoke($client, ['tag'=>['a','b'], 'flag'=>false, 'zero'=>0], 'application/x-www-form-urlencoded') !== 'tag=a&tag=b&flag=false&zero=0') throw new Exception('form changed');
if (rawurldecode($query->invoke($client, ['future'=>false], 'application/json')) !== '{"future":false}') throw new Exception('JSON query changed');
if ($sequence->invoke($client, "0\nfalse\nnull\n{\"future\":[0,false,null]}\n", 'application/x-ndjson') !== [0,false,null,['future'=>[0,false,null]]]) throw new Exception('NDJSON changed');
if ($sequence->invoke($client, "\x1e0\n\x1efalse\n", 'application/json-seq') !== [0,false]) throw new Exception('sequence changed');
$nested = (new \Probe\MultipartBody())->addText('inner', 'nested');
$plan = json_decode('{"content_type":"multipart/mixed","prefix_encoding":[{"contentType":"application/json","headers":{"X-Position":{"required":true,"schema_definition":{"default":"first"}}}},{"contentType":"multipart/mixed","prefixEncoding":[{"contentType":"text/custom"}]}],"item_encoding":{"contentType":"application/custom"}}', true, flags: JSON_THROW_ON_ERROR);
$ordered = (new \Probe\MultipartBody())->addJson('first',['zero'=>0])->addPart('second',$nested)->addFile('third',"\x00\xff",'tail.bin')->withEncoding($plan);
[$media,$encoded] = $ordered->encode();
if (!str_starts_with($media, 'multipart/mixed; boundary=')) throw new Exception('top media changed');
foreach (['Content-Type: application/json', 'X-Position: first', 'Content-Type: multipart/mixed', 'Content-Type: text/custom', 'Content-Type: application/custom', "\x00\xff"] as $wire) if (!str_contains($encoded,$wire)) throw new Exception('positional encoding changed');
if (strpos($encoded,'name="first"') >= strpos($encoded,'name="second"')) throw new Exception('part order changed');
foreach (['#bad', 'x=1?next', "x=1\nheader"] as $raw) { try { $query->invoke($client,$raw,'text/plain'); throw new Exception('unsafe accepted'); } catch (InvalidArgumentException $e) {} }
foreach (["prefix\x1e{}", "\x1e", "\x1e{}\x1einvalid"] as $raw) { try { $sequence->invoke($client,$raw,'application/json-seq'); throw new Exception('invalid sequence accepted'); } catch (UnexpectedValueException|JsonException $e) {} }
"#;
    let path = root.path().join("probe.php");
    std::fs::write(&path, script).unwrap();
    let output = std::process::Command::new("php")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires PHP 8.2+; executes generated multipart codec and request/retry methods"]
fn native_multipart_binary_json_limits_and_retry_bytes() {
    let root = tempfile::tempdir().unwrap();
    let client = render_client(&Api::default(), "MultipartProbe", SdkClientStyle::Flat);
    let start = client.find("    private function request(").unwrap();
    let end = client[start..]
        .find("    private function eventStream(")
        .unwrap()
        + start;
    let methods = &client[start..end];
    std::fs::write(
        root.path().join("MultipartBody.php"),
        include_str!("../../templates/multipart.php.tmpl")
            .replace("__NAMESPACE__", "MultipartProbe"),
    )
    .unwrap();
    std::fs::write(
        root.path().join("probe.php"),
        include_str!("../../tests/fixtures/multipart_probe.php.tmpl")
            .replace("__REQUEST_METHODS__", methods),
    )
    .unwrap();
    let result = std::process::Command::new("php")
        .arg(root.path().join("probe.php"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
