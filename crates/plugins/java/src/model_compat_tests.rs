use super::*;
use kaji_core::engine::Packages;
fn api() -> Api {
    let mut enumeration = SchemaValue::new(SchemaKind::String);
    enumeration.enum_values = vec![serde_json::json!("known-value")];
    let mut api = Api {
        name: "Compatibility".into(),
        version: "1.0.0".into(),
        schemas: vec![
            Schema::new("Count", {
                let mut value = SchemaValue::new(SchemaKind::Integer);
                value.enum_values = vec![serde_json::json!(1), serde_json::json!(2)];
                value
            }),
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
            method: kaji_core::HttpMethod::Post,
            path: "/sequence".into(),
            parameters: vec![kaji_core::OperationParameter {
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
            request_body: Some(kaji_core::OperationRequestBody {
                required: true,
                media_types: vec![kaji_core::OperationMediaType {
                    content_type: "application/x-ndjson".into(),
                    schema: Some(SchemaValue::new(SchemaKind::Array {
                        items: Box::new(SchemaValue::new(SchemaKind::Any)),
                    })),
                }],
                description: None,
            }),
            responses: vec![kaji_core::OperationResponse {
                status: "200".into(),
                media_types: vec![kaji_core::OperationMediaType {
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
        method: kaji_core::HttpMethod::Post,
        path: "/ordered".into(),
        request_body: Some(kaji_core::OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![kaji_core::OperationMediaType {
                content_type: "multipart/mixed".into(),
                schema: Some(SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Any)),
                })),
            }],
        }),
        ..Default::default()
    };
    ordered.annotations.insert("kaji.request_content".into(),serde_json::json!([{"content_type":"multipart/mixed","prefix_encoding":[{"contentType":"application/json","headers":{"X-Part":{"required":true,"example_json":"\"v1\""}}},{"contentType":"multipart/mixed","prefixEncoding":[{"contentType":"text/plain","headers":{"X-Child":{"required":true,"example_json":"\"child\""}}}]}],"item_encoding":{"contentType":"application/octet-stream"}}]));
    api.operations.push(ordered);
    api.operations.push(Operation {
        id: "collisionWire".into(),
        method: kaji_core::HttpMethod::Get,
        path: "/collision".into(),
        parameters: [
            ("notify", "query"),
            ("filter-a", "query"),
            ("filter_a", "query"),
            ("notify", "header"),
        ]
        .into_iter()
        .map(|(name, location)| kaji_core::OperationParameter {
            name: name.into(),
            location: location.into(),
            required: true,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        })
        .collect(),
        ..Default::default()
    });

    api.schemas.push(Schema::new(
        "Large",
        SchemaValue::new(SchemaKind::Object {
            fields: (0..260)
                .map(|index| kaji_core::Field {
                    name: format!("field{index}"),
                    required: index == 0,
                    value: {
                        let mut value = SchemaValue::new(SchemaKind::String);
                        value.nullable = index == 0;
                        value
                    },
                    annotations: Default::default(),
                })
                .collect(),
            additional_properties: AdditionalProperties::Any,
        }),
    ));

    let mut params = Operation {
        id: "jsonParameters".into(),
        method: kaji_core::HttpMethod::Get,
        path: "/params/{path}".into(),
        ..Default::default()
    };
    for (name, location) in [
        ("path", "path"),
        ("filter", "query"),
        ("x-json", "header"),
        ("cookie", "cookie"),
    ] {
        let mut parameter = kaji_core::OperationParameter {
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
        parameter.annotations.insert("kaji.parameter_content".into(),serde_json::json!([{"content_type":"application/json","schema_definition":{"type":"string"}}]));
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
                .name("io.kaji.compat")
                .with(crate::sdk().open_enums(true)),
        )
        .generate(&api(), None)
        .unwrap();
    assert!(
        tree.get("sdk/src/main/java/io/kaji/compat/model/Count.java")
            .unwrap()
            .contains("JsonCreator.Mode.DELEGATING")
    );
    assert!(
        tree.get("sdk/src/main/java/io/kaji/compat/model/Choice.java")
            .unwrap()
            .contains("JsonNode value")
    );
    assert!(
        tree.get("sdk/src/main/java/io/kaji/compat/model/State.java")
            .unwrap()
            .contains("Extensible wire value")
    );
}
#[test]
#[ignore = "requires Maven+JDK17; executes transparent aliases, union JSON and unknown string enums"]
fn native_models_preserve_alias_union_and_unknown_enum_wire_values() {
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("io.kaji.compat")
                .with(crate::sdk().open_enums(true))
                .with(crate::operation_tests()),
        )
        .generate(&api(), None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/src/test/java/io/kaji/compat/ModelsProbe.java"), r#"package io.kaji.compat;
import io.kaji.compat.model.*;
import com.fasterxml.jackson.databind.*;
public final class ModelsProbe extends ClientBase {
 private ModelsProbe(){super(new ClientConfig("https://example.test",null));}
 public static void main(String[] args)throws Exception {
  var mapper=new ObjectMapper();
  var large=mapper.readValue("{\"field0\":null,\"field259\":\"last\",\"future\":{\"nested\":false}}",Large.class);
  var roundtrip=mapper.readTree(mapper.writeValueAsString(large));
  if(!roundtrip.has("field0") || !roundtrip.get("field0").isNull() || roundtrip.has("field1") || !roundtrip.get("field259").asText().equals("last") || !roundtrip.path("future").path("nested").isBoolean())throw new AssertionError("large typed model roundtrip");

  var runtime=new ModelsProbe();
  var records=mapper.readTree("[false,0,null,{\"future\":\"雪\"}]");
  for(var media:new String[]{"application/x-ndjson","application/json-seq"}) {
   var encoded=runtime.encodeSequentialJson(records,media);
   if(!mapper.readTree(runtime.normalizeSequentialJson(encoded,media)).equals(records))throw new AssertionError("sequential roundtrip");
  }
  for(var invalid:new String[]{"missing separator","\u001e\u001e0","\u001e0 trailing"}) { try{runtime.normalizeSequentialJson(invalid,"application/json-seq");throw new AssertionError("invalid sequence accepted");}catch(IllegalStateException expected){} }
  var server=com.sun.net.httpserver.HttpServer.create(new java.net.InetSocketAddress("127.0.0.1",0),0);
  var parts=java.util.List.of(OrderedMultipart.Part.json("metadata",mapper.readTree("{\"future\":\"雪\"}")),OrderedMultipart.Part.nested("nested",java.util.List.of(OrderedMultipart.Part.bytes("child","false".getBytes(java.nio.charset.StandardCharsets.UTF_8),"text/plain"))),OrderedMultipart.Part.bytes("file",new byte[]{0,(byte)255},"application/octet-stream"));
  server.createContext("/params",exchange->{try{if(exchange.getRequestURI().getRawPath().equals("/params/null")){if(!exchange.getRequestURI().getRawQuery().equals("filter=null")||!exchange.getRequestHeaders().getFirst("x-json").equals("null")||!exchange.getRequestHeaders().getFirst("Cookie").equals("cookie=null"))throw new AssertionError("required null content wire");exchange.sendResponseHeaders(204,-1);return;}if(!exchange.getRequestURI().getRawPath().equals("/params/%22hello%20world%22")||!exchange.getRequestURI().getRawQuery().equals("filter=%22query%22")||!exchange.getRequestHeaders().getFirst("x-json").equals("\"header\"")||!exchange.getRequestHeaders().getFirst("Cookie").equals("cookie=%22cookie%22"))throw new AssertionError("JSON parameter wire");exchange.sendResponseHeaders(204,-1);}finally{exchange.close();}});
  server.createContext("/collision",exchange->{try{if(!exchange.getRequestURI().getRawQuery().equals("notify=notify&filter-a=dash&filter_a=underscore") || !exchange.getRequestHeaders().getFirst("notify").equals("header"))throw new AssertionError("allocated arguments changed wire names"); exchange.sendResponseHeaders(204,-1);}finally{exchange.close();}});
  server.createContext("/ordered",exchange->{try{var raw=exchange.getRequestBody().readAllBytes();var text=new String(raw,java.nio.charset.StandardCharsets.UTF_8);if(!exchange.getRequestHeaders().getFirst("Content-Type").startsWith("multipart/mixed")||!text.contains("X-Part: v1")||!text.contains("X-Child: child")||!text.contains("Content-Disposition: attachment"))throw new AssertionError("ordered MIME wire");exchange.sendResponseHeaders(204,-1);}finally{exchange.close();}});
  server.createContext("/sequence",exchange->{try {
   if(!exchange.getRequestURI().getRawQuery().equals("zero=0&false=false&name=%E9%9B%AA"))throw new AssertionError("query wire");
   var request=new String(exchange.getRequestBody().readAllBytes(),java.nio.charset.StandardCharsets.UTF_8);
   if(!mapper.readTree(runtime.normalizeSequentialJson(request,"application/x-ndjson")).equals(records))throw new AssertionError("sequence request wire");
   var bytes=runtime.encodeSequentialJson(records,"application/x-ndjson").getBytes(java.nio.charset.StandardCharsets.UTF_8);
   exchange.getResponseHeaders().set("Content-Type","application/x-ndjson");exchange.sendResponseHeaders(200,bytes.length);exchange.getResponseBody().write(bytes);
  } finally {exchange.close();}});server.start();
  try {var client=new Client(new ClientConfig("http://127.0.0.1:"+server.getAddress().getPort(),null));
   client.jsonParameters(new Client.JsonParametersRequest("hello world","query","header","cookie"));
   client.jsonParameters(new Client.JsonParametersRequest(null,null,null,null));
   client.ordered(new Client.OrderedRequest(new OrderedMultipartBody(parts)));
   client.collisionWire(new Client.CollisionWireRequest("notify","dash","underscore","header"));
   var result=client.sequence(new Client.SequenceRequest("zero=0&false=false&name=%E9%9B%AA",mapper.convertValue(records,new com.fasterxml.jackson.core.type.TypeReference<java.util.List<JsonNode>>(){})));
   if(!result.equals(records))throw new AssertionError("sequence response wire");
  }finally{server.stop(0);}
  validateWholeQuery("zero=0&false=false&name=%E9%9B%AA");
  for(var invalid:new String[]{"?a=1","a=#fragment","a=%ZZ","a=%","a=\n"}) {try{validateWholeQuery(invalid);throw new AssertionError("invalid query accepted");}catch(IllegalArgumentException expected){}}

  var count=mapper.readValue("42",Count.class);if(count.value()!=42L || !mapper.writeValueAsString(count).equals("42"))throw new AssertionError("scalar alias");
  for(var wire:new String[]{"42","true","\"future\"","{\"unknown\":[1,null]}"}) {
   var choice=mapper.readValue(wire,Choice.class);
   if(!mapper.readTree(mapper.writeValueAsString(choice)).equals(mapper.readTree(wire)))throw new AssertionError("union wire");
  }
  var state=mapper.readValue("\"future\"",State.class);
  if(!state.value().equals("future") || !mapper.writeValueAsString(state).equals("\"future\""))throw new AssertionError("unknown enum");
 }
}"#).unwrap();
    let result = std::process::Command::new("mvn")
        .args([
            "-q",
            "test-compile",
            "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
            "-Dexec.mainClass=io.kaji.compat.ModelsProbe",
            "-Dexec.classpathScope=test",
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
