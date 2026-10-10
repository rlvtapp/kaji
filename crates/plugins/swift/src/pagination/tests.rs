use super::*;

#[test]
#[ignore = "Requires Swift6; real generated offset/URL operations and fake native transport"]
fn native_offset_and_url_pagination_preserves_auth_and_laziness() {
    use super::*;
    use poolster_core::{
        AdditionalProperties, Field, HttpMethod, OperationParameter, OperationResponse, Schema,
    };
    let integer = |name: &str| OperationParameter {
        name: name.into(),
        location: "query".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::Integer)),
        description: None,
        annotations: Default::default(),
    };
    let mut offset = Operation {
        id: "listItems".into(),
        method: HttpMethod::Get,
        path: "/items".into(),
        parameters: vec![integer("offset"), integer("limit")],
        responses: vec![OperationResponse::json(
            "200",
            SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::String)),
            }),
        )],
        ..Default::default()
    };
    offset.annotations.insert("x-poolster-pagination".into(),serde_json::json!({"type":"offsetLimit","inputs":[{"name":"offset","type":"offset"},{"name":"limit","type":"limit"}],"outputs":{"results":"$"}}));
    let mut url = Operation {
        id: "listLinks".into(),
        method: HttpMethod::Get,
        path: "/links".into(),
        parameters: vec![OperationParameter {
            name: "tenant".into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        }],
        responses: vec![OperationResponse::json(
            "200",
            SchemaValue::reference("#/components/schemas/Page"),
        )],
        ..Default::default()
    };
    url.annotations.insert(
        "x-poolster-pagination".into(),
        serde_json::json!({"type":"url","outputs":{"nextUrl":"/next"}}),
    );
    let api = Api {
        name: "Pages".into(),
        version: "1.0.0".into(),
        operations: vec![offset, url],
        schemas: vec![Schema::new(
            "Page",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![Field {
                    name: "next".into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: false,
                    annotations: Default::default(),
                }],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        )],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_sdk(&api, "sdk", Some("Pages"), SdkClientStyle::Namespaced)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let main = root.path().join("main.swift");
    std::fs::write(
        &main,
        include_str!("../../tests/fixtures/pagination_probe.swift"),
    )
    .unwrap();
    fn sources(path: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, out)
            } else if path.extension().is_some_and(|ext| ext == "swift") {
                out.push(path)
            }
        }
    }
    let mut files = vec![];
    sources(&root.path().join("sdk/Sources"), &mut files);
    let output = std::process::Command::new("swiftc")
        .args([
            "-parse-as-library",
            "-swift-version",
            "6",
            "-warnings-as-errors",
            "-module-cache-path",
        ])
        .arg(root.path().join("cache"))
        .args(files)
        .arg(main)
        .arg("-o")
        .arg(root.path().join("probe"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::process::Command::new(root.path().join("probe"))
            .status()
            .unwrap()
            .success()
    );
}
#[test]
#[ignore = "requires Swift toolchain"]
fn native_page_sequence_is_lazy_and_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = temp.path().join("Pagination.swift");
    std::fs::write(
        &runtime,
        include_str!("../../templates/page_runtime.swift.tmpl"),
    )
    .unwrap();
    let main = temp.path().join("main.swift");
    std::fs::write(&main, r#"
import Foundation
actor Calls { var pages: [Int] = []; func record(_ page: Int) { pages.append(page) }; func values() -> [Int] { pages } }
@main struct Test {
 static func main() async throws {
  let calls = Calls()
  let sequence = PoolsterPageSequence<[Int]>(page: 0, limit: 2) { page in
   await calls.record(page); return (page == 0 ? [1,2] : [3], page == 0 ? 2 : 1)
  }
  let before = await calls.values(); precondition(before.isEmpty)
  var iterator = sequence.makeAsyncIterator()
  let first = try await iterator.next(); precondition(first == [1,2])
  let second = try await iterator.next(); precondition(second == [3])
  let end = try await iterator.next(); precondition(end == nil)
  let pages = await calls.values(); precondition(pages == [0,1])
  var invalid = PoolsterPageSequence<Int>(page: -1, limit: nil) { _ in fatalError("must not request") }.makeAsyncIterator()
  do { _ = try await invalid.next(); fatalError("invalid page") } catch PoolsterPaginationError.invalidPage {}
  var overflow = PoolsterPageSequence<Int>(page: Int.max, limit: nil) { _ in (1,1) }.makeAsyncIterator()
  _ = try await overflow.next()
  do { _ = try await overflow.next(); fatalError("overflow") } catch PoolsterPaginationError.pageOverflow {}
  let count = try poolsterPageCount(["a/b": [[1],[2,3]]], [.field("a/b"), .index(-1)])
  precondition(count == 2)
  for key in ["01", "+1", "-1", ""] {
    do { _ = try poolsterPageCount([1,2], [.field(key)]); fatalError("invalid pointer index") }
    catch PoolsterPaginationError.invalidResults {}
  }

 }
}
"#).unwrap();
    let exe = temp.path().join("test");
    let output = std::process::Command::new("swiftc")
        .args([
            "-parse-as-library",
            "-swift-version",
            "6",
            "-warnings-as-errors",
            "-module-cache-path",
        ])
        .arg(temp.path().join("cache"))
        .arg(runtime)
        .arg(main)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(std::process::Command::new(exe).status().unwrap().success());
}

