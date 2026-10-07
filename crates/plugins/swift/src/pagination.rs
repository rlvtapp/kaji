use super::*;
use kaji_core::pagination::{PaginationKind, SelectorSegment, normalize_pagination};

pub(super) fn render(api: &Api) -> Result<String> {
    let mut output = String::new();
    for operation in &api.operations {
        let extension = operation
            .annotations
            .get("x-kaji-pagination")
            .or_else(|| operation.annotations.get("x-speakeasy-pagination"));
        if extension
            .and_then(|value| value.get("type"))
            .and_then(serde_json::Value::as_str)
            != Some("page")
        {
            continue;
        }
        let Some(plan) = normalize_pagination(api, operation, None)? else {
            continue;
        };
        if plan.kind != PaginationKind::Page {
            continue;
        }
        let page = plan
            .inputs
            .iter()
            .find(|input| input.role == "page")
            .unwrap();
        let limit = plan.inputs.iter().find(|input| input.role == "limit");
        // Body bindings need immutable model copying, which this native API does not provide.
        if plan
            .inputs
            .iter()
            .any(|input| input.location == "requestBody")
        {
            bail!(
                "Swift page pagination for {} requires parameter controls",
                operation.id
            );
        }
        for input in &plan.inputs {
            let parameter = operation
                .parameters
                .iter()
                .find(|parameter| parameter.name == input.name)
                .unwrap();
            let mut value = parameter.schema.as_ref().unwrap();
            for _ in 0..128 {
                if value.nullable || value.optional || !value.enum_values.is_empty() {
                    bail!(
                        "Swift page controls must be nonnullable integer scalars: {}",
                        input.name
                    );
                }
                match &value.kind {
                    SchemaKind::Reference { reference } => {
                        let name = reference.rsplit('/').next().unwrap();
                        value = &api
                            .schemas
                            .iter()
                            .find(|schema| schema.name == name)
                            .unwrap()
                            .value;
                    }
                    SchemaKind::Integer => break,
                    _ => bail!(
                        "Swift page controls must be integer scalars: {}",
                        input.name
                    ),
                }
            }
        }
        let page_name = identifier(&page.name);
        let initial = if page.required {
            page_name.clone()
        } else {
            format!("{page_name} ?? 1")
        };
        let limit_expr = limit
            .map(|input| identifier(&input.name))
            .unwrap_or_else(|| "nil".into());
        let parameters = operation_parameters(operation);
        let signature = parameters
            .iter()
            .map(|parameter| parameter.signature.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let args = parameters
            .iter()
            .map(|parameter| {
                let name = parameter.signature.split(':').next().unwrap();
                format!(
                    "{name}: {}",
                    if name == page_name { "current" } else { name }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let response = swift_type(operation.success_schema().unwrap(), false);
        let selectors = plan
            .results
            .unwrap()
            .segments
            .iter()
            .map(|segment| match segment {
                SelectorSegment::Field(field) => {
                    format!(".field({})", swift_literal(field))
                }
                SelectorSegment::Index(index) => format!(".index({index})"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let name = function_name(&operation.id);
        writeln!(
            output,
            "    func {name}Pages({signature}) -> KajiPageSequence<{response}> {{\n        KajiPageSequence(page: {initial}, limit: {limit_expr}) {{ current in\n            let response = try await self.{name}({args})\n            return (response, try kajiPageCount(response, [{selectors}]))\n        }}\n    }}"
        )?;
    }
    Ok(output)
}

fn swift_literal(value: &str) -> String {
    let mut result = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            ch if ch.is_control() => {
                let _ = write!(result, "\\u{{{:x}}}", ch as u32);
            }
            ch => result.push(ch),
        }
    }
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires Swift toolchain"]
    fn native_page_sequence_is_lazy_and_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("Pagination.swift");
        std::fs::write(&runtime, include_str!("page_runtime.swift.txt")).unwrap();
        let main = temp.path().join("main.swift");
        std::fs::write(&main, r#"
import Foundation
actor Calls { var pages: [Int] = []; func record(_ page: Int) { pages.append(page) }; func values() -> [Int] { pages } }
@main struct Test {
 static func main() async throws {
  let calls = Calls()
  let sequence = KajiPageSequence<[Int]>(page: 0, limit: 2) { page in
   await calls.record(page); return (page == 0 ? [1,2] : [3], page == 0 ? 2 : 1)
  }
  let before = await calls.values(); precondition(before.isEmpty)
  var iterator = sequence.makeAsyncIterator()
  let first = try await iterator.next(); precondition(first == [1,2])
  let second = try await iterator.next(); precondition(second == [3])
  let end = try await iterator.next(); precondition(end == nil)
  let pages = await calls.values(); precondition(pages == [0,1])
  var invalid = KajiPageSequence<Int>(page: -1, limit: nil) { _ in fatalError("must not request") }.makeAsyncIterator()
  do { _ = try await invalid.next(); fatalError("invalid page") } catch KajiPaginationError.invalidPage {}
  var overflow = KajiPageSequence<Int>(page: Int.max, limit: nil) { _ in (1,1) }.makeAsyncIterator()
  _ = try await overflow.next()
  do { _ = try await overflow.next(); fatalError("overflow") } catch KajiPaginationError.pageOverflow {}
  let count = try kajiPageCount(["a/b": [[1],[2,3]]], [.field("a/b"), .index(-1)])
  precondition(count == 2)
  for key in ["01", "+1", "-1", ""] {
    do { _ = try kajiPageCount([1,2], [.field(key)]); fatalError("invalid pointer index") }
    catch KajiPaginationError.invalidResults {}
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
}

#[cfg(test)]
mod generation_tests {
    use super::*;
    #[test]
    #[ignore = "requires Swift toolchain"]
    fn declared_pages_preserve_required_and_optional_parameters() {
        use kaji_core::{HttpMethod, OperationParameter, OperationResponse};
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
        operation.annotations.insert("x-kaji-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page","in":"parameters"}],"outputs":{"results":"$"}}));
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
struct Driver: KajiTransport {
 func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
  precondition(request.value(forHTTPHeaderField: "page") == "0")
  return (Data("[]".utf8), HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!)
 }
}
@main struct Test {
 static func main() async throws {
  let client = KajiClient(options: .init(baseURL: URL(string: "https://example.com")!), transport: Driver())
  var iterator = client.pets.listPetsPages(page: 0).makeAsyncIterator()
  let first = try await iterator.next(); precondition(first == [])
  let end = try await iterator.next(); precondition(end == nil)
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
