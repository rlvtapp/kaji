use super::*;
use poolster_core::{HttpMethod, OperationParameter, OperationRequestBody, OperationResponse};

#[test]
fn large_operation_sets_keep_public_methods_in_bounded_extensions() {
    let api = Api {
        name: "Split".into(),
        operations: (0..201)
            .map(|index| Operation {
                id: format!("operation{index}"),
                method: HttpMethod::Get,
                path: format!("/items/{index}"),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let tree = render_sdk(&api, "swift", Some("SplitSdk"), SdkClientStyle::Flat).unwrap();
    let sources: Vec<_> = tree
        .into_files()
        .filter(|(file, _)| {
            file.path.ends_with("Operations.swift")
                || file.path.to_string_lossy().contains("PoolsterOperations")
        })
        .collect();
    assert_eq!(sources.len(), 3);
    for (source, _) in sources {
        assert!(source.contents.matches("async throws ->").count() <= 100);
    }
}

#[test]
#[ignore = "requires Swift; compiles and executes across resource chunk boundaries"]
fn native_split_resource_keeps_last_methods_and_transport() {
    let api = Api {
        name: "Split".into(),
        operations: (0..201)
            .map(|index| Operation {
                id: format!("operation{index}"),
                method: HttpMethod::Get,
                path: format!("/items/{index}"),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    let tree = render_sdk(&api, "sdk", Some("SplitSdk"), SdkClientStyle::Namespaced).unwrap();
    let files: Vec<_> = tree
        .iter()
        .filter(|(path, _)| {
            path.extension().is_some_and(|ext| ext == "swift")
                && path.file_name().is_some_and(|name| name != "Package.swift")
        })
        .map(|(path, _)| root.path().join(path))
        .collect();
    tree.write_to(root.path()).unwrap();
    std::fs::write(root.path().join("Probe.swift"), r#"import Foundation
struct Mock: PoolsterTransport {
func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
    precondition(["/items/100", "/items/200"].contains(request.url!.path))
    return (Data(), HTTPURLResponse(url: request.url!, statusCode: 204, httpVersion: nil, headerFields: nil)!)
}
}
@main struct Probe {
static func main() async throws {
    let client = PoolsterClient(options: .init(baseURL: URL(string: "https://unused.test")!), transport: Mock())
    try await client.items.operation100()
    try await client.items.operation200()
}
}
"#).unwrap();
    let output = std::process::Command::new("swiftc")
        .args(["-swift-version", "6", "-module-cache-path", "cache"])
        .args(files)
        .args(["Probe.swift", "-o", "probe"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(root.path().join("probe"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn oversized_queries_keep_public_arguments_and_bound_private_helpers() {
    let operation = Operation {
        id: "massive".into(),
        method: HttpMethod::Get,
        path: "/query".into(),
        parameters: (0..600)
            .map(|index| OperationParameter {
                name: format!("q{index}"),
                location: "query".into(),
                required: index == 2,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: Default::default(),
            })
            .collect(),
        ..Default::default()
    };
    let source = render_operation(&operation, "    ");
    assert_eq!(
        source
            .matches("private func __poolsterQuery_massive_")
            .count(),
        12
    );
    assert!(source.contains("q2: String,"));
    assert!(!source.contains("q2: String ="));
    assert!(source.contains("q599: String? = nil"));
    assert!(source.contains("URLQueryItem(name: \"q599\", value: String(describing: value))"));
    assert!(!source.contains(".map {"));
    assert_eq!(source.matches("self.sendVoid(request").count(), 1);
}

#[test]
#[ignore = "requires Swift; many-parameter typechecker and native query wire regression"]
fn native_large_query_operation_preserves_scalars_arrays_and_omission() {
    let mut parameters: Vec<OperationParameter> = (0..600)
        .map(|index| {
            let kind = match index % 4 {
                0 => SchemaKind::Boolean,
                1 => SchemaKind::Integer,
                2 => SchemaKind::String,
                _ => SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::String)),
                },
            };
            OperationParameter {
                name: format!("q{index}"),
                location: "query".into(),
                required: index == 2,
                schema: Some(SchemaValue::new(kind)),
                description: None,
                annotations: if index == 7 {
                    BTreeMap::from([("explode".into(), serde_json::json!(false))])
                } else {
                    BTreeMap::new()
                },
            }
        })
        .collect();
    let mut nullable_array = SchemaValue::new(SchemaKind::Array {
        items: Box::new(SchemaValue::new(SchemaKind::String)),
    });
    nullable_array.nullable = true;
    parameters.push(OperationParameter {
        name: "requiredNullable".into(),
        location: "query".into(),
        required: true,
        schema: Some(nullable_array),
        description: None,
        annotations: Default::default(),
    });
    let api = Api {
        operations: vec![Operation {
            id: "largeQuery".into(),
            method: HttpMethod::Get,
            path: "/query".into(),
            parameters,
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::Integer),
            )],
            ..Default::default()
        }],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Client.swift"), client_runtime()).unwrap();
    std::fs::write(
        root.path().join("Operations.swift"),
        render_operations(&api),
    )
    .unwrap();
    std::fs::write(
        root.path().join("Resource.swift"),
        render_resource(&api, "Query", &[&api.operations[0]], false),
    )
    .unwrap();
    std::fs::write(root.path().join("Probe.swift"), r#"import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
struct Mock: PoolsterTransport {
 func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
  let query = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!.queryItems!
  precondition(query.count == 7 || query.count == 9)
  precondition(query.filter { $0.name == "requiredNullable" }.map { $0.value! } == (query.count == 9 ? ["n1", "n2"] : []))
  precondition(query.filter { $0.name == "q0" }.map { $0.value! } == ["false"])
  precondition(query.filter { $0.name == "q1" }.map { $0.value! } == ["0"])
  precondition(query.filter { $0.name == "q2" }.map { $0.value! } == ["héllo 雪"])
  precondition(query.filter { $0.name == "q3" }.map { $0.value! } == ["a", "λ"])
  precondition(query.filter { $0.name == "q7" }.map { $0.value! } == ["c,d"])
  precondition(query.filter { $0.name == "q599" }.map { $0.value! } == ["tail"])
  return (Data("1".utf8), HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: [:])!)
 }
}
@main struct Probe {
 static func main() async throws {
  let client = PoolsterClient(options: .init(baseURL: URL(string: "https://unused.test")!), transport: Mock())
  let result = try await client.largeQuery(q0: false, q1: 0, q2: "héllo 雪", q3: ["a", "λ"], q7: ["c", "d"], q599: ["tail"], requiredNullable: ["n1", "n2"])
  precondition(result == 1)
  let omitted = try await client.query.largeQuery(q0: false, q1: 0, q2: "héllo 雪", q3: ["a", "λ"], q7: ["c", "d"], q599: ["tail"], requiredNullable: nil)
  precondition(omitted == 1)
 }
}
"#).unwrap();
    let output = std::process::Command::new("python3")
        .args(["-c", "import subprocess,sys; result=subprocess.run(sys.argv[1:],timeout=120); sys.exit(result.returncode)", "swiftc"])
        .args([
            "-g",
            "-swift-version",
            "6",
            "-warnings-as-errors",
            "-module-cache-path",
            "cache",
            "Client.swift",
            "Operations.swift",
            "Resource.swift",
            "Probe.swift",
            "-o",
            "probe",
        ])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(root.path().join("probe"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires a Swift toolchain"]
fn generated_idempotency_keys_preserve_caller_values() {
    let mut api = Api {
        operations: vec![Operation {
            id: "createItem".into(),
            method: HttpMethod::Post,
            path: "/items".into(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::Integer),
            )],
            annotations: BTreeMap::from([(
                "x-poolster-idempotency".into(),
                serde_json::json!({"header":"X-Once","auto_generate":true}),
            )]),
            ..Default::default()
        }],
        ..Default::default()
    };
    api.operations.push(Operation {
        id: "collide".into(),
        method: HttpMethod::Post,
        path: "/collisions".into(),
        parameters: vec![OperationParameter {
            name: "request2".into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        }],
        responses: vec![OperationResponse::json(
            "200",
            SchemaValue::new(SchemaKind::Integer),
        )],
        annotations: BTreeMap::from([(
            "x-poolster-idempotency".into(),
            serde_json::json!({"header":"Request","auto_generate":true}),
        )]),
        ..Default::default()
    });
    let api = poolster_core::idempotency::prepare_api(&api, &Default::default()).unwrap();
    let api = native_api(&api);
    let collision = &api.operations[1];
    assert_eq!(parameter_name(&collision.parameters[0]), "request2");
    assert_eq!(parameter_name(&collision.parameters[1]), "request3");
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Client.swift"), client_runtime()).unwrap();
    std::fs::write(
        root.path().join("Operations.swift"),
        render_operations(&api),
    )
    .unwrap();
    std::fs::write(root.path().join("Probe.swift"),r#"import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
actor Events {
 var collisions:[String]=[]
 func addCollision(_ key:String)->Int { collisions.append(key); return collisions.count }
 func collisionSnapshot()->[String]{collisions}
 var keys:[String]=[]
 func add(_ key:String){ keys.append(key) }
 func snapshot()->[String]{keys}
}
struct Mock:PoolsterTransport {
 let events:Events
 func execute(_ request:URLRequest) async throws -> (Data,URLResponse) {
  if request.url!.absoluteString.contains("/collisions") {
   let query = URLComponents(url:request.url!,resolvingAgainstBaseURL:false)!.queryItems!
   precondition(query.first { $0.name == "request2" }!.value == "query-only")
   let key = request.value(forHTTPHeaderField:"Request")!
   let count = await events.addCollision(key)
   return(Data("1".utf8),HTTPURLResponse(url:request.url!,statusCode:count % 2 == 1 ? 503 : 200,httpVersion:nil,headerFields:[:])!)
  }
  precondition(request.url!.absoluteString.contains("/prefix%2Fkeep/items"))
  await events.add(request.value(forHTTPHeaderField:"X-Once")!)
  return(Data("1".utf8),HTTPURLResponse(url:request.url!,statusCode:200,httpVersion:nil,headerFields:[:])!)
 }
}
@main struct Probe {
 static func main() async throws {
  let events=Events();let client=PoolsterClient(options:.init(baseURL:URL(string:"https://unused.test/prefix%2Fkeep/")!, maxAttempts:2, retryBaseDelay:0, retryMaxDelay:0),transport:Mock(events:events))
  _ = try await client.createItem();_ = try await client.createItem();_ = try await client.createItem(xOnce:"durable-key");_ = try await client.createItem(xOnce:"")
  _ = try await client.collide(request2:"query-only")
  _ = try await client.collide(request2:"query-only",request3:"caller-key")
  let collisionKeys=await events.collisionSnapshot(); precondition(collisionKeys.count==4)
  precondition(UUID(uuidString:collisionKeys[0]) != nil); precondition(collisionKeys[0]==collisionKeys[1])
  precondition(collisionKeys[2]=="caller-key" && collisionKeys[3]=="caller-key")
  let literal = try client.makeRequest(method: "GET", path: "/literal?x#y")
  precondition(literal.url!.absoluteString == "https://unused.test/prefix%2Fkeep/literal%3Fx%23y")
  let keys=await events.snapshot();precondition(keys.count==4);precondition(keys[0] != keys[1]);precondition(UUID(uuidString:keys[0]) != nil);precondition(keys[2]=="durable-key");precondition(keys[3]=="");precondition(keys[0].split(separator:"-")[2].first=="4")
 }
}
"#).unwrap();
    let output = std::process::Command::new("swiftc")
        .args([
            "-swift-version",
            "6",
            "-warnings-as-errors",
            "-module-cache-path",
            "cache",
            "Client.swift",
            "Operations.swift",
            "Probe.swift",
            "-o",
            "probe",
        ])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(root.path().join("probe"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires a Swift toolchain"]
fn swift_models_round_trip_unknown_null_and_wide_integer_values() {
    use std::process::Command;
    let mut nullable = SchemaValue::new(SchemaKind::String);
    nullable.nullable = true;
    let schema = Schema::new(
        "OpenModel",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![
                Field {
                    name: "id".into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "optional".into(),
                    value: nullable,
                    required: false,
                    annotations: Default::default(),
                },
            ],
            additional_properties: AdditionalProperties::Any,
        }),
    );
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("JSONValue.swift"), json_value()).unwrap();
    std::fs::write(root.path().join("OpenModel.swift"), render_model(&schema)).unwrap();
    let mut nullable_extra = SchemaValue::new(SchemaKind::String);
    nullable_extra.nullable = true;
    let typed = Schema::new(
        "TypedExtras",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![],
            additional_properties: AdditionalProperties::Schema {
                value: Box::new(nullable_extra),
            },
        }),
    );
    std::fs::write(root.path().join("TypedExtras.swift"), render_model(&typed)).unwrap();
    let script = r##"import Foundation
let inputs = [#"{"id":"one","optional":null,"future":{"nested":true},"wide":9007199254740993}"#, #"{"id":"two","extra":null}"#]
for input in inputs {
let data = input.data(using: .utf8)!
let model = try JSONDecoder().decode(OpenModel.self, from: data)
let encoded = try JSONEncoder().encode(model)
let before = try JSONSerialization.jsonObject(with: data) as! NSDictionary
let after = try JSONSerialization.jsonObject(with: encoded) as! NSDictionary
precondition(before == after)
if input.contains("wide") { precondition(String(data: encoded, encoding: .utf8)!.contains("9007199254740993")) }
}
let typedInput = #"{"nullable":null,"text":"hello"}"#.data(using: .utf8)!
let typed = try JSONDecoder().decode(TypedExtras.self, from: typedInput)
let typedEncoded = try JSONEncoder().encode(typed)
let typedBefore = try JSONSerialization.jsonObject(with: typedInput) as! NSDictionary
let typedAfter = try JSONSerialization.jsonObject(with: typedEncoded) as! NSDictionary
precondition(typedBefore == typedAfter)
let constructed = OpenModel(id: "three")
let value = try JSONSerialization.jsonObject(with: JSONEncoder().encode(constructed)) as! NSDictionary
precondition(value["optional"] == nil)
"##;
    let collisions = Schema::new(
        "ReservedModel",
        SchemaValue::new(SchemaKind::Object {
            fields: ["self", "self2", "encode", "encode2", "container"]
                .into_iter()
                .map(|name| Field {
                    name: name.into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: true,
                    annotations: Default::default(),
                })
                .collect(),
            additional_properties: AdditionalProperties::Forbidden,
        }),
    );
    std::fs::write(
        root.path().join("ReservedModel.swift"),
        render_model(&collisions),
    )
    .unwrap();
    let script = format!(
        "{script}\nlet reservedData = #\"{{\"self\":\"a\",\"self2\":\"b\",\"encode\":\"c\",\"encode2\":\"d\",\"container\":\"e\"}}\"#.data(using: .utf8)!\nlet reserved = try JSONDecoder().decode(ReservedModel.self, from: reservedData)\nlet reservedAfter = try JSONSerialization.jsonObject(with: JSONEncoder().encode(reserved)) as! NSDictionary\nlet reservedBefore = try JSONSerialization.jsonObject(with: reservedData) as! NSDictionary\nprecondition(reservedAfter == reservedBefore)\n"
    );
    std::fs::write(root.path().join("main.swift"), script).unwrap();
    let output = Command::new("swiftc")
        .args([
            "-module-cache-path",
            "cache",
            "JSONValue.swift",
            "OpenModel.swift",
            "TypedExtras.swift",
            "ReservedModel.swift",
            "main.swift",
            "-o",
            "probe",
        ])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(root.path().join("probe")).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn streaming_is_native_and_unstructured_multipart_uses_ordered_parts() {
    let mut api = Api::default();
    let mut operation = Operation {
        id: "events".into(),
        ..Operation::default()
    };
    operation.responses.push(OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![poolster_core::OperationMediaType {
            content_type: "text/event-stream; charset=utf-8".into(),
            schema: None,
        }],
    });
    api.operations.push(operation);
    assert!(render_sdk(&api, "swift", None, SdkClientStyle::Flat).is_ok());
    api.operations[0].responses.clear();
    api.operations[0].request_body = Some(poolster_core::OperationRequestBody {
        required: true,
        description: None,
        media_types: vec![poolster_core::OperationMediaType {
            content_type: "multipart/form-data".into(),
            schema: Some(SchemaValue::new(SchemaKind::String)),
        }],
    });
    let tree = render_sdk(&api, "swift", None, SdkClientStyle::Flat).unwrap();
    assert!(
        tree.iter()
            .any(|(_, source)| source.contains("public var parts: [PoolsterOrderedPart]"))
    );
}

#[test]
fn renders_a_swift_package_with_models_operations_and_resources() {
    let api = Api {
        name: "Pet Store".into(),
        version: "1.0.0".into(),
        schemas: vec![Schema::new(
            "Pet",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![Field {
                    name: "pet_id".into(),
                    value: SchemaValue::new(SchemaKind::Integer),
                    required: true,
                    annotations: BTreeMap::new(),
                }],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        )],
        operations: vec![Operation {
            id: "listPets".into(),
            method: HttpMethod::Get,
            path: "/pets".into(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::reference("#/components/schemas/Pet")),
                }),
            )],
            ..Operation::default()
        }],
        ..Api::default()
    };
    let tree = render_sdk(&api, "swift", None, SdkClientStyle::Namespaced).unwrap();
    assert!(
        tree.get("swift/Package.swift")
            .unwrap()
            .contains("swift-tools-version")
    );
    assert!(
        tree.get("swift/Sources/PetStoreSdk/Models/Pet.swift")
            .unwrap()
            .contains("petId")
    );
    assert!(
        tree.get("swift/Sources/PetStoreSdk/Operations.swift")
            .unwrap()
            .contains("listPets")
    );
    assert!(
        tree.get("swift/Sources/PetStoreSdk/Resources/PetsResource.swift")
            .is_some()
    );
}

#[test]
fn rejects_escaping_output() {
    assert!(render_sdk(&Api::default(), "../swift", None, SdkClientStyle::Flat).is_err());
}

#[test]
#[ignore = "requires a Swift toolchain"]
fn generated_package_builds_with_swiftpm() {
    use std::process::Command;

    let mut api = Api {
        name: "Example API".into(),
        version: "1.0.0".into(),
        operations: vec![
            Operation {
                id: "listItems".into(),
                method: HttpMethod::Get,
                path: "/items".into(),
                parameters: vec![OperationParameter {
                    name: "limit".into(),
                    location: "query".into(),
                    required: false,
                    schema: Some(SchemaValue::new(SchemaKind::Integer)),
                    description: None,
                    annotations: BTreeMap::new(),
                }],
                responses: vec![OperationResponse::json(
                    "200",
                    SchemaValue::new(SchemaKind::Array {
                        items: Box::new(SchemaValue::new(SchemaKind::String)),
                    }),
                )],
                ..Operation::default()
            },
            Operation {
                id: "createItem".into(),
                method: HttpMethod::Post,
                path: "/items".into(),
                request_body: Some(OperationRequestBody::json(
                    SchemaValue::new(SchemaKind::Object {
                        fields: Vec::new(),
                        additional_properties: AdditionalProperties::Any,
                    }),
                    true,
                )),
                ..Operation::default()
            },
        ],
        ..Api::default()
    };
    api.schemas = ["Operations", "MediaResource"]
        .into_iter()
        .map(|name| Schema::new(name, SchemaValue::new(SchemaKind::String)))
        .collect();
    let mut duplicate_enum = SchemaValue::new(SchemaKind::String);
    duplicate_enum.enum_values = vec![
        serde_json::json!("ok"),
        serde_json::json!("ok"),
        serde_json::json!("new"),
    ];
    api.schemas
        .push(Schema::new("RepeatedEnum", duplicate_enum));
    api.operations.push(Operation {
        id: "reservedPath".into(),
        method: HttpMethod::Get,
        path: "/session/{default}".into(),
        parameters: vec![OperationParameter {
            name: "default".into(),
            location: "path".into(),
            required: true,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        }],
        ..Default::default()
    });
    api.operations.push(Operation {
        id: "reservedClient".into(),
        method: HttpMethod::Get,
        path: "/media".into(),
        parameters: vec![OperationParameter {
            name: "client".into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        }],
        ..Default::default()
    });
    let directory = tempfile::tempdir().unwrap();
    render_sdk(&api, "sdk", None, SdkClientStyle::Namespaced)
        .unwrap()
        .write_to(directory.path())
        .unwrap();
    let output = Command::new("swift")
        .args(["build", "--disable-sandbox"])
        .env(
            "CLANG_MODULE_CACHE_PATH",
            directory.path().join("clang-cache"),
        )
        .env(
            "SWIFTPM_MODULECACHE_OVERRIDE",
            directory.path().join("swift-cache"),
        )
        .current_dir(directory.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
#[ignore = "requires a Swift toolchain; execute during native runtime verification"]
fn swift_transport_middleware_executes_without_network() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Client.swift"), client_runtime()).unwrap();
    std::fs::write(root.path().join("Probe.swift"), r#"import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
actor Events {
var values: [String] = []
func add(_ value: String) { values.append(value) }
func snapshot() -> [String] { values }
}
enum Failure: Error { case expected }
struct Terminal: PoolsterTransport {
let events: Events
func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
    precondition(request.value(forHTTPHeaderField: "X-Customer") == "yes")
    await events.add("terminal")
    throw Failure.expected
}
}
@main struct Probe {
static func main() async throws {
    let events = Events()
    let recovery = PoolsterMiddlewareTransport(inner: Terminal(events: events)) { request, next in
        var request = request
        request.setValue("yes", forHTTPHeaderField: "X-Customer")
        await events.add("inner:request")
        do { return try await next(request) } catch {
            await events.add("inner:error")
            return (Data("\"recovered\"".utf8), HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: [:])!)
        }
    }
    let outer = PoolsterMiddlewareTransport(inner: recovery) { request, next in
        await events.add("outer:request")
        let result = try await next(request)
        await events.add("outer:response")
        return (Data("\"transformed\"".utf8), result.1)
    }
    let client = PoolsterClient(options: .init(baseURL: URL(string: "https://unused.example")!), transport: outer)
    let request = try client.makeRequest(method: "GET", path: "/label")
    let value = try await client.send(request, as: String.self)
    precondition(value == "transformed")
    let order = await events.snapshot()
    precondition(order == ["outer:request", "inner:request", "terminal", "inner:error", "outer:response"])
    let shortcut = PoolsterMiddlewareTransport(inner: Terminal(events: events)) { request, _ in
        (Data("\"cached\"".utf8), HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: [:])!)
    }
    let cached = PoolsterClient(options: .init(baseURL: URL(string: "https://unused.example")!), transport: shortcut)
    let cachedValue = try await cached.send(request, as: String.self)
    precondition(cachedValue == "cached")
    let after = await events.snapshot(); precondition(after == order)
    let cancellation = PoolsterMiddlewareTransport(inner: Terminal(events: events)) { _, _ in
        throw CancellationError()
    }
    let cancelled = PoolsterClient(options: .init(baseURL: URL(string: "https://unused.example")!), transport: cancellation)
    do {
        let _: String = try await cancelled.send(request, as: String.self)
        preconditionFailure("cancellation swallowed")
    } catch is CancellationError { }
}
}
"#).unwrap();
    let output = std::process::Command::new("swiftc")
        .args([
            "-swift-version",
            "6",
            "-warnings-as-errors",
            "-module-cache-path",
            "cache",
            "Client.swift",
            "Probe.swift",
            "-o",
            "probe",
        ])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(root.path().join("probe"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
