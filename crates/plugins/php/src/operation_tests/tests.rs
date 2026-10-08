
use super::*;
use poolster_core::engine::Packages;
use poolster_core::{
    AdditionalProperties, Field, HttpMethod, OperationParameter, OperationRequestBody,
    OperationResponse, Schema,
};
fn api() -> Api {
    let mut field = SchemaValue::new(SchemaKind::String);
    field.default = Some(json!("NEVER_COPY_SECRET"));
    field
        .extensions
        .insert("example".into(), json!("NEVER_COPY_SECRET"));
    let mut operation = Operation {
        id: "getContact".into(),
        method: HttpMethod::Get,
        path: "/contacts/{id}".into(),
        parameters: vec![
            OperationParameter {
                name: "id".into(),
                location: "path".into(),
                required: true,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: Default::default(),
            },
            OperationParameter {
                name: "enabled".into(),
                location: "query".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                description: None,
                annotations: Default::default(),
            },
        ],
        request_body: None,
        responses: vec![OperationResponse::json(
            "200",
            SchemaValue::reference("#/components/schemas/Contact"),
        )],
        security: vec![],
        annotations: Default::default(),
    };
    let schema = Schema::new(
        "Contact",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "id".into(),
                value: field,
                required: true,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    );
    let mut create = operation.clone();
    create.id = "createContact".into();
    create.method = HttpMethod::Post;
    create.path = "/contacts/{id}".into();
    create.request_body = Some(OperationRequestBody::json(
        SchemaValue::reference("#/components/schemas/Contact"),
        true,
    ));
    let mut unsupported = operation.clone();
    unsupported.id = "streamContact".into();
    unsupported.responses[0].media_types[0].content_type = "text/event-stream".into();
    operation
        .annotations
        .insert("example".into(), json!("NEVER_COPY_SECRET"));
    Api {
        name: "Fixture".into(),
        version: "0.1.0".into(),
        schemas: vec![schema],
        operations: vec![operation, create, unsupported],
        ..Default::default()
    }
}
fn generate() -> GeneratedTree {
    Packages::new()
        .package(
            crate::package("sdk")
                .name("acme/smoke-sdk")
                .with(crate::sdk())
                .with(operation_tests()),
        )
        .generate(&api(), None)
        .unwrap()
}
#[test]
fn emits_sanitized_public_operation_cases_and_explicit_diagnostics() {
    let tree = generate();
    let fixture = tree.get("sdk/tests/operation-fixtures.json").unwrap();
    assert!(!fixture.contains("NEVER_COPY_SECRET"));
    let value: Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(value["cases"].as_array().unwrap().len(), 2);
    assert_eq!(value["cases"][1]["body_model"], "Contact");
    assert_eq!(value["cases"][1]["first_optional"], 1);
    let diagnostics = tree
        .get("sdk/.poolster/operation-test-diagnostics.json")
        .unwrap();
    assert!(diagnostics.contains("streamContact"));
    assert!(diagnostics.contains("non-JSON/stream response unsupported"));
    assert!(
        tree.get("sdk/tests/operations.php")
            .unwrap()
            .contains("Smoke\\Sdk\\Client")
    );
}
#[test]
fn constrained_and_write_only_schemas_are_diagnostic_not_fabricated() {
    let api = api();
    let mut operation = api.operations[0].clone();
    operation.responses[0].media_types[0].schema = Some(SchemaValue::new(SchemaKind::OneOf {
        variants: vec![SchemaValue::new(SchemaKind::String)],
    }));
    assert!(case(&api, &operation).is_err());
    let mut secret = SchemaValue::new(SchemaKind::String);
    secret.write_only = true;
    assert!(sample(&api, &secret).is_err());
}
#[test]
#[ignore = "Requires PHP8.2+, Composer and dependency download; native fake transport only"]
fn generated_operation_tests_execute_native_public_calls_and_decode_failures() {
    let dir = tempfile::tempdir().unwrap();
    generate().write_to(dir.path()).unwrap();
    let root = dir.path().join("sdk");
    let setup = std::process::Command::new("composer")
        .args(["install", "--no-interaction", "--no-progress"])
        .current_dir(&root)
        .output()
        .expect("Required dependency manager unavailable");
    assert!(
        setup.status.success(),
        "{}{}",
        String::from_utf8_lossy(&setup.stdout),
        String::from_utf8_lossy(&setup.stderr)
    );
    let output = std::process::Command::new("php")
        .args(["tests/operations.php"])
        .current_dir(&root)
        .output()
        .expect("Required native toolchain unavailable");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
