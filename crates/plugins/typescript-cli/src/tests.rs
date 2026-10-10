use super::*;
use poolster_core::engine::Packages;
use poolster_core::{
    AdditionalProperties, Field, HttpMethod, Operation, OperationRequestBody, Schema, SchemaKind,
    SchemaValue, SecurityRequirement,
};
use std::collections::BTreeMap;

fn large_api() -> Api {
    Api {
        name: "Chunk probe".into(),
        version: "1.0.0".into(),
        operations: (0..101)
            .map(|index| Operation {
                id: format!("listItems{index}"),
                method: HttpMethod::Get,
                path: format!("/resource{index}"),
                ..Operation::default()
            })
            .collect(),
        ..Api::default()
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires POOLSTER_TSC_JS and POOLSTER_TS_CLI_NODE_MODULES"]
fn split_commands_compile_and_show_last_chunk_in_help() {
    let root = tempfile::tempdir().unwrap();
    Packages::new()
        .package(package("cli").with(cli().command_name("probe")))
        .generate(&large_api(), None)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let package = root.path().join("cli");
    std::os::unix::fs::symlink(
        std::env::var_os("POOLSTER_TS_CLI_NODE_MODULES").expect("set CLI dependency directory"),
        package.join("node_modules"),
    )
    .unwrap();
    let compile = std::process::Command::new("node")
        .arg(std::env::var_os("POOLSTER_TSC_JS").expect("set TypeScript compiler path"))
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
        .args(["dist/index.js", "--help"])
        .current_dir(&package)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.contains("resource0") && help.contains("resource100"),
        "{help}"
    );
}

#[test]
fn emits_auth_commands_and_openapi_operation_flags() {
    let api = Api {
        name: "Example API".into(),
        version: "1.0.0".into(),
        operations: vec![Operation {
            id: "getPet".into(),
            method: HttpMethod::Get,
            path: "/pets/{id}".into(),
            parameters: vec![poolster_core::OperationParameter {
                name: "id".into(),
                location: "path".into(),
                required: true,
                schema: None,
                description: Some("Pet id".into()),
                annotations: BTreeMap::new(),
            }],
            security: vec![SecurityRequirement {
                schemes: BTreeMap::from([("Bearer".into(), vec![])]),
            }],
            ..Operation::default()
        }],
        ..Api::default()
    };
    let tree = Packages::new()
        .package(package("cli").with(cli().command_name("example")))
        .generate(&api, None)
        .unwrap();
    let index = tree.get("cli/src/index.ts").unwrap();
    let pets = tree.get("cli/references/pets.md").unwrap();
    assert!(index.contains("auth.command('login')"));
    assert!(index.contains("import { extension } from './poolster.extension.js'"));
    assert!(index.contains("\"pets\",\n      \"get\""));
    assert!(index.contains("\"option\": \"id\""));
    assert!(
        tree.get("cli/src/runtime.ts")
            .unwrap()
            .contains("deviceLogin")
    );
    assert!(
        tree.get("cli/src/runtime.ts")
            .unwrap()
            .contains("export type CliExtension")
    );
    assert!(
        tree.get("cli/src/runtime.ts")
            .unwrap()
            .contains("defaultAuthenticate: () => Promise<void>")
    );
    assert!(
        tree.get("cli/src/runtime.ts")
            .unwrap()
            .contains("result !== 'handled'")
    );
    let runtime = tree.get("cli/src/runtime.ts").unwrap();
    assert!(runtime.contains("hydrateOperation"));
    assert!(runtime.contains("process.stdin.isTTY && process.stdout.isTTY"));
    assert!(runtime.contains("promptText(promptLabel(label, description), secret)"));
    assert!(runtime.contains("JSON.stringify({ error: { message, code: 'cli_error' } })"));
    assert!(pets.contains("## example pets get"));
    assert!(tree.preserves_existing("cli/src/poolster.extension.ts"));
}

#[test]
fn promotes_simple_request_bodies_to_grouped_cli_flags() {
    let request = SchemaValue::new(SchemaKind::Object {
        fields: vec![
            Field {
                name: "from".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: true,
                annotations: BTreeMap::new(),
            },
            Field {
                name: "to".into(),
                value: SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::String)),
                }),
                required: true,
                annotations: BTreeMap::new(),
            },
            Field {
                name: "html".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: false,
                annotations: BTreeMap::new(),
            },
        ],
        additional_properties: AdditionalProperties::Forbidden,
    });
    let api = Api {
        name: "Email".into(),
        version: "1.0.0".into(),
        schemas: vec![Schema::new("SendMessageRequest", request)],
        operations: vec![Operation {
            id: "sendMessage".into(),
            method: HttpMethod::Post,
            path: "/messages".into(),
            request_body: Some(OperationRequestBody::json(
                SchemaValue::reference("#/components/schemas/SendMessageRequest"),
                true,
            )),
            annotations: BTreeMap::from([(
                "x-poolster-cli".into(),
                json!({ "command": "messages send" }),
            )]),
            ..Operation::default()
        }],
        ..Api::default()
    };
    let tree = Packages::new()
        .package(package("cli").with(cli()))
        .generate(&api, None)
        .unwrap();
    let index = tree.get("cli/src/index.ts").unwrap();
    assert!(index.contains("\"messages\",\n      \"send\""));
    assert!(index.contains("\"option\": \"from\""));
    assert!(index.contains("\"option\": \"to\""));
    assert!(index.contains("\"option\": \"html\""));
    assert!(index.contains("\"file\": true"));
}

#[test]
fn groups_known_operation_verbs_by_resource_path_without_extensions() {
    let operation = Operation {
        id: "sendMessage".into(),
        method: HttpMethod::Post,
        path: "/messages".into(),
        ..Operation::default()
    };
    assert_eq!(command_parts(&operation), ["messages", "send"]);
}

#[test]
fn derives_nested_namespaces_from_literal_path_segments() {
    let operation = Operation {
        id: "getUser".into(),
        method: HttpMethod::Get,
        path: "/admin/users/{userId}".into(),
        ..Operation::default()
    };
    assert_eq!(command_parts(&operation), ["admin", "users", "get"]);
}
