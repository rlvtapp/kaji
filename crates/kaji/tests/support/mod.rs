use std::collections::BTreeMap;

use kaji_core::{
    Api, Field, HttpMethod, Operation, OperationMediaType, OperationResponse, Schema, SchemaKind,
    SchemaValue,
};

/// A deliberately compact contract that nevertheless exercises models, JSON
/// responses, authentication and a regular read operation in every target.
pub fn sdk_contract_api() -> Api {
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
    Api {
        name: "Kaji Contract API".into(),
        version: "1.0.0".into(),
        schemas: vec![contact],
        operations: vec![Operation {
            id: "getContact".into(),
            method: HttpMethod::Get,
            path: "/v1/contacts/current".into(),
            response_type: "Contact".into(),
            responses: vec![OperationResponse {
                status: "200".into(),
                description: Some("The current contact".into()),
                media_types: vec![OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::reference("#/components/schemas/Contact")),
                }],
            }],
            ..Operation::default()
        }],
        ..Api::default()
    }
}
