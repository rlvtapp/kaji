use super::*;
use poolster_core::engine::Packages;
use poolster_core::{
    Field, HttpMethod, OperationMediaType, OperationParameter, OperationRequestBody,
    OperationResponse, Schema,
};
#[test]
fn emitted_public_operation_tests_execute_and_report_unsupported_sse() {
    let object = SchemaValue::new(SchemaKind::Object {
        fields: vec![Field {
            name: "id".into(),
            value: SchemaValue::new(SchemaKind::String),
            required: true,
            annotations: Default::default(),
        }],
        additional_properties: AdditionalProperties::Any,
    });
    let api = Api {
        name: "demo".into(),
        version: "1.0.0".into(),
        schemas: vec![Schema::new("Item", object)],
        operations: vec![
            Operation {
                id: "createItem".into(),
                method: HttpMethod::Post,
                path: "/items/{id}".into(),
                parameters: vec![OperationParameter {
                    name: "id".into(),
                    location: "path".into(),
                    required: true,
                    schema: Some({
                        let mut value = SchemaValue::new(SchemaKind::String);
                        value.enum_values = vec![json!("space /?&")];
                        value
                    }),
                    description: None,
                    annotations: Default::default(),
                }],
                request_body: Some(OperationRequestBody {
                    required: true,
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "application/json".into(),
                        schema: Some(SchemaValue::reference("#/components/schemas/Item")),
                    }],
                }),
                responses: vec![OperationResponse::json(
                    "201",
                    SchemaValue::reference("#/components/schemas/Item"),
                )],
                ..Default::default()
            },
            Operation {
                id: "deleteItem".into(),
                method: HttpMethod::Delete,
                path: "/items".into(),
                responses: vec![OperationResponse {
                    status: "204".into(),
                    description: None,
                    media_types: vec![],
                }],
                ..Default::default()
            },
            Operation {
                id: "events".into(),
                path: "/events".into(),
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "text/event-stream".into(),
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                    }],
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut api = api;
    if let SchemaKind::Object { fields, .. } = &mut api.schemas[0].value.kind {
        fields.push(Field {
            name: "weight".into(),
            value: SchemaValue::new(SchemaKind::Number),
            required: true,
            annotations: Default::default(),
        });
    }
    for (name, location, kind) in [
        ("enabled", "query", SchemaKind::Boolean),
        ("X-Ratio", "header", SchemaKind::Number),
    ] {
        api.operations[0].parameters.push(OperationParameter {
            name: name.into(),
            location: location.into(),
            required: false,
            schema: Some(SchemaValue::new(kind)),
            description: None,
            annotations: Default::default(),
        });
    }
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .with(crate::sdk())
                .with(operation_tests()),
        )
        .generate(&api, None)
        .unwrap();
    let diagnostics: Value = serde_json::from_str(
        tree.get("sdk/.poolster/operation-test-diagnostics.json")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(diagnostics["generated"], 2);
    assert!(
        diagnostics["skipped"]["events"]
            .as_str()
            .unwrap()
            .contains("SSE")
    );
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let output = std::process::Command::new("go")
        .args(["test", "-race", "./..."])
        .current_dir(root.path().join("sdk"))
        .env("GOCACHE", root.path().join("go-cache"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let bounded = Packages::new()
        .package(
            crate::package("sdk")
                .with(crate::sdk())
                .with(operation_tests().max_operations(1)),
        )
        .generate(&api, None)
        .unwrap();
    let diagnostics: Value = serde_json::from_str(
        bounded
            .get("sdk/.poolster/operation-test-diagnostics.json")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(diagnostics["generated"], 1);
    assert_eq!(
        diagnostics["skipped"]["deleteItem"],
        "operation count bound reached"
    );
}
