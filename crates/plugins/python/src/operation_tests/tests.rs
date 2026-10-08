use super::*;
use poolster_core::{
    AdditionalProperties, Field, HttpMethod, Operation, OperationParameter, OperationRequestBody,
    OperationResponse, Schema, engine::Packages,
};
fn fixture() -> Api {
    let string = || SchemaValue::new(SchemaKind::String);
    let object = SchemaValue::new(SchemaKind::Object {
        fields: vec![Field {
            name: "name".into(),
            value: string(),
            required: true,
            annotations: Default::default(),
        }],
        additional_properties: AdditionalProperties::Forbidden,
    });
    let mut api = Api {
        name: "Smoke".into(),
        schemas: vec![Schema::new("Contact", object.clone())],
        ..Default::default()
    };
    let mut read = Operation {
        id: "getContact".into(),
        method: HttpMethod::Get,
        path: "/contacts/{path}".into(),
        responses: vec![OperationResponse::json(
            "200",
            SchemaValue::reference("#/components/schemas/Contact"),
        )],
        ..Default::default()
    };
    for (name, location) in [("path", "path"), ("query", "query"), ("headers", "header")] {
        read.parameters.push(OperationParameter {
            name: name.into(),
            location: location.into(),
            required: true,
            schema: Some(string()),
            description: None,
            annotations: Default::default(),
        })
    }
    api.operations.push(read);
    api.operations.push(Operation {
        id: "createContact".into(),
        method: HttpMethod::Post,
        path: "/contacts".into(),
        request_body: Some(OperationRequestBody::json(object, true)),
        responses: vec![OperationResponse::json(
            "201",
            SchemaValue::reference("#/components/schemas/Contact"),
        )],
        ..Default::default()
    });
    api
}
#[test]
fn explicit_contract_and_unsupported_diagnostics_are_emitted() {
    let mut api = fixture();
    api.operations[0].responses[0].media_types[0].content_type = "text/event-stream".into();
    let sdk = crate::sdk();
    let tests = operation_tests().using_models(sdk.models());
    let tree = Packages::new()
        .package(crate::package("sdk").with(tests).with(sdk))
        .generate(&api, None)
        .unwrap();
    let fixture: Value =
        serde_json::from_str(tree.get("sdk/tests/operation-fixtures.json").unwrap()).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 1);
    assert!(
        tree.get("sdk/.poolster/operation-test-diagnostics.json")
            .unwrap()
            .contains("getContact")
    );
}
#[test]
#[ignore = "requires Python 3.10+; set POOLSTER_TEST_PYTHON"]
fn generated_smoke_tests_execute_flat_and_namespaced() {
    for flat in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let sdk = if flat {
            crate::sdk().flat()
        } else {
            crate::sdk().namespaced()
        };
        let tests = operation_tests().models_from(&sdk);
        Packages::new()
            .package(crate::package("sdk").with(sdk).with(tests))
            .generate(&fixture(), None)
            .unwrap()
            .write_to(root.path())
            .unwrap();
        let output = std::process::Command::new(
            std::env::var("POOLSTER_TEST_PYTHON")
                .or_else(|_| std::env::var("KAJI_TEST_PYTHON"))
                .unwrap_or_else(|_| "python3".into()),
        )
        .args(["-m", "unittest", "discover", "-v"])
        .current_dir(root.path().join("sdk"))
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
