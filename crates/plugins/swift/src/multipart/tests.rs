use super::*;
use poolster_core::{OperationMediaType, OperationRequestBody, OperationResponse};
use serde_json::json;
use std::{fs, process::Command};

fn field(name: &str, value: SchemaValue) -> Field {
    Field {
        name: name.into(),
        value,
        required: true,
        annotations: Default::default(),
    }
}
fn fixture() -> Api {
    let scalar = |name: &str, kind| field(name, SchemaValue::new(kind));
    let mut binary = SchemaValue::new(SchemaKind::String);
    binary.format = Some("binary".into());
    let metadata = SchemaValue::new(SchemaKind::Object {
        fields: vec![scalar("value", SchemaKind::String)],
        additional_properties: AdditionalProperties::Forbidden,
    });
    let mut fields = vec![
        scalar("title", SchemaKind::String),
        scalar("enabled", SchemaKind::Boolean),
        scalar("count", SchemaKind::Integer),
    ];
    for name in ["tags", "csv"] {
        fields.push(field(
            name,
            SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::String)),
            }),
        ));
    }
    fields.push(scalar("jsonText", SchemaKind::String));
    fields.push(field(
        "metadata",
        SchemaValue::reference("#/components/schemas/Metadata"),
    ));
    fields.push(field(
        "metadatas",
        SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::reference("#/components/schemas/Metadata")),
        }),
    ));
    fields.push(field("file", binary.clone()));
    fields.push(field(
        "files",
        SchemaValue::new(SchemaKind::Array {
            items: Box::new(binary),
        }),
    ));
    let mut optional = field("optional", SchemaValue::new(SchemaKind::String));
    optional.required = false;
    fields.push(optional);
    let mut upload = Operation {
        id: "upload".into(),
        path: "/upload".into(),
        method: poolster_core::HttpMethod::Post,
        request_body: Some(OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "multipart/form-data".into(),
                schema: Some(SchemaValue::reference("#/components/schemas/UploadForm")),
            }],
        }),
        responses: vec![OperationResponse {
            status: "204".into(),
            description: None,
            media_types: vec![],
        }],
        ..Default::default()
    };
    upload.annotations.insert("poolster.request_body_encodings".into(),json!({"multipart/form-data":{"csv":{"style":"form","explode":false,"contentType":"application/json"},"jsonText":{"contentType":"application/json"},"metadata":{"contentType":"application/json","headers":{"X-Part":{"required":true},"Content-Type":{"required":true}}},"file":{"headers":{"X-File":{"required":true}}},"files":{"contentType":"image/png"}}}));
    let mut submit = upload.clone();
    submit.id = "submit".into();
    submit.path = "/submit".into();
    submit
        .request_body
        .as_mut()
        .unwrap()
        .media_types
        .push(OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(SchemaValue::reference("#/components/schemas/Metadata")),
        });
    Api {
        name: "Multipart Probe".into(),
        version: "1".into(),
        schemas: vec![
            Schema::new("Metadata", metadata),
            Schema::new(
                "UploadForm",
                SchemaValue::new(SchemaKind::Object {
                    fields,
                    additional_properties: AdditionalProperties::Forbidden,
                }),
            ),
        ],
        operations: vec![upload, submit],
        ..Default::default()
    }
}
fn tree(api: &Api) -> GeneratedTree {
    render_sdk(api, "sdk", Some("multipart-probe"), SdkClientStyle::Flat).unwrap()
}
#[test]
fn typed_multipart_does_not_change_json_models_and_preserves_encoding_selection() {
    let tree = tree(&fixture());
    let dto = tree
        .get("sdk/Sources/MultipartProbe/UploadMultipartBody.swift")
        .unwrap();
    assert!(dto.contains("file: PoolsterMultipartFile"));
    assert!(dto.contains("files: [PoolsterMultipartFile]"));
    assert!(dto.contains("requiredHeaders: [\"X-File\"]"));
    assert!(
        tree.get("sdk/Sources/MultipartProbe/Models/UploadForm.swift")
            .unwrap()
            .contains("file: String")
    );
    let mixed = tree
        .get("sdk/Sources/MultipartProbe/SubmitMultipartBody.swift")
        .unwrap();
    assert!(mixed.contains("case json(Metadata)"));
    assert!(mixed.contains("case multipart(SubmitMultipartBody)"));
    assert!(
        tree.get("sdk/Sources/MultipartProbe/Operations.swift")
            .unwrap()
            .contains("request.setValue(encoded.contentType")
    );
}
#[test]
fn wider_multipart_shapes_use_explicit_ordered_parts() {
    for mutate in [0, 1, 2, 3] {
        let mut api = fixture();
        let SchemaKind::Object {
            fields,
            additional_properties,
        } = &mut api.schemas[1].value.kind
        else {
            unreachable!()
        };
        match mutate {
            0 => *additional_properties = AdditionalProperties::Any,
            1 => fields[0].value.nullable = true,
            2 => fields[0].name = "bad\r\nname".into(),
            _ => fields[0].name = "partHeaders".into(),
        }
        let generated = render_sdk(&api, ".", None, SdkClientStyle::Flat).unwrap();
        assert!(
            generated
                .iter()
                .any(|(_, source)| source.contains("public var parts: [PoolsterOrderedPart]"))
        );
    }
}
fn native_probe(directory: &std::path::Path) -> std::path::PathBuf {
    let tree = tree(&fixture());
    tree.write_to(directory).unwrap();
    fs::write(
        directory.join("Probe.swift"),
        include_str!("../../tests/fixtures/multipart_probe.swift"),
    )
    .unwrap();
    let mut compiler = Command::new("swiftc");
    compiler.args(["-swift-version", "6", "-parse-as-library"]);
    for (path, _) in tree.iter().filter(|(path, _)| {
        path.starts_with("sdk/Sources/")
            && path
                .extension()
                .is_some_and(|extension| extension == "swift")
    }) {
        compiler.arg(directory.join(path));
    }
    let binary = directory.join("probe");
    compiler
        .arg(directory.join("Probe.swift"))
        .arg("-o")
        .arg(&binary);
    let cache = std::env::temp_dir().join("poolster-swift-cache");
    compiler
        .env("CLANG_MODULE_CACHE_PATH", &cache)
        .env("SWIFT_MODULECACHE_PATH", &cache);
    let output = compiler.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}
