use super::*;
use poolster_core::engine::Packages;
fn api() -> Api {
    let mut enumeration = SchemaValue::new(SchemaKind::String);
    enumeration.enum_values = vec![serde_json::json!("known-value")];
    let mut api = Api {
        name: "Compatibility".into(),
        version: "1.0.0".into(),
        schemas: vec![
            Schema::new("Count", SchemaValue::new(SchemaKind::Integer)),
            Schema::new(
                "Choice",
                SchemaValue::new(SchemaKind::OneOf {
                    variants: vec![
                        SchemaValue::new(SchemaKind::String),
                        SchemaValue::new(SchemaKind::Integer),
                    ],
                }),
            ),
            Schema::new("State", enumeration),
        ],
        operations: vec![Operation {
            id: "sequence".into(),
            method: poolster_core::HttpMethod::Post,
            path: "/sequence".into(),
            parameters: vec![poolster_core::OperationParameter {
                name: "whole_query".into(),
                location: "querystring".into(),
                required: true,
                schema: Some(SchemaValue::new(SchemaKind::Object {
                    fields: vec![],
                    additional_properties: AdditionalProperties::Any,
                })),
                description: None,
                annotations: Default::default(),
            }],
            request_body: Some(poolster_core::OperationRequestBody {
                required: true,
                media_types: vec![poolster_core::OperationMediaType {
                    content_type: "application/x-ndjson".into(),
                    schema: Some(SchemaValue::new(SchemaKind::Array {
                        items: Box::new(SchemaValue::new(SchemaKind::Any)),
                    })),
                }],
                description: None,
            }),
            responses: vec![poolster_core::OperationResponse {
                status: "200".into(),
                media_types: vec![poolster_core::OperationMediaType {
                    content_type: "application/x-ndjson".into(),
                    schema: Some(SchemaValue::new(SchemaKind::Array {
                        items: Box::new(SchemaValue::new(SchemaKind::Any)),
                    })),
                }],
                description: None,
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut ordered = Operation {
        id: "ordered".into(),
        method: poolster_core::HttpMethod::Post,
        path: "/ordered".into(),
        request_body: Some(poolster_core::OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![poolster_core::OperationMediaType {
                content_type: "multipart/mixed".into(),
                schema: Some(SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Any)),
                })),
            }],
        }),
        ..Default::default()
    };
    ordered.annotations.insert("poolster.request_content".into(),serde_json::json!([{"content_type":"multipart/mixed","prefix_encoding":[{"contentType":"application/json","headers":{"X-Part":{"required":true,"example_json":"\"v1\""}}},{"contentType":"multipart/mixed","prefixEncoding":[{"contentType":"text/plain","headers":{"X-Child":{"required":true,"example_json":"\"child\""}}}]}],"item_encoding":{"contentType":"application/octet-stream"}}]));
    api.operations.push(ordered);
    let mut params = Operation {
        id: "jsonParameters".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/params/{path}".into(),
        ..Default::default()
    };
    for (name, location) in [
        ("path", "path"),
        ("filter", "query"),
        ("x-json", "header"),
        ("cookie", "cookie"),
    ] {
        let mut parameter = poolster_core::OperationParameter {
            name: name.into(),
            location: location.into(),
            required: true,
            schema: Some({
                let mut value = SchemaValue::new(SchemaKind::String);
                value.nullable = true;
                value
            }),
            description: None,
            annotations: Default::default(),
        };
        parameter.annotations.insert("poolster.parameter_content".into(),serde_json::json!([{"content_type":"application/json","schema_definition":{"type":"string"}}]));
        params.parameters.push(parameter);
    }
    api.operations.push(params);
    api
}
#[test]
fn transparent_aliases_and_opt_in_enum_values_keep_wire_representation() {
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("Poolster.Compat")
                .with(crate::sdk().open_enums(true)),
        )
        .generate(&api(), None)
        .unwrap();
    assert!(
        tree.get(std::path::Path::new("sdk/Models").join(bounded_filename("Count", 0, "cs")))
            .unwrap()
            .contains("JsonSerializer.Deserialize<long>(ref reader")
    );
    assert!(
        tree.get(std::path::Path::new("sdk/Models").join(bounded_filename("Choice", 1, "cs")))
            .unwrap()
            .contains("JsonSerializer.Deserialize<JsonElement>(ref reader")
    );
    assert!(
        tree.get(std::path::Path::new("sdk/Models").join(bounded_filename("State", 2, "cs")))
            .unwrap()
            .contains("public sealed record State(string Value)")
    );
    assert!(
        tree.get(std::path::Path::new("sdk/Models").join(bounded_filename("State", 2, "cs")))
            .unwrap()
            .contains("new(\"known-value\")")
    );
}
#[test]
#[ignore = "requires .NET8; executes transparent aliases, union JSON and unknown string enums"]
fn native_models_preserve_alias_union_and_unknown_enum_wire_values() {
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("Poolster.Compat")
                .with(crate::sdk().open_enums(true))
                .with(crate::operation_tests()),
        )
        .generate(&api(), None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/tests/OperationTests/Program.cs"), r#"using System.Text.Json;
using Poolster.PoolsterCompat;
class Probe {
 static void Main() {
  var normalize=typeof(PoolsterClient).GetMethod("NormalizeSequentialJson",System.Reflection.BindingFlags.Static|System.Reflection.BindingFlags.NonPublic)!;
  var encode=typeof(PoolsterClient).GetMethod("EncodeSequentialJson",System.Reflection.BindingFlags.Static|System.Reflection.BindingFlags.NonPublic)!.MakeGenericMethod(typeof(JsonElement));
  var validate=typeof(PoolsterClient).GetMethod("ValidateWholeQuery",System.Reflection.BindingFlags.Static|System.Reflection.BindingFlags.NonPublic)!;
  var records=JsonSerializer.Deserialize<JsonElement>("[false,0,null,{\"future\":\"雪\"}]");
  foreach(var media in new[]{"application/x-ndjson","application/json-seq"}) {
   using var content=(HttpContent)encode.Invoke(null,new object[]{records,media})!;
   var serialized=content.ReadAsStringAsync().GetAwaiter().GetResult();
   var roundtrip=(string)normalize.Invoke(null,new object[]{serialized,media})!;
   if(JsonSerializer.Serialize(JsonSerializer.Deserialize<JsonElement>(roundtrip))!=JsonSerializer.Serialize(records))throw new Exception("sequence roundtrip");
  }
  foreach(var invalid in new[]{"missing separator","\u001e\u001e0","\u001e0 trailing"}) {try{normalize.Invoke(null,new object[]{invalid,"application/json-seq"});throw new Exception("accepted invalid sequence");}catch(System.Reflection.TargetInvocationException){}}
  using var driver=new SequenceDriver();using var http=new HttpClient(driver);var client=new PoolsterClient(http,new PoolsterClientOptions{BaseUrl="https://example.test"});
  var ordered=new OrderedMultipartBody{Parts=new OrderedMultipartPart[]{OrderedMultipartPart.Json("metadata",new{future="雪"}),new(){Name="nested",ContentType="multipart/mixed",Nested=new OrderedMultipartPart[]{new(){Name="child",Bytes=System.Text.Encoding.UTF8.GetBytes("false"),ContentType="text/plain"}}},new(){Name="file",Bytes=new byte[]{0,255},ContentType="application/octet-stream"}}};
  client.JsonParametersAsync("hello world","query","header","cookie").GetAwaiter().GetResult();
  client.JsonParametersAsync(null,null,null,null).GetAwaiter().GetResult();
  client.OrderedAsync(ordered).GetAwaiter().GetResult();
  var result=client.SequenceAsync("zero=0&false=false&name=%E9%9B%AA",records.EnumerateArray().Select(item=>item.Clone()).ToList()).GetAwaiter().GetResult();
  if(result.Count!=4||result[0].ValueKind!=JsonValueKind.False||result[2].ValueKind!=JsonValueKind.Null)throw new Exception("sequence wire response");
  validate.Invoke(null,new object[]{"zero=0&false=false&name=%E9%9B%AA"});
  foreach(var invalid in new[]{"?a=1","a=#fragment","a=%ZZ","a=%","a=\n"}) {try{validate.Invoke(null,new object[]{invalid});throw new Exception("accepted invalid query");}catch(System.Reflection.TargetInvocationException){}}
  var count=JsonSerializer.Deserialize<Count>("42")!;
  if(count.Value!=42 || JsonSerializer.Serialize(count)!="42")throw new Exception("scalar alias");
  foreach(var wire in new[]{"42","true","\"future\"","{\"unknown\":[1,null]}"}) {
   var choice=JsonSerializer.Deserialize<Choice>(wire)!;
   if(JsonSerializer.Serialize(choice)!=wire)throw new Exception("union wire");
  }
  var state=JsonSerializer.Deserialize<State>("\"future\"")!;
  if(state.Value!="future" || JsonSerializer.Serialize(state)!="\"future\"")throw new Exception("unknown enum");
  if(JsonSerializer.Serialize(State.KnownValue)!="\"known-value\"")throw new Exception("known wire name");
 }
}
class SequenceDriver:HttpMessageHandler {
 protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request,CancellationToken token) {
  if(request.RequestUri!.AbsolutePath=="/params/null"){if(request.RequestUri.Query!="?filter=null"||request.Headers.GetValues("x-json").Single()!="null"||request.Headers.GetValues("Cookie").Single()!="cookie=null")throw new Exception("required null content wire");return new HttpResponseMessage(System.Net.HttpStatusCode.NoContent);}
  if(request.RequestUri!.AbsolutePath.StartsWith("/params/")){if(request.RequestUri.AbsolutePath!="/params/%22hello%20world%22"||request.RequestUri.Query!="?filter=%22query%22"||request.Headers.GetValues("x-json").Single()!="\"header\""||request.Headers.GetValues("Cookie").Single()!="cookie=%22cookie%22")throw new Exception("JSON parameter wire");return new HttpResponseMessage(System.Net.HttpStatusCode.NoContent);}
  if(request.RequestUri!.AbsolutePath=="/ordered"){var bytes=await request.Content!.ReadAsByteArrayAsync(token);var text=System.Text.Encoding.UTF8.GetString(bytes);if(request.Content.Headers.ContentType!.MediaType!="multipart/mixed"||!text.Contains("X-Part: v1")||!text.Contains("X-Child: child")||!text.Contains("Content-Disposition: attachment"))throw new Exception("ordered MIME wire");return new HttpResponseMessage(System.Net.HttpStatusCode.NoContent);}
  if(request.RequestUri!.Query!="?zero=0&false=false&name=%E9%9B%AA")throw new Exception("query wire");
  if(request.Content!.Headers.ContentType!.MediaType!="application/x-ndjson")throw new Exception("request media");
  var wire=await request.Content.ReadAsStringAsync(token);
  if(!wire.Contains("false\n0\nnull\n"))throw new Exception("sequence request wire");
  return new HttpResponseMessage(System.Net.HttpStatusCode.OK){Content=new StringContent(wire,System.Text.Encoding.UTF8,"application/x-ndjson")};
 }
}
"#).unwrap();
    let result = std::process::Command::new("dotnet")
        .args([
            "run",
            "--project",
            "tests/OperationTests/OperationTests.csproj",
        ])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
