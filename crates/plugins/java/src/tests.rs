use std::collections::BTreeMap;

use poolster_core::{
    AdditionalProperties, Api, Field, GeneratedTree, HttpMethod, Operation, OperationMediaType,
    OperationParameter, OperationRequestBody, OperationResponse, Schema, SchemaKind, SchemaValue,
};

use super::{
    api_exception, render_client_base, render_model, render_operation, render_sdk,
    render_sdk_with_policy, render_test_sdk,
};
use poolster_core::SdkClientStyle;

fn string() -> SchemaValue {
    SchemaValue::new(SchemaKind::String)
}

fn integer() -> SchemaValue {
    SchemaValue::new(SchemaKind::Integer)
}

fn contact_api() -> Api {
    Api {
        name: "Poolster Email API".into(),
        version: "2026-09-19".into(),
        schemas: vec![Schema::new(
            "Contact",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![
                    Field {
                        name: "email".into(),
                        value: string(),
                        required: true,
                        annotations: BTreeMap::new(),
                    },
                    Field {
                        name: "display_name".into(),
                        value: string(),
                        required: false,
                        annotations: BTreeMap::new(),
                    },
                ],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        )],
        operations: vec![Operation {
            id: "getContact".into(),
            method: HttpMethod::Get,
            path: "/v1/contacts/{contact_id}".into(),
            parameters: vec![
                OperationParameter {
                    name: "contact_id".into(),
                    location: "path".into(),
                    required: true,
                    schema: Some(string()),
                    description: None,
                    annotations: BTreeMap::new(),
                },
                OperationParameter {
                    name: "expand".into(),
                    location: "query".into(),
                    required: false,
                    schema: Some(string()),
                    description: None,
                    annotations: BTreeMap::new(),
                },
                OperationParameter {
                    name: "X-Request-ID".into(),
                    location: "header".into(),
                    required: false,
                    schema: Some(string()),
                    description: None,
                    annotations: BTreeMap::new(),
                },
            ],
            request_body: Some(OperationRequestBody {
                required: false,
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::reference("#/components/schemas/Contact")),
                }],
            }),
            responses: vec![OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::reference("#/components/schemas/Contact")),
                }],
            }],
            security: Vec::new(),
            annotations: BTreeMap::new(),
        }],
        annotations: BTreeMap::new(),
    }
}

/// The public client inherits generated operations from bounded internal
/// classes; surface assertions intentionally inspect all Java sources.
fn rendered_java(tree: &GeneratedTree) -> String {
    tree.iter()
        .filter(|(path, _)| {
            path.extension()
                .is_some_and(|extension| extension == "java")
        })
        .map(|(_, contents)| contents)
        .collect::<Vec<_>>()
        .join("\n")
}

mod facades;
mod models;
mod pagination;
mod runtime;
