use super::*;
use poolster_core::{Api, HttpMethod, Operation, engine::Packages};
fn api() -> Api {
    Api {
        name: "Contacts API".into(),
        version: "1.0.0".into(),
        operations: vec![Operation {
            id: "listContacts".into(),
            method: HttpMethod::Get,
            path: "/contacts".into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}
#[test]
fn auxiliary_chunks_relocate_and_cleanup_when_output_shrinks() {
    let mut spec = api();
    spec.schemas = vec![
        poolster_core::Schema::new(
            "Leaf",
            poolster_core::SchemaValue::new(poolster_core::SchemaKind::String),
        ),
        poolster_core::Schema::new(
            "Branch",
            poolster_core::SchemaValue::reference("#/components/schemas/Leaf"),
        ),
    ];
    let build = |budget| {
        Packages::new()
            .package(
                crate::package("ts")
                    .with(models().output("domain/models"))
                    .with(transport())
                    .with(operations())
                    .with(zod().output("validation/schemas").max_file_bytes(budget))
                    .with(faker().output("fixtures/factories").max_file_bytes(budget))
                    .with(msw().output("fixtures/handlers").max_file_bytes(budget))
                    .with(cypress().output("tests/smoke").max_file_bytes(budget)),
            )
            .generate(&spec, None)
            .unwrap()
    };
    let directory =
        std::env::temp_dir().join(format!("poolster-ts-aux-shrink-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    build(1).write_to(&directory).unwrap();
    for chunk in [
        "validation/schemas_chunks/schemas_0001.ts",
        "fixtures/factories_chunks/chunk_0001.ts",
        "fixtures/handlers_chunks/chunk_0000.ts",
        "tests/smoke_chunks/chunk_0000.ts",
    ] {
        assert!(directory.join("ts").join(chunk).exists(), "missing {chunk}");
    }
    std::fs::write(
        directory.join("ts/custom.ts"),
        "export const custom = true;",
    )
    .unwrap();
    build(100_000).write_to(&directory).unwrap();
    assert!(
        !directory
            .join("ts/validation/schemas_chunks/schemas_0001.ts")
            .exists()
    );
    assert!(
        !directory
            .join("ts/tests/smoke_chunks/chunk_0000.ts")
            .exists()
    );
    assert!(directory.join("ts/custom.ts").exists());
    std::fs::remove_dir_all(directory).unwrap();
}
struct CustomTransport {
    meta: Meta,
}
impl Plugin<TypeScript> for CustomTransport {
    fn kind(&self) -> &'static str {
        "custom-transport"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Transport>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let config = sdk::SdkConfig::new("__package");
        let tree = sdk::generate_sdk(cx.api, &config, cx.security_schemes)?;
        cx.files.emit(GeneratedFile::new(
            "custom/request.ts",
            tree.get("__package/.poolster/client.ts").unwrap(),
        )?)?;
        cx.publish(Transport {
            module: "custom/request".into(),
            lossless_json: true,
        })
    }
}
#[test]
fn relocated_providers_and_custom_transport_compose() {
    let tree = Packages::new()
        .package(
            crate::package("ts")
                .with(models().output("domain/types"))
                .with(CustomTransport { meta: Meta::new() })
                .with(operations().output("api/calls"))
                .with(client().output("api/client"))
                .with(react_query().output("ui/queries")),
        )
        .generate(&api(), None)
        .unwrap();
    let call = tree.get("ts/api/calls/listContacts.ts").unwrap();
    assert!(call.contains("from '../../custom/request.js'"), "{call}");
    assert!(
        call.contains("from '../../domain/types/ListContacts.js'"),
        "{call}"
    );
    let client = tree.get("ts/api/client.ts").unwrap();
    assert!(client.contains("export class Contacts"), "{client}");
    assert!(
        client.contains("from './calls/listContacts.js'"),
        "{client}"
    );
    let query = tree.get("ts/ui/queries.ts").unwrap();
    assert!(
        query.contains("from \"../api/calls/listContacts.js\""),
        "{query}"
    );
    assert!(tree.get("ts/.poolster/client.ts").is_none());
    let manifest: serde_json::Value =
        serde_json::from_str(tree.get("ts/package.json").unwrap()).unwrap();
    assert_eq!(
        manifest["peerDependencies"]["@tanstack/react-query"],
        "^5.0.0"
    );
}
#[test]
fn unsupported_lossless_transport_fails_before_emitting_operations() {
    struct Unsupported {
        meta: Meta,
    }
    impl Plugin<TypeScript> for Unsupported {
        fn kind(&self) -> &'static str {
            "unsupported-runtime"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<Transport>()]
        }
        fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
            cx.publish(Transport::new("foreign/runtime"))
        }
    }
    let error = Packages::new()
        .package(
            crate::package("ts")
                .with(models().model_options(ModelOptions {
                    integer_as_string: true,
                    ..Default::default()
                }))
                .with(Unsupported { meta: Meta::new() })
                .with(operations()),
        )
        .generate(&api(), None)
        .unwrap_err();
    assert!(format!("{error:#}").contains("does not support schema-directed lossless JSON"));
}
#[test]
fn sdk_publishes_real_grouped_operation_symbols_for_consumers() {
    let sdk = crate::sdk().raw();
    let query = vue_query().using_operations(sdk.operations_handle());
    let tree = Packages::new()
        .package(crate::package("ts").with(query).with(sdk))
        .generate(&api(), None)
        .unwrap();
    assert!(
        tree.get("ts/vue-query.ts")
            .unwrap()
            .contains("from \"./clients/contacts/listContacts.js\"")
    );
}
#[test]
fn ambiguous_transport_requires_an_explicit_binding() {
    let first = transport().output("runtime/first");
    let handle = first.transport_handle();
    let tree = Packages::new()
        .package(
            crate::package("ts")
                .with(models())
                .with(first)
                .with(transport().output("runtime/second"))
                .with(operations().using_transport(handle)),
        )
        .generate(&api(), None)
        .unwrap();
    assert!(
        tree.get("ts/operations/listContacts.ts")
            .unwrap()
            .contains("../runtime/first")
    );
    let error = Packages::new()
        .package(
            crate::package("ts")
                .with(models())
                .with(transport().output("runtime/first"))
                .with(transport().output("runtime/second"))
                .with(operations()),
        )
        .generate(&api(), None)
        .unwrap_err();
    assert!(format!("{error:#}").contains("Select a provider handle explicitly"));
}
#[test]
fn invalid_query_partition_budget_fails_before_output() {
    let error = Packages::new()
        .package(
            crate::package("ts")
                .with(crate::sdk().raw())
                .with(react_query().max_operations_per_file(0)),
        )
        .generate(&api(), None)
        .unwrap_err();
    assert!(format!("{error:#}").contains("max_operations_per_file must be positive"));
}

#[test]
fn query_selection_names_classification_and_bounded_barrels() {
    let mut source = api();
    source.operations.push(Operation {
        id: "searchContacts".into(),
        method: HttpMethod::Post,
        path: "/search".into(),
        ..Operation::default()
    });
    let build = |query| {
        Packages::new()
            .package(crate::package("ts").with(crate::sdk().raw()).with(query))
            .generate(&source, None)
    };
    let tree = build(
        react_query()
            .include_operations(["searchContacts"])
            .operation_kind("searchContacts", QueryKind::Query)
            .operation_name("searchContacts", "findContacts")
            .layout(poolster_core::SourceLayout::PerOperation),
    )
    .unwrap();
    assert!(tree.iter().any(|(path, _)| {
        path.to_string_lossy()
            .contains("react-query_operations/searchContacts")
    }));
    let text = tree
        .iter()
        .filter(|(path, _)| path.to_string_lossy().contains("react-query"))
        .map(|(_, contents)| contents)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("findContactsQueryOptions"));
    assert!(!text.contains("listContactsQueryOptions"));
    assert!(text.contains("searchContacts.js"));
    assert!(build(react_query().include_operations(["missing"])).is_err());
    assert!(build(react_query().operation_name("searchContacts", "listContacts")).is_err());
    for index in 0..110 {
        source.operations.push(Operation {
            id: format!("read{index}"),
            method: HttpMethod::Get,
            path: format!("/items/{index}"),
            ..Operation::default()
        });
    }
    let tree = Packages::new()
        .package(
            crate::package("ts")
                .with(crate::sdk().raw())
                .with(react_query().layout(poolster_core::SourceLayout::PerOperation)),
        )
        .generate(&source, None)
        .unwrap();
    assert!(
        tree.iter()
            .any(|(path, _)| path.to_string_lossy().ends_with("index_0000.ts"))
    );
    let entry = tree.get("ts/react-query.ts").unwrap();
    assert!(entry.len() < 4096);
    assert_eq!(
        tree.iter()
            .filter(|(path, _)| path
                .to_string_lossy()
                .ends_with("react-query_chunks/runtime.ts"))
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
#[ignore = "requires POOLSTER_TSC_JS and POOLSTER_TS_NODE_MODULES with framework dependencies"]
fn query_factories_cache_callbacks_and_abort_execute_natively() {
    let root = tempfile::tempdir().unwrap();
    let mut source = api();
    source.operations.push(Operation {
        id: "createContact".into(),
        method: HttpMethod::Post,
        path: "/contacts".into(),
        ..Operation::default()
    });
    for (id, kind) in [
        ("cursorContacts", "cursor"),
        ("pageContacts", "page"),
        ("offsetContacts", "offsetLimit"),
        ("urlContacts", "url"),
    ] {
        let role = match kind {
            "page" => "page",
            "offsetLimit" => "offset",
            _ => "cursor",
        };
        let mut op = Operation {
            id: id.into(),
            method: HttpMethod::Get,
            path: format!("/{id}"),
            ..Operation::default()
        };
        for (name, schema) in [
            (
                role,
                if role == "cursor" {
                    poolster_core::SchemaKind::String
                } else {
                    poolster_core::SchemaKind::Integer
                },
            ),
            ("limit", poolster_core::SchemaKind::Integer),
        ] {
            op.parameters.push(poolster_core::OperationParameter {
                name: name.into(),
                location: "query".into(),
                schema: Some(poolster_core::SchemaValue::new(schema)),
                required: false,
                description: None,
                annotations: BTreeMap::new(),
            });
        }
        let inputs = if kind == "url" {
            serde_json::json!([])
        } else if kind == "cursor" {
            serde_json::json!([{ "name":role,"type":role }])
        } else {
            serde_json::json!([{ "name":role,"type":role }, {"name":"limit","type":"limit"}])
        };
        let outputs = match kind {
            "cursor" => serde_json::json!({"nextCursor":"$.next"}),
            "url" => serde_json::json!({"nextUrl":"$.next"}),
            _ => serde_json::json!({"results":"$.items"}),
        };
        op.annotations.insert(
            "x-poolster-pagination".into(),
            serde_json::json!({"type":kind,"inputs":inputs,"outputs":outputs}),
        );
        let mut next = poolster_core::SchemaValue::new(poolster_core::SchemaKind::String);
        next.nullable = true;
        op.responses.push(poolster_core::OperationResponse::json(
            "200",
            poolster_core::SchemaValue::new(poolster_core::SchemaKind::Object {
                fields: vec![
                    poolster_core::Field {
                        name: "next".into(),
                        value: next,
                        required: false,
                        annotations: BTreeMap::new(),
                    },
                    poolster_core::Field {
                        name: "items".into(),
                        value: poolster_core::SchemaValue::new(poolster_core::SchemaKind::Array {
                            items: Box::new(poolster_core::SchemaValue::new(
                                poolster_core::SchemaKind::Integer,
                            )),
                        }),
                        required: false,
                        annotations: BTreeMap::new(),
                    },
                ],
                additional_properties: poolster_core::AdditionalProperties::Unspecified,
            }),
        ));

        source.operations.push(op);
    }
    Packages::new()
        .package(
            crate::package("ts")
                .with(crate::sdk().raw())
                .with(
                    react_query()
                        .layout(poolster_core::SourceLayout::PerOperation)
                        .output("ui/react"),
                )
                .with(
                    vue_query()
                        .layout(poolster_core::SourceLayout::PerResource {
                            max_file_bytes: 128 * 1024,
                            max_declarations: Some(1),
                        })
                        .output("ui/vue"),
                )
                .with(swr().output("ui/swr")),
        )
        .generate(&source, None)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let package = root.path().join("ts");
    std::os::unix::fs::symlink(
        std::env::var_os("POOLSTER_TS_NODE_MODULES").unwrap(),
        package.join("node_modules"),
    )
    .unwrap();
    std::fs::write(
        package.join("query_probe.ts"),
        include_str!("../../tests/fixtures/query_probe.ts"),
    )
    .unwrap();
    std::fs::write(
        package.join("pagination_probe.ts"),
        include_str!("../../tests/fixtures/query_pagination_probe.ts"),
    )
    .unwrap();
    let compile = std::process::Command::new("node")
        .arg(std::env::var_os("POOLSTER_TSC_JS").unwrap())
        .args(["-p", "tsconfig.json"])
        .current_dir(&package)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = std::process::Command::new("node")
        .arg("dist/query_probe.js")
        .current_dir(&package)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pagination = std::process::Command::new("node")
        .arg("dist/pagination_probe.js")
        .current_dir(&package)
        .output()
        .unwrap();
    assert!(
        pagination.status.success(),
        "{}{}",
        String::from_utf8_lossy(&pagination.stdout),
        String::from_utf8_lossy(&pagination.stderr)
    );
}

#[test]
#[ignore = "requires POOLSTER_TSC_JS and POOLSTER_TS_NODE_MODULES"]
fn custom_transport_and_relocated_query_consumer_compile() {
    let compiler = std::env::var("POOLSTER_TSC_JS").unwrap();
    let modules = std::env::var("POOLSTER_TS_NODE_MODULES").unwrap();
    let directory =
        std::env::temp_dir().join(format!("poolster-ts-compose-{}", std::process::id()));
    let tree = Packages::new()
        .package(
            crate::package("ts")
                .with(models().output("domain/types"))
                .with(CustomTransport { meta: Meta::new() })
                .with(operations().output("api/calls"))
                .with(client().output("api/client"))
                .with(react_query().output("ui/queries")),
        )
        .generate(&api(), None)
        .unwrap();
    tree.write_to(&directory).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(modules, directory.join("ts/node_modules")).unwrap();
    std::fs::write(directory.join("ts/consumer.ts"), "import { Contacts } from './api/client'; import { useListContacts } from './ui/queries'; const result = new Contacts().listContacts({}); console.log(result, useListContacts);").unwrap();
    let result = std::process::Command::new("node")
        .arg(compiler)
        .args(["--noEmit", "--project"])
        .arg(directory.join("ts/tsconfig.json"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(directory);
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "requires POOLSTER_TSC_JS and POOLSTER_TS_NODE_MODULES with auxiliary dependencies"]
fn namespaced_and_auxiliary_consumers_compile_and_validate_selected_models() {
    let compiler = std::env::var("POOLSTER_TSC_JS").unwrap();
    let modules = std::env::var("POOLSTER_TS_NODE_MODULES").unwrap();
    let directory = std::env::temp_dir().join(format!("poolster-ts-aux-{}", std::process::id()));
    let mut api = api();
    let mut id = poolster_core::SchemaValue::new(poolster_core::SchemaKind::Integer);
    id.format = Some("int64".into());
    let mut status = poolster_core::SchemaValue::new(poolster_core::SchemaKind::String);
    status.enum_values = vec![serde_json::json!("active"), serde_json::json!("disabled")];
    api.schemas = vec![
        poolster_core::Schema::new("Status", status),
        poolster_core::Schema::new(
            "Contact",
            poolster_core::SchemaValue::new(poolster_core::SchemaKind::Object {
                fields: vec![
                    poolster_core::Field {
                        name: "id".into(),
                        value: id,
                        required: true,
                        annotations: Default::default(),
                    },
                    poolster_core::Field {
                        name: "status".into(),
                        value: poolster_core::SchemaValue::reference("#/components/schemas/Status"),
                        required: true,
                        annotations: Default::default(),
                    },
                ],
                additional_properties: Default::default(),
            }),
        ),
    ];
    api.operations[0].responses = vec![poolster_core::OperationResponse::json(
        "200",
        poolster_core::SchemaValue::reference("#/components/schemas/Contact"),
    )];
    api.schemas.push(poolster_core::Schema::new(
        "Node",
        poolster_core::SchemaValue::new(poolster_core::SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "child".into(),
                value: poolster_core::SchemaValue::reference("#/components/schemas/Node"),
                required: false,
                annotations: Default::default(),
            }],
            additional_properties: Default::default(),
        }),
    ));
    for (name, reference) in [("NodeA", "NodeB"), ("NodeB", "NodeA")] {
        api.schemas.push(poolster_core::Schema::new(
            name,
            poolster_core::SchemaValue::new(poolster_core::SchemaKind::Object {
                fields: vec![poolster_core::Field {
                    name: "child".into(),
                    value: poolster_core::SchemaValue::reference(format!(
                        "#/components/schemas/{reference}"
                    )),
                    required: false,
                    annotations: Default::default(),
                }],
                additional_properties: Default::default(),
            }),
        ));
    }
    for index in 0..24 {
        let mut operation = api.operations[0].clone();
        operation.id = format!("listContacts{index}");
        operation.path = format!("/contacts/{index}");
        api.operations.push(operation);
    }
    let tree = Packages::new()
        .package(
            crate::package("ts")
                .with(
                    models()
                        .output("domain/models")
                        .model_options(ModelOptions {
                            int64_type: crate::Int64Type::BigInt,
                            enum_type: crate::EnumType::AsConst,
                            ..Default::default()
                        }),
                )
                .with(transport().output("network/runtime"))
                .with(operations().output("api/calls"))
                .with(client().namespaced().output("facade/client"))
                .with(react_query().output("hooks/react"))
                .with(vue_query().output("hooks/vue"))
                .with(swr().output("hooks/swr"))
                .with(zod().output("validation/schemas").max_file_bytes(1100))
                .with(faker().output("fixtures/factories").max_file_bytes(1100))
                .with(msw().output("fixtures/handlers").max_file_bytes(1100))
                .with(cypress().output("tests/smoke").max_file_bytes(1100)),
        )
        .generate(&api, None)
        .unwrap();
    tree.write_to(&directory).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(modules, directory.join("ts/node_modules")).unwrap();
    std::fs::write(directory.join("ts/consumer.ts"),"import { Contacts } from './facade/client'; import { createContact } from './fixtures/factories'; import { ContactSchema } from './validation/schemas'; const contact = createContact(); ContactSchema.parse(contact); const id: bigint = contact.id; new Contacts().contacts.list({}); console.log(id);").unwrap();
    let result = std::process::Command::new("node")
        .arg(compiler)
        .args(["--project"])
        .arg(directory.join("ts/tsconfig.json"))
        .args([
            "--module",
            "commonjs",
            "--moduleResolution",
            "node",
            "--outDir",
            "compiled",
        ])
        .current_dir(directory.join("ts"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::write(
        directory.join("ts/compiled/package.json"),
        "{\"type\":\"commonjs\"}",
    )
    .unwrap();
    std::fs::write(directory.join("ts/test.cjs"),"const assert = require('node:assert/strict'); const {ContactSchema} = require('./compiled/validation/schemas.js'); const {createContact} = require('./compiled/fixtures/factories.js'); const contact = createContact(); assert.equal(typeof contact.id,'bigint'); assert.equal(ContactSchema.parse(contact).id,contact.id); assert.throws(()=>ContactSchema.parse({...contact,id:'1'})); const {NodeSchema,poolsterSchemas,poolsterOperationSchemas} = require('./compiled/validation/schemas.js'); NodeSchema.parse({child:{child:{}}}); NodeSchema.parse(require('./compiled/fixtures/factories.js').createNode()); assert.equal(poolsterSchemas.Node,NodeSchema); const validators = require('./compiled/validation/schemas.js'); validators.NodeASchema.parse({child:{child:{}}}); validators.NodeASchema.parse(require('./compiled/fixtures/factories.js').createNodeA()); assert.equal(Object.keys(poolsterOperationSchemas).length,25); const {handlers} = require('./compiled/fixtures/handlers.js'); assert.equal(handlers.length,25); const calls = []; global.Cypress={env:()=>undefined}; global.describe=(_name,body)=>body(); global.it=(_name,body)=>body(); global.cy={request:options=>{calls.push(options);return {its:()=>({should:()=>{}})}}}; require('./compiled/tests/smoke.js'); assert.equal(calls.length,25);").unwrap();
    let result = std::process::Command::new("node")
        .arg(directory.join("ts/test.cjs"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
