use super::recipes::combined_optional_packages;
use super::*;

#[test]
fn combined_optional_reference_smoke_and_data_source_consumers_generate() {
    use poolster_core::{
        AdditionalProperties, Field, HttpMethod, Operation, OperationParameter,
        OperationRequestBody, OperationResponse, SchemaKind, SchemaValue,
    };
    let string = || SchemaValue::new(SchemaKind::String);
    let object = |identity: bool| {
        SchemaValue::new(SchemaKind::Object {
            fields: if identity {
                vec![
                    Field {
                        name: "id".into(),
                        value: {
                            let mut value = string();
                            value.read_only = true;
                            value
                        },
                        required: true,
                        annotations: Default::default(),
                    },
                    Field {
                        name: "name".into(),
                        value: string(),
                        required: true,
                        annotations: Default::default(),
                    },
                ]
            } else {
                vec![Field {
                    name: "name".into(),
                    value: string(),
                    required: true,
                    annotations: Default::default(),
                }]
            },
            additional_properties: AdditionalProperties::Forbidden,
        })
    };
    let parameter = OperationParameter {
        name: "id".into(),
        location: "path".into(),
        required: true,
        schema: Some(string()),
        description: None,
        annotations: Default::default(),
    };
    let mut api = Api {
        name: "Combined".into(),
        version: "1.0.0".into(),
        ..Default::default()
    };
    api.operations = vec![
        Operation {
            id: "createItem".into(),
            method: HttpMethod::Post,
            path: "/items".into(),
            request_body: Some(OperationRequestBody::json(object(false), true)),
            responses: vec![OperationResponse::json("201", object(true))],
            ..Default::default()
        },
        Operation {
            id: "getItem".into(),
            method: HttpMethod::Get,
            path: "/items/{id}".into(),
            parameters: vec![parameter.clone()],
            responses: vec![OperationResponse::json("200", object(true))],
            ..Default::default()
        },
        Operation {
            id: "updateItem".into(),
            method: HttpMethod::Put,
            path: "/items/{id}".into(),
            parameters: vec![parameter.clone()],
            request_body: Some(OperationRequestBody::json(object(false), true)),
            responses: vec![OperationResponse::json("200", object(true))],
            ..Default::default()
        },
        Operation {
            id: "deleteItem".into(),
            method: HttpMethod::Delete,
            path: "/items/{id}".into(),
            parameters: vec![parameter],
            responses: vec![OperationResponse {
                status: "204".into(),
                description: None,
                media_types: vec![],
            }],
            ..Default::default()
        },
    ];
    let configured = combined_optional_packages();
    let packages: Vec<PackageConfig> = serde_json::from_value(configured.clone()).unwrap();
    let tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Namespaced, &packages).unwrap(),
    )
    .unwrap();
    for path in [
        "./python/API_REFERENCE.md",
        "./python/tests/test_operations.py",
        "./python/.poolster/operation-test-diagnostics.json",
        "./terraform/API_REFERENCE.md",
        "./terraform/internal/provider/data_source_item.go",
    ] {
        assert!(tree.get(path).is_some(), "missing {path}");
    }
    let mut release_recipe = configured.clone();
    release_recipe[1]["plugins"][0]["provider_name"] = serde_json::json!("widgets");
    release_recipe[1]["plugins"][0]["registry_namespace"] = serde_json::json!("acme");
    release_recipe[1]["plugins"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"name":"release-scaffold"}));
    let packages: Vec<PackageConfig> = serde_json::from_value(release_recipe).unwrap();
    let release_tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Namespaced, &packages).unwrap(),
    )
    .unwrap();
    assert!(
        release_tree
            .get("./terraform/main.go")
            .unwrap()
            .contains("registry.terraform.io/acme/widgets")
    );
    assert!(
        release_tree
            .get("./terraform/main.go")
            .unwrap()
            .contains("provider.New(version)")
    );
    assert!(
        release_tree
            .get("./terraform/.goreleaser.yml")
            .unwrap()
            .contains("project_name: terraform-provider-widgets")
    );
    assert!(
        release_tree.preserves_existing("./terraform/.poolster/templates/terraform-release.yml")
    );
    assert!(
        release_tree
            .get("./terraform/.github/workflows/terraform-release.yml")
            .is_none()
    );
    let mut polling_recipe = configured.clone();
    polling_recipe[1]["plugins"][0]["resources"][0]["polling"] = serde_json::json!({
        "create": {"success": [{"status":200}]},
        "delete": {"interval_ms":1,"max_attempts":3,"timeout_ms":1000,"success":[{"status":404}]}
    });
    let polling_packages: Vec<PackageConfig> = serde_json::from_value(polling_recipe).unwrap();
    assert!(
        polling_packages[1].plugins[0].resources[0]
            .polling
            .is_some()
    );
    let polling_tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Namespaced, &polling_packages).unwrap(),
    )
    .unwrap();
    let explanation: serde_json::Value = serde_json::from_str(
        polling_tree
            .get("./terraform/.poolster/terraform-plan.json")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        explanation["resources"][0]["polling"]["delete"]["max_attempts"],
        3
    );
    let mut migration_recipe = configured.clone();
    migration_recipe[1]["plugins"][0]["resources"][0]["schema_version"] = serde_json::json!(1);
    migration_recipe[1]["plugins"][0]["resources"][0]["state_upgrades"] =
        serde_json::json!([{"version":0,"rename_fields":{"old_name":"name"}}]);
    let migration_packages: Vec<PackageConfig> = serde_json::from_value(migration_recipe).unwrap();
    assert_eq!(
        migration_packages[1].plugins[0].resources[0].schema_version,
        1
    );
    let migration_tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Namespaced, &migration_packages).unwrap(),
    )
    .unwrap();
    assert!(
        migration_tree
            .get("./terraform/internal/provider/resource_item.go")
            .unwrap()
            .contains("UpgradeState")
    );
    let mut composite_api = api.clone();
    let parent = OperationParameter {
        name: "organization".into(),
        location: "path".into(),
        required: true,
        schema: Some(string()),
        description: None,
        annotations: Default::default(),
    };
    for operation in &mut composite_api.operations {
        operation.path = format!("/organizations/{{organization}}{}", operation.path);
        operation.parameters.push(parent.clone());
        for response in &mut operation.responses {
            for media in &mut response.media_types {
                if let Some(SchemaValue {
                    kind: SchemaKind::Object { fields, .. },
                    ..
                }) = &mut media.schema
                {
                    fields.push(Field {
                        name: "organizationId".into(),
                        value: {
                            let mut value = string();
                            value.read_only = true;
                            value
                        },
                        required: true,
                        annotations: Default::default(),
                    });
                }
            }
        }
    }
    let mut composite_recipe = configured.clone();
    let resource = &mut composite_recipe[1]["plugins"][0]["resources"][0];
    resource.as_object_mut().unwrap().remove("id_parameter");
    resource.as_object_mut().unwrap().remove("id_field");
    resource["identity"] = serde_json::json!([{"parameter":"organization","field":"organizationId"},{"parameter":"id","field":"id"}]);
    let composite_packages: Vec<PackageConfig> = serde_json::from_value(composite_recipe).unwrap();
    let composite_tree = poolster::generate(
        &composite_api,
        config_profiles(SdkClientStyle::Namespaced, &composite_packages).unwrap(),
    )
    .unwrap();
    assert!(
        composite_tree
            .get("./terraform/internal/provider/resource_item.go")
            .unwrap()
            .contains("ParseIdentity")
    );
    let mut defaults = configured;
    for package in defaults.as_array_mut().unwrap() {
        package.as_object_mut().unwrap().remove("api_reference");
    }
    defaults[0]["plugins"] = serde_json::json!([{"name":"sdk"}]);
    defaults[1]["plugins"][0]
        .as_object_mut()
        .unwrap()
        .remove("data_sources");
    let packages: Vec<PackageConfig> = serde_json::from_value(defaults).unwrap();
    let tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Flat, &packages).unwrap(),
    )
    .unwrap();
    for path in [
        "./python/API_REFERENCE.md",
        "./python/tests/test_operations.py",
        "./terraform/API_REFERENCE.md",
        "./terraform/internal/provider/data_source_item.go",
    ] {
        assert!(tree.get(path).is_none(), "unexpected default output {path}");
    }
}
