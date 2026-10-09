use super::*;
use crate::PackageExt;
use poolster_core::{
    AdditionalProperties, Field, HttpMethod, OperationMediaType, OperationParameter,
    OperationRequestBody, OperationResponse, Schema, engine::Packages,
};
fn api() -> Api {
    let object = SchemaValue::new(SchemaKind::Object {
        fields: vec![Field {
            name: "id".into(),
            value: SchemaValue::new(SchemaKind::String),
            required: true,
            annotations: Default::default(),
        }],
        additional_properties: AdditionalProperties::Forbidden,
    });
    let mut operation = Operation {
        id: "createWidget".into(),
        method: HttpMethod::Post,
        path: "/widgets/{id}".into(),
        responses: vec![OperationResponse::json(
            "200",
            SchemaValue::reference("#/components/schemas/Widget"),
        )],
        request_body: Some(OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(SchemaValue::reference("#/components/schemas/Widget")),
            }],
        }),
        ..Default::default()
    };
    for (name, location, kind) in [
        ("id", "path", SchemaKind::String),
        ("count", "query", SchemaKind::Integer),
        ("X-Flag", "header", SchemaKind::Boolean),
    ] {
        operation.parameters.push(OperationParameter {
            name: name.into(),
            location: location.into(),
            required: true,
            schema: Some(SchemaValue::new(kind)),
            description: None,
            annotations: Default::default(),
        });
    }
    Api {
        name: "Smoke".into(),
        version: "1.0.0".into(),
        schemas: vec![Schema::new("Widget", object)],
        operations: vec![
            operation,
            Operation {
                id: "deleteWidget".into(),
                method: HttpMethod::Delete,
                path: "/widgets".into(),
                responses: vec![OperationResponse {
                    status: "204".into(),
                    description: None,
                    media_types: vec![],
                }],
                ..Default::default()
            },
            Operation {
                id: "events".into(),
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "text/event-stream".into(),
                        schema: None,
                    }],
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}
#[test]
fn bounds_and_unsupported_media_are_explicit() {
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("smoke-sdk")
                .with(crate::sdk())
                .with(operation_tests().max_operations(1)),
        )
        .generate(&api(), None)
        .unwrap();
    let report: Value = serde_json::from_str(
        tree.get("sdk/.poolster/operation-test-diagnostics.json")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report["generated"], 1);
    assert_eq!(report["skipped"].as_object().unwrap().len(), 2);
    assert!(
        tree.get("sdk/src/operation_tests.rs")
            .unwrap()
            .contains("#[ignore=")
    );
    assert!(
        tree.get("sdk/Cargo.toml")
            .unwrap()
            .contains("[dev-dependencies]")
    );
}
#[test]
#[ignore = "requires Cargo and generated Reqwest dependencies (offline cache supported)"]
fn native_generated_rust_operation_tests_execute() {
    let temp = tempfile::tempdir().unwrap();
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("smoke-sdk")
                .with(crate::sdk().operation_prefix("wire_"))
                .with(operation_tests()),
        )
        .generate(&api(), None)
        .unwrap();
    tree.write_to(temp.path()).unwrap();
    let mut command = crate::native_cargo();
    command.args(["test", "--lib"]);
    command.current_dir(temp.path().join("sdk"));
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("2 passed"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 ignored"));
}
