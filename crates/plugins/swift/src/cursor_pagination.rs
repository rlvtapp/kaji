use super::*;
use poolster_core::pagination::{
    PaginationKind, PaginationValueKind, SelectorSegment, normalize_pagination,
};

pub(super) fn render(api: &Api) -> Result<String> {
    let mut output = String::new();
    for operation in &api.operations {
        let extension = operation
            .annotations
            .get("x-kaji-pagination")
            .or_else(|| operation.annotations.get("x-speakeasy-pagination"));
        if extension
            .and_then(|value| value.get("type"))
            .and_then(Value::as_str)
            != Some("cursor")
        {
            continue;
        }
        let Some(plan) = normalize_pagination(api, operation, None)? else {
            continue;
        };
        if plan.kind != PaginationKind::Cursor {
            continue;
        }
        let cursor = plan
            .inputs
            .iter()
            .find(|input| input.role == "cursor")
            .expect("validated cursor");
        if cursor.location == "requestBody" || cursor.value_kind != PaginationValueKind::String {
            bail!(
                "Swift cursor pagination for {} requires a string parameter cursor; body/integer controls are unsupported",
                operation.id
            );
        }
        let parameter = operation
            .parameters
            .iter()
            .find(|parameter| {
                parameter.name == cursor.name && parameter.location == cursor.location
            })
            .expect("validated cursor parameter");
        let mut schema = parameter.schema.as_ref().expect("validated schema");
        for _ in 0..128 {
            if !schema.enum_values.is_empty()
                || schema.const_value.is_some()
                || (cursor.required && (schema.nullable || schema.optional || schema.nullish))
            {
                bail!(
                    "Swift cursor pagination for {} requires an unconstrained string control",
                    operation.id
                );
            }
            match &schema.kind {
                SchemaKind::Reference { reference } => {
                    schema = &api
                        .schemas
                        .iter()
                        .find(|schema| schema.name == reference.rsplit('/').next().unwrap())
                        .expect("resolved schema")
                        .value;
                }
                SchemaKind::String => break,
                _ => bail!(
                    "Swift cursor pagination for {} requires a string control",
                    operation.id
                ),
            }
        }
        let parameters = operation_parameters(operation);
        if parameters
            .iter()
            .any(|parameter| parameter.signature.split(':').next() == Some("kajiCursorValue"))
        {
            bail!(
                "Swift cursor pagination control name collision for {}",
                operation.id
            )
        }
        let cursor_name = identifier(&cursor.name);
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
                    if name == cursor_name {
                        "kajiCursorValue"
                    } else {
                        name
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let guard = if cursor.required {
            "            guard let kajiCursorValue else { throw PoolsterPaginationError.invalidResults }\n"
        } else {
            ""
        };
        let response = swift_type(
            operation.success_schema().expect("validated response"),
            false,
        );
        let selector = plan
            .continuation
            .expect("validated continuation")
            .segments
            .iter()
            .map(|segment| match segment {
                SelectorSegment::Field(field) => {
                    format!(".field({})", super::pagination::swift_literal(field))
                }
                SelectorSegment::Index(index) => format!(".index({index})"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let name = function_name(&operation.id);
        writeln!(
            output,
            "    func {name}Pages({signature}) -> PoolsterCursorSequence<{response}> {{\n        PoolsterCursorSequence(cursor: {cursor_name}) {{ kajiCursorValue in\n{guard}            let response = try await self.{name}({args})\n            return (response, try kajiNextCursor(response, [{selector}]))\n        }}\n    }}"
        )?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn api() -> Api {
        let mut operation = Operation {
            id: "listItems".into(),
            method: poolster_core::HttpMethod::Get,
            path: "/items".into(),
            ..Default::default()
        };
        operation.parameters = vec![
            poolster_core::OperationParameter {
                name: "cursor".into(),
                location: "query".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: Default::default(),
            },
            poolster_core::OperationParameter {
                name: "X-Label".into(),
                location: "header".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: Default::default(),
            },
        ];
        let mut next = SchemaValue::new(SchemaKind::String);
        next.nullable = true;
        operation.responses = vec![poolster_core::OperationResponse::json(
            "200",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![Field {
                    name: "next".into(),
                    value: next,
                    required: false,
                    annotations: Default::default(),
                }],
                additional_properties: Default::default(),
            }),
        )];
        operation.annotations.insert("x-kaji-pagination".into(),serde_json::json!({"type":"cursor","inputs":[{"name":"cursor","type":"cursor"}],"outputs":{"nextCursor":"/next"}}));
        Api {
            name: "Cursor".into(),
            version: "1.0.0".into(),
            operations: vec![operation],
            ..Default::default()
        }
    }
    #[test]
    fn string_cursor_preserves_operation_arguments_and_rejects_body_integer() {
        let mut api = api();
        let output = render(&api).unwrap();
        assert!(output.contains("PoolsterCursorSequence(cursor: cursor)"));
        assert!(output.contains("listItems(cursor: kajiCursorValue, xLabel: xLabel)"));
        api.operations[0].parameters[0].schema = Some(SchemaValue::new(SchemaKind::Integer));
        assert!(render(&api).is_err());
    }
    #[test]
    #[ignore = "requires Swift toolchain; generated cursor operations plus real middleware"]
    fn native_cursor_sequence_preserves_transport_is_lazy_and_stops_cycles() {
        let tree = render_sdk(&api(), "sdk", Some("Cursor"), SdkClientStyle::Namespaced).unwrap();
        let root = tempfile::tempdir().unwrap();
        tree.write_to(root.path()).unwrap();
        let main = root.path().join("main.swift");
        std::fs::write(&main,r#"
import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
actor Calls {var cursors:[String?]=[];func record(_ value:String?){cursors.append(value)};func snapshot()->[String?]{cursors}}
struct Driver:PoolsterTransport {
 let calls:Calls
 func execute(_ request:URLRequest) async throws ->(Data,URLResponse){
  let cursor=URLComponents(url:request.url!,resolvingAgainstBaseURL:false)!.queryItems?.first(where:{$0.name=="cursor"})?.value
  await calls.record(cursor)
  precondition(request.value(forHTTPHeaderField:"X-Label")=="caller")
  precondition(request.value(forHTTPHeaderField:"X-Policy")=="enabled")
  let body=cursor==nil ? "{\"next\":\"a\"}" : "{\"next\":\"a\"}"
  return(Data(body.utf8),HTTPURLResponse(url:request.url!,statusCode:200,httpVersion:nil,headerFields:nil)!)
 }
}
@main struct Test {
 static func main() async throws {
  let calls=Calls()
  let transport=PoolsterMiddlewareTransport(inner:Driver(calls:calls)){request,next in
   var modified=request;modified.setValue("enabled",forHTTPHeaderField:"X-Policy");return try await next(modified)
  }
  let client=PoolsterClient(options:.init(baseURL:URL(string:"https://unused.example")!),transport:transport)
  var iterator=client.items.listItemsPages(xLabel:"caller").makeAsyncIterator()
  let before=await calls.snapshot();precondition(before.isEmpty)
  let first=try await iterator.next();precondition(first != nil)
  let second=try await iterator.next();precondition(second != nil)
  let end=try await iterator.next();precondition(end==nil)
  let seen=await calls.snapshot();precondition(seen.count==2 && seen[0]==nil && seen[1]=="a")
  var explicit=client.listItemsPages(cursor:"a",xLabel:"caller").makeAsyncIterator()
  _=try await explicit.next();let repeated=try await explicit.next();precondition(repeated==nil)
  var null=PoolsterCursorSequence<Int>(cursor:nil){_ in (1,nil)}.makeAsyncIterator()
  _=try await null.next();let finished=try await null.next();precondition(finished==nil)
 }
}
"#).unwrap();
        let mut command = std::process::Command::new("swiftc");
        command
            .args([
                "-parse-as-library",
                "-swift-version",
                "6",
                "-warnings-as-errors",
                "-module-cache-path",
            ])
            .arg(root.path().join("cache"));
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
        command
            .args(files)
            .arg(main)
            .arg("-o")
            .arg(root.path().join("probe"));
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            std::process::Command::new(root.path().join("probe"))
                .status()
                .unwrap()
                .success()
        );
    }
}
