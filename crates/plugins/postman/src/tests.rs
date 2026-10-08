use super::*;
use poolster_core::engine::Packages;
use poolster_core::{
    Field, HttpMethod, Operation, OperationMediaType, OperationParameter, OperationRequestBody,
    OperationResponse, Schema, SecurityRequirement, SecurityScheme, SecuritySchemeCatalog,
    SecuritySchemeKind,
};
fn api() -> Api {
    let mut password = SchemaValue::new(SchemaKind::String);
    password.write_only = true;
    let schema = SchemaValue::new(SchemaKind::Object {
        fields: vec![
            Field {
                name: "name".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: true,
                annotations: Default::default(),
            },
            Field {
                name: "password".into(),
                value: password,
                required: true,
                annotations: Default::default(),
            },
        ],
        additional_properties: Default::default(),
    });
    Api {
        name: "Pets".into(),
        version: "1.0.0".into(),
        schemas: vec![Schema::new("Pet", schema)],
        operations: vec![Operation {
            id: "createPet".into(),
            method: HttpMethod::Post,
            path: "/pets/{id}".into(),
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
                    name: "tags".into(),
                    location: "query".into(),
                    required: false,
                    schema: Some(SchemaValue::new(SchemaKind::Array {
                        items: Box::new(SchemaValue::new(SchemaKind::String)),
                    })),
                    description: None,
                    annotations: BTreeMap::from([("example".into(), json!(["cat", "dog"]))]),
                },
            ],
            request_body: Some(OperationRequestBody::json(
                SchemaValue::reference("#/components/schemas/Pet"),
                true,
            )),
            responses: vec![
                OperationResponse::json("201", SchemaValue::reference("#/components/schemas/Pet")),
                OperationResponse::json("400", SchemaValue::new(SchemaKind::String)),
            ],
            security: vec![],
            annotations: BTreeMap::from([
                ("tags".into(), json!(["Pets"])),
                (
                    "poolster.openapi.servers".into(),
                    json!([{"url":"https://api.example.com/{version}","variables":[{"name":"version","default":"v1"}]}]),
                ),
                (
                    "poolster.docs.request_examples".into(),
                    json!([{"content_type":"application/json","example_json":"{\"name\":\"cat\",\"password\":\"do-not-export\",\"access_token\":\"live-token\"}"}]),
                ),
            ]),
        }],
        annotations: Default::default(),
    }
}
fn generate(api: &Api, catalog: Option<&SecuritySchemeCatalog>) -> poolster_core::GeneratedTree {
    Packages::new()
        .package(package("postman").with(collection()).with(environment()))
        .generate(api, catalog)
        .unwrap()
}
fn document(tree: &poolster_core::GeneratedTree) -> Value {
    serde_json::from_str(tree.get("postman/collection.json").unwrap()).unwrap()
}
#[test]
fn mappings_are_deterministic_scrubbed_and_optional_inputs_disabled() {
    let tree = generate(&api(), None);
    assert_eq!(tree, generate(&api(), None));
    let doc = document(&tree);
    let item = &doc["item"][0]["item"][0];
    assert_eq!(item["request"]["method"], "POST");
    assert_eq!(item["request"]["url"]["path"], json!(["pets", ":id"]));
    assert_eq!(item["request"]["url"]["query"].as_array().unwrap().len(), 2);
    assert_eq!(item["request"]["url"]["query"][0]["disabled"], true);
    assert_eq!(item["response"].as_array().unwrap().len(), 2);
    let output = tree.get("postman/collection.json").unwrap();
    assert!(!output.contains("do-not-export"));
    assert!(!output.contains("live-token"));
    assert!(output.contains("<redacted>"));
    assert!(output.contains("https://api.example.com/v1"));
    let env: Value = serde_json::from_str(tree.get("postman/environment.json").unwrap()).unwrap();
    assert!(
        env["values"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["value"] == "")
    );
}
#[test]
fn environment_is_create_once_and_collection_updates_owned() {
    let tree = generate(&api(), None);
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let env = root.path().join("postman/environment.json");
    std::fs::write(&env, "handwritten credential environment").unwrap();
    tree.write_to(root.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(env).unwrap(),
        "handwritten credential environment"
    );
}
#[test]
fn raw_url_matches_enabled_query_and_encodes_literal_values() {
    let mut api = api();
    let parameter = &mut api.operations[0].parameters[1];
    parameter.required = true;
    parameter
        .annotations
        .insert("example".into(), json!(["cat & dog", "café"]));
    let doc = document(&generate(&api, None));
    let raw = doc["item"][0]["item"][0]["request"]["url"]["raw"]
        .as_str()
        .unwrap();
    assert!(
        raw.ends_with("?tags=cat%20%26%20dog&tags=caf%C3%A9"),
        "{raw}"
    );
    api.operations[0].parameters[1].required = false;
    let doc = document(&generate(&api, None));
    assert!(
        !doc["item"][0]["item"][0]["request"]["url"]["raw"]
            .as_str()
            .unwrap()
            .contains('?')
    );
}
#[test]
fn security_alternatives_and_combinations_are_explicit_and_credentials_blank() {
    let mut api = api();
    api.operations[0].security = vec![
        SecurityRequirement {
            schemes: BTreeMap::from([("bearer".into(), vec![]), ("key".into(), vec![])]),
        },
        SecurityRequirement {
            schemes: BTreeMap::new(),
        },
    ];
    let catalog = SecuritySchemeCatalog {
        schemes: vec![
            SecurityScheme {
                name: "bearer".into(),
                description: None,
                kind: SecuritySchemeKind::Http {
                    scheme: Some("bearer".into()),
                    bearer_format: None,
                },
            },
            SecurityScheme {
                name: "key".into(),
                description: None,
                kind: SecuritySchemeKind::ApiKey {
                    name: Some("x-api-key".into()),
                    location: Some("header".into()),
                },
            },
        ],
    };
    let tree = generate(&api, Some(&catalog));
    let doc = document(&tree);
    let items = doc["item"][0]["item"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["request"]["auth"]["type"], "bearer");
    assert_eq!(items[1]["request"]["auth"]["type"], "noauth");
    assert!(
        items[0]["request"]["header"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["key"] == "x-api-key")
    );
    assert!(
        doc["variable"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["key"].as_str().unwrap().starts_with("credential_"))
            .all(|v| v["value"] == "")
    );
}
#[test]
fn unsupported_styles_and_missing_security_are_visible_or_fail_strictly() {
    let mut api = api();
    api.operations[0].parameters[0]
        .annotations
        .insert("style".into(), json!("matrix"));
    api.operations[0].security = vec![SecurityRequirement {
        schemes: BTreeMap::from([("unknown".into(), vec![])]),
    }];
    let tree = generate(&api, None);
    let diagnostics = tree.get("postman/diagnostics.json").unwrap();
    assert!(diagnostics.contains("parameter-style"));
    assert!(diagnostics.contains("security-scheme"));
    assert!(
        tree.get("postman/collection.json")
            .unwrap()
            .contains("Poolster mapping diagnostics")
    );
    assert!(
        Packages::new()
            .package(package("postman").with(collection().strict(true)))
            .generate(&api, None)
            .is_err()
    );
}
#[test]
fn independent_provider_and_explicit_handles_feed_collection() {
    struct Custom {
        meta: Meta,
    }
    impl Plugin<Postman> for Custom {
        fn kind(&self) -> &'static str {
            "custom"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<RequestExamples>()]
        }
        fn generate(&self, cx: &mut PluginContext<'_, Postman>) -> Result<()> {
            cx.publish(RequestExamples {
                requests: BTreeMap::from([(
                    ("createPet".into(), "application/json".into()),
                    json!({"name":"CUSTOM","password":"custom-secret"}),
                )]),
                ..Default::default()
            })
        }
    }
    let custom = Custom { meta: Meta::new() };
    let handle = custom.meta.handle();
    let collection = collection().using_examples(handle);
    let result = collection.handle();
    let tree = Packages::new()
        .package(
            package("postman")
                .with(custom)
                .with(collection)
                .with(environment().using_collection(result)),
        )
        .generate(&api(), None)
        .unwrap();
    assert!(
        tree.get("postman/collection.json")
            .unwrap()
            .contains("CUSTOM")
    );
    assert!(
        !tree
            .get("postman/collection.json")
            .unwrap()
            .contains("custom-secret")
    );
}
#[test]
fn binary_form_and_streaming_are_distinct_from_json() {
    let mut api = api();
    api.operations[0].request_body = Some(OperationRequestBody {
        required: true,
        description: None,
        media_types: vec![
            OperationMediaType {
                content_type: "application/octet-stream".into(),
                schema: None,
            },
            OperationMediaType {
                content_type: "application/x-www-form-urlencoded".into(),
                schema: Some(SchemaValue::reference("#/components/schemas/Pet")),
            },
            OperationMediaType {
                content_type: "multipart/form-data".into(),
                schema: Some(SchemaValue::reference("#/components/schemas/Pet")),
            },
        ],
    });
    api.operations[0].responses.push(OperationResponse {
        status: "default".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "text/event-stream".into(),
            schema: None,
        }],
    });
    let doc = document(&generate(&api, None));
    let modes = doc["item"][0]["item"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["request"]["body"]["mode"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(modes, vec!["file", "urlencoded", "formdata"]);
}
#[test]
fn duplicate_ids_and_unresolved_redaction_fail_safely() {
    let mut api = api();
    api.operations.push(api.operations[0].clone());
    assert!(
        Packages::new()
            .package(package("postman").with(collection()))
            .generate(&api, None)
            .is_err()
    );
    api.operations.pop();
    api.schemas.clear();
    assert!(
        Packages::new()
            .package(package("postman").with(collection().strict(true)))
            .generate(&api, None)
            .is_err()
    );
    let tree = generate(&api, None);
    assert!(
        !tree
            .get("postman/collection.json")
            .unwrap()
            .contains("do-not-export")
    );
}
#[test]
#[ignore = "requires Python with jsonschema; set KAJI_TEST_PYTHON and PYTHONPATH if needed"]
fn validates_actual_official_draft04_schema() {
    let root = tempfile::tempdir().unwrap();
    let mut variants = vec![api()];
    let mut forms = api();
    forms.operations[0]
        .request_body
        .as_mut()
        .unwrap()
        .media_types = vec![
        OperationMediaType {
            content_type: "multipart/form-data".into(),
            schema: Some(SchemaValue::reference("#/components/schemas/Pet")),
        },
        OperationMediaType {
            content_type: "application/octet-stream".into(),
            schema: None,
        },
    ];
    variants.push(forms);
    for (i, api) in variants.iter().enumerate() {
        let mut tree = generate(api, None);
        let _ = &mut tree;
        let aggregate = document(&tree);
        let mut documents = vec![("aggregate".to_owned(), aggregate.clone())];
        documents.extend(render::split_collections(&aggregate).unwrap());
        for (index, (_, document)) in documents.iter().enumerate() {
            let path = root.path().join(format!("collection-{i}-{index}.json"));
            std::fs::write(&path, serde_json::to_string_pretty(document).unwrap()).unwrap();
            let result=std::process::Command::new(std::env::var("KAJI_TEST_PYTHON").unwrap_or_else(|_|"python3".into())).arg("-c").arg("import json,sys,jsonschema; schema=json.load(open(sys.argv[1])); jsonschema.Draft4Validator.check_schema(schema); jsonschema.Draft4Validator(schema).validate(json.load(open(sys.argv[2])))").arg(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/schema/collection-v2.1.0.json")).arg(path).output().unwrap();
            assert!(
                result.status.success(),
                "{}{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}

#[test]
#[ignore = "requires npm ci in packages/internal/postman-execute and permission for ephemeral loopback mock"]
fn generated_collection_executes_with_newman_local_mock() {
    let root = tempfile::tempdir().unwrap();
    let aggregate = document(&generate(&api(), None));
    let mut documents = vec![("collection.json".to_owned(), aggregate.clone())];
    documents.extend(render::split_collections(&aggregate).unwrap());
    for (relative, document) in documents {
        let path = root.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string(&document).unwrap()).unwrap();
        let output = std::process::Command::new(
            std::env::var("KAJI_TEST_NODE").unwrap_or_else(|_| "node".into()),
        )
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../packages/internal/postman-execute/run.mjs"
        ))
        .arg(path)
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("\"requests\":1"));
    }
}

#[test]
fn split_groups_keep_request_identity_and_blank_credentials() {
    let mut source = api();
    source.operations[0]
        .annotations
        .insert("tags".into(), json!(["../Pets"]));
    let mut other = source.operations[0].clone();
    other.id = "createOtherPet".into();
    other.path = "/other/{id}".into();
    other.annotations.insert("tags".into(), json!(["../pets"]));
    source.operations.push(other);
    let tree = Packages::new()
        .package(package("postman").with(collection().split_by_group(true)))
        .generate(&source, None)
        .unwrap();
    let aggregate: Value =
        serde_json::from_str(tree.get("postman/collection.json").unwrap()).unwrap();
    let split: Vec<Value> = (0..2)
        .map(|index| {
            serde_json::from_str(
                tree.get(format!("postman/collections/group-{index:04}.json"))
                    .unwrap(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(split.len(), aggregate["item"].as_array().unwrap().len());
    for (index, child) in split.iter().enumerate() {
        assert_eq!(child["item"], json!([aggregate["item"][index].clone()]));
        assert_eq!(child["variable"], aggregate["variable"]);
        assert_eq!(child["info"]["schema"], aggregate["info"]["schema"]);
        assert_ne!(
            child["info"]["_postman_id"],
            aggregate["info"]["_postman_id"]
        );
        assert!(child.get("event").is_none());
    }
    assert_ne!(
        split[0]["info"]["_postman_id"],
        split[1]["info"]["_postman_id"]
    );
    let second = Packages::new()
        .package(package("postman").with(collection().split_by_group(true)))
        .generate(&source, None)
        .unwrap();
    assert_eq!(tree, second);
}

#[test]
fn regenerating_split_exports_preserves_only_the_customer_environment() {
    let export = || {
        Packages::new()
            .package(
                package("postman")
                    .with(collection().split_by_group(true))
                    .with(environment()),
            )
            .generate(&api(), None)
            .unwrap()
    };
    let root = tempfile::tempdir().unwrap();
    export().write_to(root.path()).unwrap();
    let environment = root.path().join("postman/environment.json");
    std::fs::write(&environment, "customer-owned credentials").unwrap();
    let split = root.path().join("postman/collections/group-0000.json");
    std::fs::write(&split, "edited generated requests").unwrap();
    let rejected = export().write_to(root.path()).unwrap_err();
    assert!(
        rejected
            .to_string()
            .contains("locally modified generated file")
    );
    assert_eq!(
        std::fs::read_to_string(&split).unwrap(),
        "edited generated requests"
    );
    let restored = export();
    std::fs::write(
        &split,
        restored.get("postman/collections/group-0000.json").unwrap(),
    )
    .unwrap();
    restored.write_to(root.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(environment).unwrap(),
        "customer-owned credentials"
    );
    let regenerated: Value =
        serde_json::from_str(&std::fs::read_to_string(split).unwrap()).unwrap();
    assert!(regenerated["item"][0]["item"].is_array());
    assert!(
        !serde_json::to_string(&regenerated)
            .unwrap()
            .contains("customer-owned credentials")
    );
}
