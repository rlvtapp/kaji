//! Types emission for the Swift HTTP SDK.
use crate::*;

pub(crate) struct ParameterRender {
    pub(crate) signature: String,
}

pub(crate) fn parameter_json_content(parameter: &OperationParameter) -> bool {
    poolster_core::openapi32::parameter_content(parameter)
        .ok()
        .and_then(|items| items.into_iter().next())
        .is_some_and(|content| {
            content.content_type == "application/json" || content.content_type.ends_with("+json")
        })
}
pub(crate) fn swift_type(schema: &SchemaValue, optional: bool) -> String {
    let mut ty = match &schema.kind {
        SchemaKind::Any
        | SchemaKind::Null
        | SchemaKind::Not { .. }
        | SchemaKind::OneOf { .. }
        | SchemaKind::AnyOf { .. }
        | SchemaKind::AllOf { .. } => "JSONValue".to_owned(),
        SchemaKind::Boolean => "Bool".to_owned(),
        SchemaKind::Integer => "Int".to_owned(),
        SchemaKind::Number => "Double".to_owned(),
        SchemaKind::String => "String".to_owned(),
        SchemaKind::Array { items } => format!("[{}]", swift_type(items, false)),
        SchemaKind::Object { .. } => "JSONValue".to_owned(),
        SchemaKind::Reference { reference } => {
            type_name(reference.rsplit('/').next().unwrap_or(reference))
        }
    };
    if optional || schema.nullable || schema.optional || schema.nullish {
        ty.push('?');
    }
    ty
}
