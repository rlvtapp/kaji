use super::*;

#[test]
fn native_whole_query_and_sequential_json() {
    let operation = Operation {
        id: "events".into(),
        method: HttpMethod::Get,
        path: "/events".into(),
        parameters: vec![OperationParameter {
            name: "filter".into(),
            location: "querystring".into(),
            required: true,
            schema: Some(SchemaValue::new(SchemaKind::Object {
                fields: vec![],
                additional_properties: AdditionalProperties::Any,
            })),
            description: None,
            annotations: BTreeMap::from([(
                "poolster.parameter_content".into(),
                serde_json::json!([{"content_type":"application/x-www-form-urlencoded","schema_definition":{"type":"object"}}]),
            )]),
        }],
        responses: vec![OperationResponse {
            status: "200".into(),
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/x-ndjson".into(),
                schema: Some(SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                })),
            }],
        }],
        ..Default::default()
    };
    let upload = Operation {
        id: "sendEvents".into(),
        method: HttpMethod::Post,
        path: "/events".into(),
        request_body: Some(OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/json-seq".into(),
                schema: Some(SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                })),
            }],
        }),
        ..Default::default()
    };
    let api = Api {
        name: "Wire".into(),
        version: "1".into(),
        operations: vec![operation, upload],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api, "sdk", Some("wire"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(root.path().join("sdk/wire_test.go"),r#"package wire
import("context";"io";"net/http";"net/url";"strings";"testing")
type wireTransport struct{}
func(wireTransport)Do(request *http.Request)(*http.Response,error){
 if request.Method=="POST" {body,_:=io.ReadAll(request.Body);if request.Header.Get("Content-Type")!="application/json-seq"||string(body)!="\x1e0\n\x1e2\n"{panic(string(body))};return &http.Response{StatusCode:204,Header:http.Header{},Body:io.NopCloser(strings.NewReader("")),Request:request},nil}
 if request.URL.Query().Get("term")!="space & plus+"||request.URL.Query().Get("zero")!="0"||request.URL.Query().Get("flag")!="false"||request.URL.Query().Has("filter"){panic(request.URL.RawQuery)}
 return &http.Response{StatusCode:200,Header:http.Header{"Content-Type":[]string{"application/x-ndjson"}},Body:io.NopCloser(strings.NewReader("0\n1\n2\n")),Request:request},nil
}
func TestWholeQueryAndSequence(t *testing.T){
 client,err:=NewClient(ClientConfig{BaseURL:"https://example.test",HTTPClient:wireTransport{}});if err!=nil{t.Fatal(err)}
 values,err:=client.Events(context.Background(),&EventsRequest{Filter:map[string]any{"term":"space & plus+","zero":0,"flag":false}});if err!=nil||len(*values)!=3||(*values)[2]!=2{t.Fatal(values,err)}
 if err=client.SendEvents(context.Background(),&SendEventsRequest{Body:[]int64{0,2}});err!=nil{t.Fatal(err)}
 encoded,err:=poolsterWholeQuery(map[string]any{"csv":[]int{1,2},"json":map[string]any{"flag":false}},"application/x-www-form-urlencoded",`{"encoding":{"csv":{"explode":false},"json":{"contentType":"application/json"}}}`);if err!=nil{t.Fatal(err)};parsed,_:=url.ParseQuery(encoded);if parsed.Get("csv")!="1,2"||parsed.Get("json")!=`{"flag":false}`{t.Fatal(encoded)}
 var sequence []int64;if err=client.decodeSequentialResponse(strings.NewReader("\x1e0\n\x1e2\n"),"application/json-seq",&sequence);err!=nil||len(sequence)!=2{t.Fatal(sequence,err)}
 for _,bad:=range []string{"1 2\n","{bad}\n"}{if err=client.decodeSequentialResponse(strings.NewReader(bad),"application/x-ndjson",&sequence);err==nil{t.Fatal("accepted malformed record")}}
 if err=client.decodeSequentialResponse(strings.NewReader("0\n"),"application/json-seq",&sequence);err==nil{t.Fatal("accepted missing separator")}
}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "-race", "./..."])
        .current_dir(root.path().join("sdk"))
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
fn native_positional_nested_multipart_uses_declared_encoding() {
    let operation = Operation {
        id: "upload".into(),
        method: HttpMethod::Put,
        path: "/upload".into(),
        request_body: Some(OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "multipart/mixed".into(),
                schema: Some(SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Any)),
                })),
            }],
        }),
        annotations: BTreeMap::from([(
            "poolster.request_content".into(),
            serde_json::json!([{"content_type":"multipart/mixed","prefix_encoding":[{"contentType":"text/plain","headers":{"Content-ID":{"required":true}}},{"contentType":"multipart/mixed","prefixEncoding":[{"contentType":"application/json"}],"itemEncoding":{"contentType":"image/png"}}],"item_encoding":{"contentType":"application/octet-stream"}}]),
        )]),
        ..Default::default()
    };
    let api = Api {
        name: "MIME".into(),
        version: "1".into(),
        operations: vec![operation],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api, "sdk", Some("wire"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(root.path().join("sdk/positional_test.go"),r#"package wire
import("context";"io";"mime";"mime/multipart";"net/http";"strings";"testing")
type mimeTransport struct{t *testing.T}
func(transport mimeTransport)Do(request *http.Request)(*http.Response,error){
 media,parameters,err:=mime.ParseMediaType(request.Header.Get("Content-Type"));if err!=nil||media!="multipart/mixed"{transport.t.Fatal(media,err)}
 reader:=multipart.NewReader(request.Body,parameters["boundary"]);first,err:=reader.NextPart();if err!=nil||first.Header.Get("Content-Type")!="text/plain"||first.Header.Get("Content-ID")!="first"{transport.t.Fatal(first,err)};data,_:=io.ReadAll(first);if string(data)!="hello"{transport.t.Fatal(string(data))}
 second,_:=reader.NextPart();nestedType,nestedParameters,_:=mime.ParseMediaType(second.Header.Get("Content-Type"));if nestedType!="multipart/mixed"{transport.t.Fatal(nestedType)};nested:=multipart.NewReader(second,nestedParameters["boundary"]);jsonPart,_:=nested.NextPart();if jsonPart.Header.Get("Content-Type")!="application/json"{transport.t.Fatal(jsonPart.Header)};io.ReadAll(jsonPart);png,_:=nested.NextPart();if png.Header.Get("Content-Type")!="image/png"{transport.t.Fatal(png.Header)};data,_=io.ReadAll(png);if len(data)!=2||data[0]!=0xff{transport.t.Fatal(data)}
 last,_:=reader.NextPart();if last.Header.Get("Content-Type")!="application/octet-stream"{transport.t.Fatal(last.Header)};io.ReadAll(last);if _,err=reader.NextPart();err!=io.EOF{transport.t.Fatal(err)}
 return &http.Response{StatusCode:204,Header:http.Header{},Body:io.NopCloser(strings.NewReader("")),Request:request},nil
}
func TestPositional(t *testing.T){
 client,_:=NewClient(ClientConfig{BaseURL:"https://example.test",HTTPClient:mimeTransport{t}})
 nested:=&PoolsterMultipartBody{Parts:[]PoolsterMultipartPart{{Data:[]byte(`{"zero":0}`)},{Data:[]byte{0xff,0}}}}
 body:=&PoolsterMultipartBody{Parts:[]PoolsterMultipartPart{{Data:[]byte("hello"),Headers:http.Header{"Content-Id":[]string{"first"}}},{Nested:nested},{Data:[]byte{0}}}}
 if err:=client.Upload(context.Background(),&UploadRequest{Body:body});err!=nil{t.Fatal(err)}
 if body.ContentType!=""||body.Parts[0].ContentType!=""||nested.Parts[0].ContentType!=""{t.Fatal("mutated caller builder")}
 body.Parts[0].Headers=nil;if err:=client.Upload(context.Background(),&UploadRequest{Body:body});err==nil{t.Fatal("required header ignored")}
}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "-race", "./..."])
        .current_dir(root.path().join("sdk"))
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
fn native_named_parameter_content_preserves_json_values() {
    let parameters = [
        ("selector", "path", SchemaValue::new(SchemaKind::String)),
        (
            "values",
            "query",
            SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::Any)),
            }),
        ),
        (
            "X-Metadata",
            "header",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![],
                additional_properties: AdditionalProperties::Any,
            }),
        ),
        ("flag", "cookie", SchemaValue::new(SchemaKind::Boolean)),
    ]
    .into_iter()
    .map(|(name, location, schema)| OperationParameter {
        name: name.into(),
        location: location.into(),
        required: true,
        schema: Some(schema),
        description: None,
        annotations: BTreeMap::from([(
            "poolster.parameter_content".into(),
            serde_json::json!([{"content_type":"application/json"}]),
        )]),
    })
    .collect();
    let operation = Operation {
        id: "content".into(),
        method: HttpMethod::Get,
        path: "/content/{selector}".into(),
        parameters,
        responses: vec![OperationResponse {
            status: "200".into(),
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "multipart/mixed".into(),
                schema: Some(SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Any)),
                })),
            }],
        }],
        ..Default::default()
    };
    let api = Api {
        name: "Content".into(),
        version: "1".into(),
        operations: vec![operation],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api, "sdk", Some("content"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(root.path().join("sdk/content_test.go"),r#"package content
import("context";"io";"net/http";"strings";"testing")
type contentTransport struct{t *testing.T}
func(transport contentTransport)Do(request *http.Request)(*http.Response,error){
 if request.URL.Path!=`/content/"a/b"`||request.URL.Query().Get("values")!=`[0,false,null]`||len(request.URL.Query()["values"])!=1||request.Header.Get("X-Metadata")!=`{"flag":false,"zero":0}`||request.Header.Get("Cookie")!="flag=false"{transport.t.Fatal(request.URL.String(),request.Header)}
 return &http.Response{StatusCode:200,Body:io.NopCloser(strings.NewReader(string([]byte{0xff,0,13,10}))),Header:http.Header{"Content-Type":[]string{"multipart/mixed; boundary=test"}},Request:request},nil
}
func TestNamedContent(t *testing.T){client,_:=NewClient(ClientConfig{BaseURL:"https://example.test",HTTPClient:contentTransport{t}});raw,err:=client.Content(context.Background(),&ContentRequest{Selector:"a/b",Values:[]any{0,false,nil},XMetadata:map[string]any{"flag":false,"zero":0},Flag:false});if err!=nil||len(raw)!=4||raw[0]!=0xff{t.Fatal(raw,err)}}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "-race", "./..."])
        .current_dir(root.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
