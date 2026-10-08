use kaji_core::input::InputPlugin;
use kaji_input_arazzo::{ArazzoDocument, ArazzoInput, ArazzoModel, parse};
use serde_json::{Value, json};
const EXAMPLE: &str = include_str!("fixtures/workflows.yaml");
fn fixture() -> Value {
    serde_yaml_ng::from_str(EXAMPLE).unwrap()
}
#[test]
fn every_supported_version_keeps_native_variant() {
    for version in ["1.0.0", "1.0.1", "1.1.0"] {
        let mut value = fixture();
        value["arazzo"] = json!(version);
        let doc = parse(&value.to_string()).unwrap();
        assert_eq!(doc.source["arazzo"], version);
        match version {
            "1.1.0" => assert!(matches!(doc.model, ArazzoModel::V1_1(_))),
            _ => assert!(matches!(doc.model, ArazzoModel::V1_0(_))),
        }
    }
}
#[test]
fn yaml_and_json_preserve_workflow_order_and_summary() {
    let yaml = parse(EXAMPLE).unwrap();
    let json = parse(&fixture().to_string()).unwrap();
    assert_eq!(yaml.summary(), json.summary());
    assert_eq!(yaml.source, json.source);
    assert_eq!(json.summary().operations[0].name, "login");
}
#[test]
fn self_and_external_workflow_dependencies_are_diagnosed() {
    for (dependency, message) in [
        ("checkout", "itself"),
        (
            "$sourceDescriptions.remote.workflows.login",
            "requires source resolution",
        ),
    ] {
        let mut value = fixture();
        value["workflows"][1]["dependsOn"] = json!([dependency]);
        assert!(format!("{:#}", parse(&value.to_string()).unwrap_err()).contains(message));
    }
}
#[test]
fn diamond_dependencies_are_valid() {
    let mut value = fixture();
    value["workflows"].as_array_mut().unwrap().push(json!({"workflowId":"another","dependsOn":["login"],"steps":[{"stepId":"get","operationId":"getOrder"}]}));
    value["workflows"].as_array_mut().unwrap().push(json!({"workflowId":"finish","dependsOn":["checkout","another"],"steps":[{"stepId":"done","operationId":"done"}]}));
    assert_eq!(
        parse(&value.to_string())
            .unwrap()
            .summary()
            .operations
            .len(),
        4
    );
}
#[test]
fn step_and_workflow_reusable_actions_validate_target_kinds() {
    let mut value = fixture();
    value["components"] = json!({"successActions":{"finish":{"name":"finish","type":"end"}},"failureActions":{"stop":{"name":"stop","type":"end"}}});
    value["workflows"][0]["successActions"] =
        json!([{"reference":"$components.successActions.finish"}]);
    value["workflows"][0]["steps"][0]["onFailure"] =
        json!([{"reference":"$components.failureActions.stop"}]);
    parse(&value.to_string()).unwrap();
    value["workflows"][0]["steps"][0]["onFailure"][0]["reference"] =
        json!("$components.failureActions.missing");
    assert!(parse(&value.to_string()).is_err());
}
#[test]
fn arbitrary_payload_reference_fields_and_extensions_are_retained() {
    let mut value = fixture();
    value["x-team"] = json!("payments");
    value["workflows"][1]["steps"][0]["requestBody"] = json!({"contentType":"application/json","payload":{"reference":"ordinary business field","id":3}});
    value["workflows"][1]["steps"][0]["outputs"] = json!({"orderId":"$response.body#/id"});
    let doc = parse(&value.to_string()).unwrap();
    assert_eq!(doc.source, value);
    if let ArazzoModel::V1_0(model) = doc.model {
        assert_eq!(serde_json::to_value(model).unwrap()["x-team"], "payments");
    } else {
        panic!("wrong model")
    }
}
#[test]
fn duplicate_workflow_ids_empty_steps_and_missing_info_fail() {
    let mut value = fixture();
    value["workflows"][1]["workflowId"] = json!("login");
    assert!(parse(&value.to_string()).is_err());
    let mut value = fixture();
    value["workflows"][0]["steps"] = json!([]);
    assert!(parse(&value.to_string()).is_err());
    for source in ["{", "[]", "arazzo: 1.0.1", "arazzo: 1"] {
        assert!(parse(source).is_err());
    }
}
#[test]
fn provider_publishes_document_and_unresolved_source_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workflow.yaml");
    std::fs::write(&path, EXAMPLE).unwrap();
    let provider = ArazzoInput;
    assert_eq!(provider.id(), "arazzo.roas");
    assert_eq!(provider.format(), "arazzo");
    let loaded = provider.load(&path).unwrap();
    assert_eq!(
        loaded.get::<ArazzoDocument>().unwrap().summary(),
        loaded.summary
    );
    assert_eq!(loaded.diagnostics.len(), 1);
    assert_eq!(loaded.diagnostics[0].code, "unresolved-source");
    assert!(
        loaded.diagnostics[0]
            .message
            .contains("https://example.test/openapi.yaml")
    );
    assert!(
        format!(
            "{:#}",
            provider
                .load(&directory.path().join("missing.yaml"))
                .err()
                .unwrap()
        )
        .contains("cannot read contract")
    );
    std::fs::write(&path, "bad").unwrap();
    assert!(provider.load(&path).is_err());
    std::fs::write(&path, [0xff]).unwrap();
    assert!(provider.load(&path).is_err());
}
