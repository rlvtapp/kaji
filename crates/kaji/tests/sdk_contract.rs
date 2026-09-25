//! Cross-language release contract for the product-style Kaji SDK surface.
//!
//! This deliberately tests behaviour-visible generated source rather than a
//! particular language's formatter. Every new runtime capability belongs in
//! this fixture before it is declared available across first-party targets.

use std::collections::BTreeMap;

use kaji::{ProfileSet, generate};
use kaji_core::{
    Api, Field, HttpMethod, Operation, OperationMediaType, OperationParameter,
    OperationRequestBody, OperationResponse, Schema, SchemaKind, SchemaValue, SecurityRequirement,
};
use serde_json::json;

fn reference(name: &str) -> SchemaValue {
    SchemaValue::reference(format!("#/components/schemas/{name}"))
}

fn contract_api() -> Api {
    let contact = Schema::new(
        "Contact",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "id".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: true,
                annotations: BTreeMap::new(),
            }],
            additional_properties: Default::default(),
        }),
    );
    let error = Schema::new(
        "ErrorResponse",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "message".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: true,
                annotations: BTreeMap::new(),
            }],
            additional_properties: Default::default(),
        }),
    );
    let json = |schema: SchemaValue| OperationMediaType {
        content_type: "application/json".into(),
        schema: Some(schema),
    };
    let error_response = OperationResponse {
        status: "429".into(),
        description: Some("Rate limited".into()),
        media_types: vec![json(reference("ErrorResponse"))],
    };
    let secured = vec![SecurityRequirement {
        schemes: BTreeMap::from([("Bearer".into(), Vec::new())]),
    }];
    Api {
        name: "Kaji Email".into(),
        version: "1.0.0".into(),
        schemas: vec![contact, error],
        operations: vec![
            Operation {
                id: "listContacts".into(),
                method: HttpMethod::Get,
                path: "/v1/contacts".into(),
                response_type: "Contact[]".into(),
                request_type: None,
                parameters: vec![OperationParameter {
                    name: "cursor".into(),
                    location: "query".into(),
                    required: false,
                    schema: Some(SchemaValue::new(SchemaKind::String)),
                    description: None,
                    annotations: BTreeMap::new(),
                }],
                request_body: None,
                responses: vec![
                    OperationResponse {
                        status: "200".into(),
                        description: Some("Contacts".into()),
                        media_types: vec![json(SchemaValue::new(SchemaKind::Array {
                            items: Box::new(reference("Contact")),
                        }))],
                    },
                    error_response.clone(),
                ],
                security: secured.clone(),
                annotations: BTreeMap::from([(
                    "x-kaji-pagination".into(),
                    json!({
                        "type": "cursor",
                        "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
                        "outputs": { "nextCursor": "$.nextCursor" },
                    }),
                )]),
            },
            Operation {
                id: "createContact".into(),
                method: HttpMethod::Post,
                path: "/v1/contacts".into(),
                response_type: "Contact".into(),
                request_type: Some("Contact".into()),
                parameters: vec![OperationParameter {
                    name: "Idempotency-Key".into(),
                    location: "header".into(),
                    required: true,
                    schema: Some(SchemaValue::new(SchemaKind::String)),
                    description: None,
                    annotations: BTreeMap::new(),
                }],
                request_body: Some(OperationRequestBody {
                    required: true,
                    description: None,
                    media_types: vec![json(reference("Contact"))],
                }),
                responses: vec![
                    OperationResponse {
                        status: "201".into(),
                        description: Some("Created".into()),
                        media_types: vec![json(reference("Contact"))],
                    },
                    error_response.clone(),
                ],
                security: secured.clone(),
                annotations: BTreeMap::new(),
            },
            Operation {
                id: "streamEvents".into(),
                method: HttpMethod::Get,
                path: "/v1/events".into(),
                response_type: "string".into(),
                request_type: None,
                parameters: Vec::new(),
                request_body: None,
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: Some("SSE".into()),
                    media_types: vec![OperationMediaType {
                        content_type: "text/event-stream".into(),
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                    }],
                }],
                security: secured.clone(),
                annotations: BTreeMap::new(),
            },
            Operation {
                id: "downloadExport".into(),
                method: HttpMethod::Get,
                path: "/v1/exports/{id}".into(),
                response_type: "binary".into(),
                request_type: None,
                parameters: vec![OperationParameter {
                    name: "id".into(),
                    location: "path".into(),
                    required: true,
                    schema: Some(SchemaValue::new(SchemaKind::String)),
                    description: None,
                    annotations: BTreeMap::new(),
                }],
                request_body: None,
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: Some("Archive".into()),
                    media_types: vec![OperationMediaType {
                        content_type: "application/octet-stream".into(),
                        schema: None,
                    }],
                }],
                security: secured,
                annotations: BTreeMap::new(),
            },
        ],
        annotations: BTreeMap::new(),
    }
}

#[test]
fn all_first_party_packages_preserve_the_public_sdk_contract() {
    let tree = generate(
        &contract_api(),
        ProfileSet::new("sdks")
            .rust()
            .typescript_fetch()
            .typescript_axios()
            .go()
            .python()
            .php()
            .java()
            .dotnet()
            .elixir(),
    )
    .unwrap();

    let rust = tree.get("sdks/rust/src/client.rs").unwrap();
    assert!(rust.contains("pub struct ApiResponse"));
    assert!(rust.contains("ListContactsError"));
    assert!(rust.contains("with_bearer_token"));
    assert!(rust.contains("reqwest::Response"));
    assert!(rust.contains("Vec<u8>"));
    assert!(rust.contains("pub fn list_contacts_pages"));
    assert!(rust.contains("futures_util::stream::try_unfold"));

    for transport in ["typescript-fetch", "typescript-axios"] {
        let client = tree.get(format!("sdks/{transport}/client.ts")).unwrap();
        let barrel = tree.get(format!("sdks/{transport}/index.ts")).unwrap();
        let custom = tree
            .get(format!("sdks/{transport}/custom/index.ts"))
            .unwrap();
        let runtime = tree
            .get(format!("sdks/{transport}/.kaji/client.ts"))
            .unwrap();
        assert!(client.contains("export class KajiEmail"));
        assert!(client.contains("readonly contacts"));
        assert!(client.contains("listPages"));
        assert!(client.contains("kajiJsonPath"));
        assert!(barrel.contains("export * from './custom'"));
        assert!(custom.contains("never overwritten by Kaji"));
        assert!(runtime.contains("export class ApiError"));
        assert!(runtime.contains("SecurityCredentials"));
        assert!(runtime.contains("export const toEventStream"));
        assert!(runtime.contains("EventStreamResult<T> = AsyncIterable<T>"));
        assert!(runtime.contains("export interface RetryConfig"));
        assert!(runtime.contains("const retryAllowed"));
        assert!(runtime.contains("export interface ClientHooks"));
        assert!(runtime.contains("beforeRequest"));
        assert!(runtime.contains("afterResponse"));
        assert!(runtime.contains("onError"));
    }

    let python = tree
        .get("sdks/python/src/kaji_email_sdk/client.py")
        .unwrap();
    assert!(python.contains("class ListContactsStatus429Error(ApiError):"));
    assert!(python.contains("def stream_events"));
    assert!(python.contains("def download_export"));

    for target in ["go", "php", "java", "dotnet", "elixir"] {
        assert!(tree.get(format!("sdks/{target}/STYLE_GUIDE.md")).is_some());
    }
}
