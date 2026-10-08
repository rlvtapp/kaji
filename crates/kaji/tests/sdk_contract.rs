//! Cross-language release contract for the product-style Poolster SDK surface.
//!
//! This deliberately tests behaviour-visible generated source rather than a
//! particular language's formatter. Every new runtime capability belongs in
//! this fixture before it is declared available across first-party targets.

use std::collections::BTreeMap;

use poolster::{ProfileSet, generate_with_security_catalog};
use poolster_core::{
    Api, Field, HttpMethod, Operation, OperationMediaType, OperationParameter,
    OperationRequestBody, OperationResponse, Schema, SchemaKind, SchemaValue, SecurityRequirement,
    SecurityScheme, SecuritySchemeCatalog, SecuritySchemeKind,
};
use serde_json::json;

fn reference(name: &str) -> SchemaValue {
    SchemaValue::reference(format!("#/components/schemas/{name}"))
}

/// A release contract is about the public behaviour emitted by a target, not
/// about which internal source file happens to own an operation. Large SDKs
/// deliberately split those internals into bounded files.
fn generated_source_under(tree: &poolster_core::GeneratedTree, root: &str) -> String {
    tree.iter()
        .filter(|(path, _)| path.starts_with(root))
        .map(|(_, source)| source)
        .collect::<Vec<_>>()
        .join("\n")
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
    let page = Schema::new(
        "ContactPage",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![
                Field {
                    name: "items".into(),
                    value: SchemaValue::new(SchemaKind::Array {
                        items: Box::new(reference("Contact")),
                    }),
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "nextCursor".into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: false,
                    annotations: Default::default(),
                },
            ],
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
        name: "Poolster Email".into(),
        version: "1.0.0".into(),
        schemas: vec![contact, page, error],
        operations: vec![
            Operation {
                id: "listContacts".into(),
                method: HttpMethod::Get,
                path: "/v1/contacts".into(),
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
                        media_types: vec![json(reference("ContactPage"))],
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
    let catalog = SecuritySchemeCatalog {
        schemes: vec![SecurityScheme {
            name: "Bearer".into(),
            description: None,
            kind: SecuritySchemeKind::Http {
                scheme: Some("bearer".into()),
                bearer_format: None,
            },
        }],
    };
    let tree = generate_with_security_catalog(
        &contract_api(),
        ProfileSet::new("sdks")
            .package(poolster::rust::package("rust").with(poolster::rust::sdk()))
            .package(poolster::ts::package("typescript-fetch").with(poolster::ts::sdk().fetch()))
            .package(poolster::ts::package("typescript-axios").with(poolster::ts::sdk().axios()))
            .package(poolster::go::package("go").with(poolster::go::sdk()))
            .package(poolster::python::package("python").with(poolster::python::sdk()))
            .package(poolster::php::package("php").with(poolster::php::sdk()))
            .package(poolster::java::package("java").with(poolster::java::sdk()))
            .package(poolster::dotnet::package("dotnet").with(poolster::dotnet::sdk()))
            .package(poolster::elixir::package("elixir").with(poolster::elixir::sdk())),
        Some(&catalog),
    )
    .unwrap();

    let rust = generated_source_under(&tree, "sdks/rust/src");
    assert!(rust.contains("pub struct ApiResponse"));
    assert!(rust.contains("ListContactsError"));
    assert!(rust.contains("with_bearer_token"));
    assert!(rust.contains("reqwest::Response"));
    assert!(rust.contains("Vec<u8>"));
    assert!(rust.contains("pub fn list_contacts_pages"));
    assert!(rust.contains("futures_util::stream::try_unfold"));

    for transport in ["typescript-fetch", "typescript-axios"] {
        let client = generated_source_under(&tree, &format!("sdks/{transport}"));
        let barrel = tree.get(format!("sdks/{transport}/index.ts")).unwrap();
        let custom = tree
            .get(format!("sdks/{transport}/custom/index.ts"))
            .unwrap();
        let runtime = tree
            .get(format!("sdks/{transport}/.poolster/client.ts"))
            .unwrap();
        assert!(client.contains("export class PoolsterEmail"));
        assert!(client.contains("readonly contacts"));
        assert!(client.contains("listPages"));
        assert!(client.contains("kajiJsonPath"));
        assert!(barrel.contains("export * from './custom/index.js'"));
        assert!(custom.contains("never overwritten by Poolster"));
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

    let python = generated_source_under(&tree, "sdks/python/src/poolster_email_sdk");
    assert!(python.contains("class ListContactsStatus429Error(ApiError):"));
    assert!(python.contains("def stream_events"));
    assert!(python.contains("def download_export"));

    for target in ["go", "php", "java", "dotnet", "elixir"] {
        assert!(tree.get(format!("sdks/{target}/STYLE_GUIDE.md")).is_some());
    }
}
