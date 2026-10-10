use super::*;
use poolster_core::engine::Packages;
use poolster_core::{Api, HttpMethod, Operation};

fn api() -> Api {
    Api {
        name: "Contacts".into(),
        version: "1.0.0".into(),
        operations: vec![Operation {
            id: "listContacts".into(),
            method: HttpMethod::Get,
            path: "/contacts".into(),
            ..Operation::default()
        }],
        ..Api::default()
    }
}

#[test]
fn flat_sdk_readme_matches_its_direct_method_surface() {
    let tree = Packages::new()
        .package(
            package("sdk")
                .name("custom-sdk")
                .with(sdk().flat().operation_prefix("api")),
        )
        .generate(&api(), None)
        .unwrap();
    let client = tree.get("sdk/src/client/operations/chunk_0001.rs").unwrap();
    assert!(client.contains("pub async fn api_list_contacts"));
    assert!(!client.contains("pub fn contacts(&self)"));
    let readme = tree.get("sdk/README.md").unwrap();
    assert!(readme.contains("client.api_list_contacts().await?"));
    assert!(!readme.contains("client.contacts()"));
    assert!(
        tree.get("sdk/Cargo.toml")
            .unwrap()
            .contains("name = \"custom-sdk\"")
    );
    assert!(
        tree.get("sdk/STYLE_GUIDE.md")
            .unwrap()
            .contains("No resource accessors are generated")
    );
}

#[test]
fn namespaced_sdk_delegates_to_the_configured_direct_method() {
    let tree = Packages::new()
        .package(package("sdk").with(sdk().namespaced().operation_prefix("api")))
        .generate(&api(), None)
        .unwrap();
    let operations = tree.get("sdk/src/client/operations/chunk_0001.rs").unwrap();
    let resources = tree
        .get("sdk/src/client/resources/contacts_1/chunk_0001.rs")
        .unwrap();
    assert!(operations.contains("pub async fn api_list_contacts"));
    assert!(resources.contains("pub fn contacts(&self)"));
    assert!(resources.contains("self.client.api_list_contacts().await"));
    assert!(
        tree.get("sdk/README.md")
            .unwrap()
            .contains("client.contacts().list().await?")
    );
    assert!(
        !tree
            .get("sdk/STYLE_GUIDE.md")
            .unwrap()
            .contains("compatibility")
    );
}

#[test]
fn typed_operation_schemas_preserve_arrays_bodies_and_empty_responses() {
    use poolster_core::{
        OperationMediaType, OperationRequestBody, OperationResponse, SchemaKind, SchemaValue,
    };
    let mut api = api();
    api.operations = vec![
        Operation {
            id: "replaceContacts".into(),
            method: HttpMethod::Post,
            path: "/contacts".into(),
            request_body: Some(OperationRequestBody::json(
                SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::String)),
                }),
                true,
            )),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                }),
            )],
            ..Default::default()
        },
        Operation {
            id: "deleteContact".into(),
            method: HttpMethod::Delete,
            path: "/contacts".into(),
            responses: vec![OperationResponse {
                status: "204".into(),
                description: None,
                media_types: vec![],
            }],
            ..Default::default()
        },
        Operation {
            id: "unknownBody".into(),
            method: HttpMethod::Post,
            path: "/unknown".into(),
            request_body: Some(OperationRequestBody {
                required: true,
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "application/json".into(),
                    schema: None,
                }],
            }),
            responses: vec![OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "application/json".into(),
                    schema: None,
                }],
            }],
            ..Default::default()
        },
    ];
    let tree = Packages::new()
        .package(package("sdk").with(sdk().flat()))
        .generate(&api, None)
        .unwrap();
    let source = tree.get("sdk/src/client/operations/chunk_0001.rs").unwrap();
    assert!(source.contains(
        "replace_contacts(&self, body: &Vec<String>) -> Result<Vec<i64>, ReplaceContactsError>"
    ));
    assert!(source.contains("delete_contact(&self) -> Result<(), DeleteContactError>"));
    assert!(source.contains("unknown_body(&self, body: &serde_json::Value) -> Result<serde_json::Value, UnknownBodyError>"));
    assert!(source.contains("request = request.json(body)"));
}