#[cfg(test)]
mod generation_tests {
    use super::*;
    #[test]
    #[ignore = "requires Swift toolchain"]
    fn declared_pages_preserve_required_and_optional_parameters() {
        use poolster_core::{HttpMethod, OperationParameter, OperationResponse};
        let mut operation = Operation {
            id: "listPets".into(),
            method: HttpMethod::Get,
            path: "/pets/{page}".into(),
            ..Default::default()
        };
        operation.parameters = vec![OperationParameter {
            name: "page".into(),
            location: "path".into(),
            required: true,
            schema: Some(SchemaValue::new(SchemaKind::Integer)),
            description: None,
            annotations: Default::default(),
        }];
        operation.responses = vec![OperationResponse::json(
            "200",
            SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::String)),
            }),
        )];
        operation.annotations.insert("x-poolster-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page","in":"parameters"}],"outputs":{"results":"$"}}));
        let mut api = Api {
            operations: vec![operation],
            ..Default::default()
        };
        let source = render(&api).unwrap();
        assert!(source.contains("listPetsPages(page: Int)"));
        assert!(source.contains("listPets(page: current)"));
        api.operations[0].parameters[0].required = false;
        api.operations[0].parameters[0].location = "header".into();
        api.operations[0].path = "/pets".into();
        assert!(render(&api).unwrap().contains("page: page ?? 1"));
        let tree = render_sdk(&api, "sdk", Some("Pets"), SdkClientStyle::Namespaced).unwrap();
        let dir = tempfile::tempdir().unwrap();
        for (relative, contents) in tree.iter() {
            let path = dir.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        let main = dir.path().join("main.swift");
        std::fs::write(&main, r#"
import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
actor Calls {
 private var count = 0
 func record() { count += 1 }
 func snapshot() -> Int { count }
}
struct Driver: PoolsterTransport {
 let calls: Calls
 func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
  await calls.record()
  precondition(request.value(forHTTPHeaderField: "page") == "0")
  return (Data("[]".utf8), HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!)
 }
}
struct Blocking: PoolsterTransport {
 let calls: Calls
 func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
  await calls.record()
  try await Task.sleep(nanoseconds: 60_000_000_000)
  throw CancellationError()
 }
}
@main struct Test {
 static func main() async throws {
  let calls = Calls()
  let client = PoolsterClient(options: .init(baseURL: URL(string: "https://example.com")!), transport: Driver(calls: calls))
  var iterator = client.pets.listPetsPages(page: 0).makeAsyncIterator()
  let first = try await iterator.next(); precondition(first == [])
  let end = try await iterator.next(); precondition(end == nil)
  let before = await calls.snapshot(); precondition(before == 1)
  let cancelled = Task {
   while !Task.isCancelled { await Task.yield() }
   var pages = client.listPetsPages(page: 0).makeAsyncIterator()
   do { _ = try await pages.next(); preconditionFailure("cancelled paginator sent request") }
   catch is CancellationError {} catch { preconditionFailure("wrong cancellation error") }
  }
  cancelled.cancel(); await cancelled.value
  let after = await calls.snapshot(); precondition(after == 1)
  let blockedCalls = Calls()
  let blocked = PoolsterClient(options: .init(baseURL: URL(string: "https://example.com")!), transport: Blocking(calls: blockedCalls))
  let pending = Task {
   var pages = blocked.listPetsPages(page: 0).makeAsyncIterator()
   return try await pages.next()
  }
  while await blockedCalls.snapshot() == 0 { await Task.yield() }
  pending.cancel()
  do { _ = try await pending.value; preconditionFailure("in-flight cancellation swallowed") }
  catch is CancellationError {}
  let attempts = await blockedCalls.snapshot(); precondition(attempts == 1)
 }
}
"#).unwrap();
        let exe = dir.path().join("probe");
        let mut command = std::process::Command::new("swiftc");
        command
            .args([
                "-parse-as-library",
                "-swift-version",
                "6",
                "-warnings-as-errors",
                "-module-cache-path",
            ])
            .arg(dir.path().join("cache"));
        for (relative, _) in tree.iter().filter(|(path, _)| {
            path.to_string_lossy().contains("Sources/")
                && path.extension().is_some_and(|ext| ext == "swift")
        }) {
            command.arg(dir.path().join(relative));
        }
        let output = command.arg(main).arg("-o").arg(&exe).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(std::process::Command::new(exe).status().unwrap().success());
    }
}
