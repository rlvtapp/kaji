use super::*;
use poolster_core::{
    Field, HttpMethod, OperationMediaType, OperationParameter, OperationRequestBody,
    OperationResponse,
};

mod byte_boundary;
mod idempotency;
mod native_models;
mod native_wire;
mod nullability;
mod package_layout;
mod page_cases;
mod pagination;

fn api() -> Api {
    Api {
        name: "Pet Store API".into(),
        version: "2026-09-19".into(),
        schemas: vec![Schema::new(
            "Pet",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![
                    Field {
                        name: "id".into(),
                        value: SchemaValue::new(SchemaKind::Integer),
                        required: true,
                        annotations: BTreeMap::new(),
                    },
                    Field {
                        name: "display-name".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: false,
                        annotations: BTreeMap::new(),
                    },
                ],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        )],
        operations: vec![Operation {
            id: "getPet".into(),
            method: HttpMethod::Get,
            path: "/pets/{id}".into(),
            parameters: vec![
                OperationParameter {
                    name: "id".into(),
                    location: "path".into(),
                    required: true,
                    schema: Some(SchemaValue::new(SchemaKind::Integer)),
                    description: None,
                    annotations: BTreeMap::new(),
                },
                OperationParameter {
                    name: "include".into(),
                    location: "query".into(),
                    required: false,
                    schema: Some(SchemaValue::new(SchemaKind::String)),
                    description: None,
                    annotations: BTreeMap::new(),
                },
            ],
            responses: vec![OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::reference("#/components/schemas/Pet")),
                }],
            }],
            ..Default::default()
        }],
        annotations: BTreeMap::new(),
    }
}

fn page_fixture() -> Api {
    let mut source = api();
    let op = &mut source.operations[0];
    op.id = "listPets".into();
    op.path = "/pets".into();
    op.parameters = ["page", "limit"]
        .iter()
        .map(|name| OperationParameter {
            name: (*name).into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::Integer)),
            description: None,
            annotations: BTreeMap::new(),
        })
        .collect();
    op.responses = vec![OperationResponse::json(
        "200",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "items".into(),
                required: true,
                annotations: BTreeMap::new(),
                value: SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                }),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    )];
    op.annotations.insert("x-poolster-pagination".into(),serde_json::json!({"type":"page","inputs":[{"name":"page","in":"parameters","type":"page"},{"name":"limit","in":"parameters","type":"limit"}],"outputs":{"results":"/items"}}));
    source
}

fn idempotency_fixture() -> Api {
    let mut source = page_fixture();
    let op = &mut source.operations[0];
    op.annotations.clear();
    op.id = "createItem".into();
    op.method = poolster_core::HttpMethod::Post;
    op.parameters = vec![OperationParameter {
        name: "X-Request-Key".into(),
        location: "header".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: Default::default(),
    }];
    op.responses = vec![OperationResponse::json(
        "200",
        SchemaValue::new(SchemaKind::String),
    )];
    op.annotations.insert("x-poolster-idempotency-resolved".into(),serde_json::json!({"header":"X-Request-Key","parameter_name":"X-Request-Key","auto_generate":true}));
    source
}
fn byte_boundary_api() -> Api {
    let parameters = (0..80)
        .map(|index| poolster_core::OperationParameter {
            name: format!("queryParameter{index:03}{}", "LongName".repeat(20)),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        })
        .collect::<Vec<_>>();
    Api {
        name: "Byte Probe".into(),
        operations: (0..20)
            .map(|index| Operation {
                id: format!("getItem{index}"),
                path: format!("/items/{index}"),
                parameters: parameters.clone(),
                responses: vec![poolster_core::OperationResponse::json(
                    "200",
                    SchemaValue::new(SchemaKind::String),
                )],
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}
