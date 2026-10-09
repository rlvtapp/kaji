use poolster_core::input::{InputOptions, InputPlugin};
use poolster_input_arazzo::{
    ArazzoInput,
    contracts::{WorkflowOperations, WorkflowValue},
};
const WORKFLOW: &str = include_str!("fixtures/runner/workflows.yaml");
const OPENAPI: &str = include_str!("fixtures/runner/shop.openapi.yaml");
fn load(workflow: &str, schema: &str) -> anyhow::Result<WorkflowOperations> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("workflow.yaml");
    std::fs::write(&path, workflow)?;
    std::fs::write(temp.path().join("shop.yaml"), schema)?;
    let mut input = ArazzoInput.load_with_options(
        &path,
        &InputOptions {
            workflow_sources: [("shop".into(), "shop.yaml".into())].into(),
            ..Default::default()
        },
    )?;
    input.take()
}
#[test]
fn resolves_sources_and_preserves_presence_expressions_and_versions() {
    for version in ["1.0.0", "1.0.1", "1.1.0"] {
        let contract = load(
            &WORKFLOW.replace("arazzo: 1.0.1", &format!("arazzo: {version}")),
            OPENAPI,
        )
        .unwrap();
        assert_eq!(contract.workflows.len(), 2);
        assert_eq!(contract.workflows[1].steps[0].operation.source, "shop");
        assert_eq!(contract.workflows[1].steps[0].expected_statuses, [201]);
        assert!(
            !contract.workflows[1]
                .inputs
                .iter()
                .find(|v| v.name == "itemId")
                .unwrap()
                .optional
        );
        assert_eq!(
            contract.workflows[1]
                .inputs
                .iter()
                .find(|v| v.name == "quantity")
                .unwrap()
                .default_value
                .as_deref(),
            Some("1")
        );
        assert!(
            matches!(&contract.workflows[1].steps[0].parameters[2].value, WorkflowValue::WorkflowOutput {workflow, name} if workflow == "login" && name == "token")
        );
        assert!(contract.source_documents.contains_key("shop"));
    }
}
#[test]
fn resolves_explicit_operation_pointer_and_rejects_unsupported_semantics() {
    let pointer = WORKFLOW.replace(
        "operationId: $sourceDescriptions.shop.createOrder",
        "operationPath: '{$sourceDescriptions.shop.url}#/paths/~1orders~1{id}/post'",
    );
    assert!(load(&pointer, OPENAPI).is_ok());
    for changed in [
        WORKFLOW.replace("$inputs.itemId", "$inputs.missing"),
        WORKFLOW.replace("$statusCode == 201", "$statusCode > 199"),
        WORKFLOW.replace("$response.body#/id", "$response.header.X-ID"),
        WORKFLOW.replace("$inputs.itemId", "$steps.future.outputs.id"),
        WORKFLOW.replace(
            "operationId: $sourceDescriptions.shop.createOrder",
            "workflowId: login",
        ),
        WORKFLOW.replace("dependsOn: [login]", "dependsOn: [missing]"),
        WORKFLOW.replace("in: query", "in: cookie"),
        WORKFLOW.replace(
            "contentType: application/json",
            "contentType: multipart/form-data",
        ),
    ] {
        assert!(load(&changed, OPENAPI).is_err(), "accepted {changed}");
    }
    assert!(
        load(
            WORKFLOW,
            &OPENAPI.replace(
                "schema: {type: integer}",
                "style: form, schema: {type: integer}"
            )
        )
        .is_err()
    );
    assert!(
        load(
            WORKFLOW,
            &OPENAPI.replace("openapi: 3.1.0", "openapi: 3.1.malformed")
        )
        .is_err()
    );
}
#[test]
fn requires_mappings_and_rejects_duplicate_bare_operation_ids() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("workflow.yaml");
    std::fs::write(&path, WORKFLOW).unwrap();
    std::fs::write(temp.path().join("shop.yaml"), OPENAPI).unwrap();
    let options = InputOptions {
        workflow_sources: [("undeclared".into(), "shop.yaml".into())].into(),
        ..Default::default()
    };
    assert!(ArazzoInput.load_with_options(&path, &options).is_err());
    let ambiguous = WORKFLOW.replace("workflows:", "  - name: duplicate\n    url: https://example.invalid/other.yaml\n    type: openapi\nworkflows:");
    std::fs::write(&path, ambiguous).unwrap();
    let options = InputOptions {
        workflow_sources: [
            ("shop".into(), "shop.yaml".into()),
            ("duplicate".into(), "shop.yaml".into()),
        ]
        .into(),
        ..Default::default()
    };
    assert!(
        format!(
            "{:#}",
            ArazzoInput
                .load_with_options(&path, &options)
                .err()
                .unwrap()
        )
        .contains("ambiguous")
    );
}

#[test]
fn resolves_pinned_upstream_operation_and_rejects_full_unsupported_workflow() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("workflow.yaml");
    std::fs::write(&path, include_str!("fixtures/runner/authorize.yaml")).unwrap();
    std::fs::write(
        temp.path().join("oauth.yaml"),
        include_str!("corpus/oauth.openapi.yaml"),
    )
    .unwrap();
    let options = InputOptions {
        workflow_sources: [("oauth".into(), "oauth.yaml".into())].into(),
        ..Default::default()
    };
    let input = ArazzoInput.load_with_options(&path, &options).unwrap();
    let contract = input.get::<WorkflowOperations>().unwrap();
    assert_eq!(contract.workflows[0].steps[0].operation.path, "/authorize");
    std::fs::write(&path, include_str!("corpus/oauth-1.0.yaml")).unwrap();
    let options = InputOptions {
        workflow_sources: [("apim-auth".into(), "oauth.yaml".into())].into(),
        ..Default::default()
    };
    assert!(
        format!(
            "{:#}",
            ArazzoInput
                .load_with_options(&path, &options)
                .err()
                .unwrap()
        )
        .contains("workflowId")
    );
}

