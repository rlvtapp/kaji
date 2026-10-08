fn render_test_sdk(api: &Api, root: &str, name: Option<&str>) -> Result<GeneratedTree> {
    render_sdk(api, root, name, SdkClientStyle::Flat, 0)
}
fn all_source(tree: &GeneratedTree) -> String {
    tree.iter()
        .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "go"))
        .map(|(_, source)| source)
        .collect::<Vec<_>>()
        .join("\n")
}
use std::{collections::BTreeMap, fs, process::Command};

use poolster_core::{
    Field, HttpMethod, OperationMediaType, OperationRequestBody, OperationResponse, SchemaKind,
};

use super::*;

fn string_schema() -> SchemaValue {
    SchemaValue::new(SchemaKind::String)
}

fn contact_api() -> Api {
    Api {
        name: "Poolster Email API".into(),
        version: "1.0.0".into(),
        schemas: vec![Schema::new(
            "Contact",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![
                    Field {
                        name: "email".into(),
                        value: string_schema(),
                        required: true,
                        annotations: BTreeMap::new(),
                    },
                    Field {
                        name: "display_name".into(),
                        value: string_schema(),
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
                    schema: Some(string_schema()),
                    description: None,
                    annotations: BTreeMap::new(),
                },
                OperationParameter {
                    name: "expand".into(),
                    location: "query".into(),
                    required: false,
                    schema: Some(string_schema()),
                    description: None,
                    annotations: BTreeMap::new(),
                },
                OperationParameter {
                    name: "X-Request-ID".into(),
                    location: "header".into(),
                    required: false,
                    schema: Some(string_schema()),
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

mod basic;
mod layout;
mod middleware;
mod pagination;
mod requests;
mod runtime;
mod wire;
