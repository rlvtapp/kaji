use super::*;
use poolster_core::{
    Field, HttpMethod, OperationMediaType, OperationParameter, OperationRequestBody,
    OperationResponse,
};
use std::process::Command;

fn api() -> Api {
    Api {
        name: "Example API".into(),
        version: "2026-09-19".into(),
        schemas: vec![Schema::new(
            "Contact",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![
                    Field {
                        name: "id".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: true,
                        annotations: Default::default(),
                    },
                    Field {
                        name: "display-name".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: false,
                        annotations: Default::default(),
                    },
                ],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        )],
        operations: vec![
            Operation {
                id: "getContact".into(),
                method: HttpMethod::Get,
                path: "/contacts/{contactId}".into(),
                parameters: vec![
                    OperationParameter {
                        name: "contactId".into(),
                        location: "path".into(),
                        required: true,
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                        description: None,
                        annotations: Default::default(),
                    },
                    OperationParameter {
                        name: "includeDeleted".into(),
                        location: "query".into(),
                        required: false,
                        schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                        description: None,
                        annotations: Default::default(),
                    },
                ],
                request_body: None,
                responses: vec![poolster_core::OperationResponse::json(
                    "200",
                    SchemaValue::reference("#/components/schemas/Contact"),
                )],
                security: vec![],
                annotations: Default::default(),
            },
            Operation {
                id: "health".into(),
                method: HttpMethod::Get,
                path: "/health".into(),
                ..Operation::default()
            },
        ],
        annotations: Default::default(),
    }
}

/// Assertions about a generated surface intentionally span the split
/// runtime, operation chunks, and resource modules.
fn rendered_python(tree: &GeneratedTree) -> String {
    tree.iter()
        .filter(|(path, _)| path.extension().is_some_and(|extension| extension == "py"))
        .map(|(_, contents)| contents)
        .collect::<Vec<_>>()
        .join("\n")
}

mod async_clients;
mod idempotency;
mod layout_errors;
mod middleware;
mod models;
mod names;
mod pagination;
mod pagination_body;