#[test]
fn optional_step_blocks_keep_stable_ids_and_control_context() {
    let mut contract = load(WORKFLOW, OPENAPI).unwrap();
    let blocks = poolster_input_arazzo::blocks::step_blocks(&contract, "checkout-source");
    assert_eq!(blocks.with_capability("poolster.workflow-step").count(), 2);
    let create = blocks
        .items
        .iter()
        .find(|block| block.value.step.id == "create")
        .unwrap();
    assert_eq!(create.metadata.id.local, "/workflows/checkout/steps/create");
    assert_eq!(create.value.workflow_dependencies, ["login"]);
    assert!(
        create
            .metadata
            .references
            .iter()
            .any(|reference| reference.id.local == "/workflows/login")
    );
    let mut first: Vec<_> = blocks
        .items
        .iter()
        .map(|block| block.metadata.id.clone())
        .collect();
    first.sort();
    contract.workflows.reverse();
    let mut reordered: Vec<_> = contract
        .step_blocks("checkout-source")
        .items
        .into_iter()
        .map(|block| block.metadata.id)
        .collect();
    reordered.sort();
    assert_eq!(first, reordered);
}

#[test]
fn authenticated_operations_and_reference_siblings_fail_explicitly() {
    let secured = OPENAPI.replace(
        "info: {title: Shop, version: 1.0.0}",
        "info: {title: Shop, version: 1.0.0}\nsecurity: [{bearer: []}]",
    );
    assert!(format!("{:#}", load(WORKFLOW, &secured).unwrap_err()).contains("authenticated"));
    let mut document: serde_json::Value = serde_yaml_ng::from_str(WORKFLOW).unwrap();
    document["components"] = serde_json::json!({"inputs":{"checkout":{"type":"object","properties": {"itemId":{"type":"string"}}}}});
    document["workflows"][1]["inputs"] =
        serde_json::json!({"$ref":"#/components/inputs/checkout","required":["itemId"]});
    assert!(
        format!("{:#}", load(&document.to_string(), OPENAPI).unwrap_err())
            .contains("$ref siblings")
    );
}

#[test]
fn registry_publishes_resolved_blocks_and_keeps_unresolved_inspection_separate() {
    use poolster_core::input::InputRegistry;
    use poolster_input_arazzo::blocks::WorkflowStepBlocks;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("workflow.yaml");
    std::fs::write(&path, WORKFLOW).unwrap();
    std::fs::write(temp.path().join("shop.yaml"), OPENAPI).unwrap();
    let mut registry = InputRegistry::new();
    registry.register(ArazzoInput).unwrap();
    let options = InputOptions {
        workflow_sources: [("shop".into(), "shop.yaml".into())].into(),
        ..Default::default()
    };
    let resolved = registry
        .load_with_options("arazzo", None, &path, &options)
        .unwrap();
    assert!(resolved.contract.get::<WorkflowOperations>().is_ok());
    assert!(
        resolved
            .contract
            .get::<poolster_input_arazzo::ArazzoDocument>()
            .is_ok()
    );
    let blocks = resolved.contract.get::<WorkflowStepBlocks>().unwrap();
    assert_eq!(blocks.items.len(), 2);
    blocks
        .require_parent(
            resolved
                .contract
                .get_reference::<WorkflowOperations>()
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    let mut ids: Vec<_> = blocks
        .items
        .iter()
        .map(|block| block.metadata.id.clone())
        .collect();
    ids.sort();
    assert!(
        ids.iter()
            .all(|id| id.source == std::fs::canonicalize(&path).unwrap().to_string_lossy())
    );
    let mut document: serde_json::Value = serde_yaml_ng::from_str(WORKFLOW).unwrap();
    document["workflows"].as_array_mut().unwrap().reverse();
    std::fs::write(&path, document.to_string()).unwrap();
    let reordered = registry
        .load_with_options("arazzo", None, &path, &options)
        .unwrap();
    let mut reordered_ids: Vec<_> = reordered
        .contract
        .get::<WorkflowStepBlocks>()
        .unwrap()
        .items
        .iter()
        .map(|block| block.metadata.id.clone())
        .collect();
    reordered_ids.sort();
    assert_eq!(ids, reordered_ids);
    let inspection = registry.load("arazzo", None, &path).unwrap();
    assert!(
        inspection
            .contract
            .get::<poolster_input_arazzo::ArazzoDocument>()
            .is_ok()
    );
    assert!(inspection.contract.get::<WorkflowOperations>().is_err());
    assert!(matches!(
        inspection
            .contract
            .get::<WorkflowStepBlocks>()
            .unwrap()
            .state,
        poolster_core::blocks::CollectionState::Unavailable { .. }
    ));
    assert!(!inspection.contract.diagnostics.is_empty());
    std::fs::remove_file(temp.path().join("shop.yaml")).unwrap();
    assert!(
        registry
            .load_with_options("arazzo", None, &path, &options)
            .is_err()
    );
}