#[test]
#[ignore = "requires Swift 6 native compiler"]
fn native_multipart_mime_bytes_mixed_media_limits_headers_and_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    let binary = native_probe(dir.path());
    let output = Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Swift 6 compiler and localhost TCP sockets"]
fn native_urlsession_multipart_wire_retry_guard_and_socket_cancellation() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};
    let dir = tempfile::tempdir().unwrap();
    let binary = native_probe(dir.path());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let marker = dir.path().join("cancel-received");
    let server_marker = marker.clone();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut seen = Vec::new();
        while seen.len() < 3 {
            assert!(Instant::now() < deadline, "HTTP requests did not arrive");
            let (mut stream, _) = match listener.accept() {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(error) => panic!("accept: {error}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 4096];
            let header_end = loop {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0, "truncated HTTP header");
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(offset) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
                    break offset + 4;
                }
                assert!(bytes.len() < 65536);
            };
            let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap();
            while bytes.len() < header_end + length {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0, "truncated multipart payload");
                bytes.extend_from_slice(&buffer[..count]);
            }
            assert!(headers.contains("POST "));
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("content-type: multipart/form-data; boundary=")
            );
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer author")
            );
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("x-middleware: installed")
            );
            let body = &bytes[header_end..];
            assert!(body.windows(5).any(|value| value == [0, 255, 13, 10, 65]));
            assert!(body.windows(14).any(|value| value == b"name=\"enabled\""));
            let path = headers
                .lines()
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .to_owned();
            seen.push(path.clone());
            if path == "/cancel/upload" {
                fs::write(&server_marker, b"received").unwrap();
                match stream.read(&mut buffer) {
                    Ok(0) => {}
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::ConnectionReset
                                | std::io::ErrorKind::ConnectionAborted
                        ) => {}
                    other => panic!("cancellation did not close socket: {other:?}"),
                }
            } else {
                let status = if path == "/fail/upload" {
                    "503 Service Unavailable"
                } else {
                    "204 No Content"
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
            }
        }
        assert_eq!(seen, ["/upload", "/fail/upload", "/cancel/upload"]);
    });
    let output = Command::new(binary)
        .env("POOLSTER_MULTIPART_URL", format!("http://{address}"))
        .env("POOLSTER_MULTIPART_CANCEL_MARKER", &marker)
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Swift 6 native compiler"]
fn native_ordered_nested_multipart_applies_positional_plan() {
    let operation = Operation {
        id: "orderedUpload".into(),
        method: poolster_core::HttpMethod::Put,
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
        annotations: std::collections::BTreeMap::from([(
            "poolster.request_content".into(),
            json!([{"content_type":"multipart/mixed","prefix_encoding":[{"contentType":"text/plain","headers":{"Content-ID":{"required":true}}},{"contentType":"multipart/mixed","prefixEncoding":[{"contentType":"application/json"}],"itemEncoding":{"contentType":"image/png"}}],"item_encoding":{"contentType":"application/octet-stream"}}]),
        )]),
        ..Default::default()
    };
    let download = Operation {
        id: "download".into(),
        method: poolster_core::HttpMethod::Get,
        path: "/incoming".into(),
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
        name: "Ordered".into(),
        version: "1".into(),
        operations: vec![operation, download],
        ..Default::default()
    };
    let tree = render_sdk(&api, "sdk", Some("Ordered"), SdkClientStyle::Namespaced).unwrap();
    let directory = tempfile::tempdir().unwrap();
    tree.write_to(directory.path()).unwrap();
    std::fs::write(directory.path().join("Probe.swift"),r#"import Foundation
actor ResponseTransport: PoolsterTransport {
 func execute(_ request:URLRequest) async throws -> (Data,URLResponse) {(Data([0xff,0,13,10]),HTTPURLResponse(url:request.url!,statusCode:200,httpVersion:nil,headerFields:["Content-Type":"multipart/mixed; boundary=test"])!)}
}
@main struct Probe {
 static func main() async throws {
  let client=PoolsterClient(options:PoolsterClientOptions(baseURL:URL(string:"https://example.test")!),transport:ResponseTransport())
  let received:Data=try await client.download();guard received == Data([0xff,0,13,10]) else {fatalError("MIME response bytes changed")}
  let facade:Data=try await client.incoming.download();guard facade == received else {fatalError("facade response differs")}
  let parts:[PoolsterOrderedPart] = [.init(.text("hello"), headers:["Content-ID":"first"]), .init(.nested([.init(.json(.object(["zero":.integer(0),"flag":.bool(false),"nil":.null]))),.init(.bytes(Data([0xff,0])))])),.init(.bytes(Data([0]))) ]
  let body=OrderedUploadMultipartBody(parts:parts)
  let encoded=try body.poolsterEncoded()
  guard encoded.contentType.hasPrefix("multipart/mixed; boundary="),encoded.body.range(of:Data([0xff,0])) != nil else {fatalError("subtype or bytes lost")}
  let readable=String(decoding:encoded.body,as:UTF8.self)
  guard readable.contains("Content-ID: first"),readable.contains("Content-Type: image/png"),readable.contains("Content-Type: application/json"),readable.contains("Content-Type: application/octet-stream"),readable.contains("\"nil\":null"),!readable.contains("name=\"\"") else {fatalError(readable)}
  let first=readable.range(of:"hello")!,nested=readable.range(of:"Content-Type: multipart/mixed;")!;guard first.lowerBound<nested.lowerBound else {fatalError("part order changed")}
  do {_ = try OrderedUploadMultipartBody(parts:[.init(.text("no header"))]).poolsterEncoded();fatalError("required part header ignored")}catch PoolsterMultipartError.missingHeader("Content-ID") {}
  do {_ = try OrderedUploadMultipartBody(parts:parts,maximumBodyBytes:8).poolsterEncoded();fatalError("limit ignored")}catch PoolsterMultipartError.bodyTooLarge {}
  let cancelled=Task {try Task.checkCancellation();return try body.poolsterEncoded()};cancelled.cancel();do {_ = try await cancelled.value;fatalError("cancellation ignored")}catch is CancellationError {}
 }
}
"#).unwrap();
    let mut compiler = Command::new("swiftc");
    compiler.args(["-swift-version", "6", "-parse-as-library"]);
    for (path, _) in tree.iter().filter(|(path, _)| {
        path.starts_with("sdk/Sources/")
            && path
                .extension()
                .is_some_and(|extension| extension == "swift")
    }) {
        compiler.arg(directory.path().join(path));
    }
    let binary = directory.path().join("probe");
    compiler
        .arg(directory.path().join("Probe.swift"))
        .arg("-o")
        .arg(&binary);
    let cache = std::env::temp_dir().join("poolster-swift-cache");
    compiler
        .env("CLANG_MODULE_CACHE_PATH", &cache)
        .env("SWIFT_MODULECACHE_PATH", &cache);
    let output = compiler.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
