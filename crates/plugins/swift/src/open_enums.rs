use crate::*;

pub(crate) fn render(
    api: &Api,
    dir: &str,
    package: Option<&str>,
    style: SdkClientStyle,
    enabled: bool,
) -> Result<GeneratedTree> {
    let mut tree = render_sdk(api, dir, package, style)?;
    if !enabled {
        return Ok(tree);
    }
    let root = normalized_root(dir)?;
    let package = package
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}-sdk", kebab_case(&api.name)));
    let module = type_name(&package);
    for schema in &api.schemas {
        if matches!(schema.value.kind, SchemaKind::String) && !schema.value.enum_values.is_empty() {
            let name = type_name(&schema.name);
            let mut output = format!(
                "{NOTICE}\nimport Foundation\n\npublic struct {name}: RawRepresentable, Codable, Sendable, Hashable, CustomStringConvertible {{\n    public let rawValue: String\n    public init(rawValue: String) {{ self.rawValue = rawValue }}\n    public var description: String {{ rawValue }}\n    public init(from decoder: Decoder) throws {{ rawValue = try decoder.singleValueContainer().decode(String.self) }}\n    public func encode(to encoder: Encoder) throws {{ var container = encoder.singleValueContainer(); try container.encode(rawValue) }}\n"
            );
            let mut names =
                std::collections::BTreeSet::from(["rawValue".to_owned(), "description".to_owned()]);
            for (index, value) in schema.value.enum_values.iter().enumerate() {
                let raw = value.as_str().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Swift string enum {} contains a non-string value",
                        schema.name
                    )
                })?;
                let mut case = enum_case(raw, index);
                while !names.insert(case.trim_matches('`').to_owned()) {
                    case = format!("{}_", case.trim_matches('`'));
                }
                let _ = writeln!(
                    output,
                    "    public static let {case} = Self(rawValue: {raw:?})"
                );
            }
            output.push_str("}\n");
            let path = if root.is_empty() {
                format!("Sources/{module}/Models/{name}.swift")
            } else {
                format!("{root}/Sources/{module}/Models/{name}.swift")
            };
            tree.replace(GeneratedFile::new(path, output)?)?;
        }
    }
    let path = if root.is_empty() {
        "README.md".to_owned()
    } else {
        format!("{root}/README.md")
    };
    let readme = tree.get(&path).unwrap().to_owned()
        + "\n## Open string enums\n\nGeneration enabled `sdk().open_enums(true)`. Named string enums are `RawRepresentable` value structs: known static values retain their original wire strings, and `EnumName(rawValue: \"future-value\")` preserves unknown strings through Codable and parameter serialization. Inline string enums remain Swift `String`, preserving unknown values without generated cases. Other enum kinds are unchanged. The default generation mode retains closed named enum declarations.\n";
    tree.replace(GeneratedFile::new(path, readme)?)?;
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn api() -> Api {
        Api {
            name: "Enums".into(),
            schemas: vec![Schema {
                name: "State".into(),
                value: SchemaValue {
                    enum_values: vec![serde_json::json!("in-progress"), serde_json::json!("done")],
                    ..SchemaValue::new(SchemaKind::String)
                },
            }],
            operations: vec![Operation {
                id: "getState".into(),
                method: kaji_core::HttpMethod::Get,
                path: "/state".into(),
                parameters: vec![kaji_core::OperationParameter {
                    name: "state".into(),
                    location: "query".into(),
                    required: true,
                    schema: Some(SchemaValue::new(SchemaKind::Reference {
                        reference: "#/components/schemas/State".into(),
                    })),
                    description: None,
                    annotations: Default::default(),
                }],
                responses: vec![kaji_core::OperationResponse {
                    status: "200".into(),
                    media_types: vec![kaji_core::OperationMediaType {
                        content_type: "application/json".into(),
                        schema: Some(SchemaValue::new(SchemaKind::Reference {
                            reference: "#/components/schemas/State".into(),
                        })),
                    }],
                    description: None,
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    #[test]
    fn open_is_explicit_and_default_enum_abi_is_unchanged() {
        let closed = render(&api(), "sdk", Some("Enums"), SdkClientStyle::Flat, false).unwrap();
        assert!(
            closed
                .get("sdk/Sources/Enums/Models/State.swift")
                .unwrap()
                .contains("public enum State: String")
        );
        let open = render(&api(), "sdk", Some("Enums"), SdkClientStyle::Flat, true).unwrap();
        assert!(
            open.get("sdk/Sources/Enums/Models/State.swift")
                .unwrap()
                .contains("public struct State: RawRepresentable")
        );
    }
    #[test]
    #[ignore = "requires Swift6; executes unknown enum Codable and named query serialization"]
    fn native_unknown_enum_roundtrip_and_query_preserve_wire_strings() {
        let root = tempfile::tempdir().unwrap();
        render(&api(), "sdk", Some("Enums"), SdkClientStyle::Flat, true)
            .unwrap()
            .write_to(root.path())
            .unwrap();
        let main = root.path().join("main.swift");
        std::fs::write(&main,r#"
import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
struct Driver: KajiTransport {
 func execute(_ request:URLRequest) async throws -> (Data,URLResponse) {
  precondition(URLComponents(url:request.url!,resolvingAgainstBaseURL:false)!.queryItems!.first!.value=="future 雪")
  return (Data("\"future 雪\"".utf8),HTTPURLResponse(url:request.url!,statusCode:200,httpVersion:nil,headerFields:nil)!)
 }
}
@main struct Probe { static func main() async throws {
 let encoder=JSONEncoder();let decoder=JSONDecoder()
 precondition(State.inProgress.rawValue=="in-progress")
 let unknown=try decoder.decode(State.self,from:Data("\"future 雪\"".utf8))
 precondition(unknown.rawValue=="future 雪")
 let roundtrip=try decoder.decode(State.self,from:encoder.encode(unknown));precondition(roundtrip==unknown)
 let client=KajiClient(options:.init(baseURL:URL(string:"https://example.test")!),transport:Driver())
 let result=try await client.getState(state:unknown);precondition(result==unknown)
 do {_ = try decoder.decode(State.self,from:Data("123".utf8));fatalError("accepted number")}catch {}
}}
"#).unwrap();
        fn files(path: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for item in std::fs::read_dir(path).unwrap() {
                let p = item.unwrap().path();
                if p.is_dir() {
                    files(&p, out)
                } else if p.extension().is_some_and(|s| s == "swift") {
                    out.push(p)
                }
            }
        }
        let mut sources = vec![];
        files(&root.path().join("sdk/Sources"), &mut sources);
        let result = std::process::Command::new("swiftc")
            .args([
                "-parse-as-library",
                "-swift-version",
                "6",
                "-warnings-as-errors",
                "-module-cache-path",
            ])
            .arg(root.path().join("cache"))
            .args(sources)
            .arg(main)
            .arg("-o")
            .arg(root.path().join("probe"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result = std::process::Command::new(root.path().join("probe"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
