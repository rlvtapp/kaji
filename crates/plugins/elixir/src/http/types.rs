//! Types emission for the elixir HTTP SDK.
use crate::*;

pub(crate) fn request_body_required(operation: &Operation) -> bool {
    operation
        .request_body
        .as_ref()
        .map(|body| body.required)
        .unwrap_or(false)
}

pub(crate) fn response_schema(operation: &Operation) -> Option<&SchemaValue> {
    operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .and_then(|response| {
            response
                .media_types
                .iter()
                .find(|media| media.content_type.contains("json"))
        })
        .or_else(|| {
            operation
                .responses
                .iter()
                .find(|response| response.status.starts_with('2'))
                .and_then(|response| response.media_types.first())
        })
        .and_then(|media| media.schema.as_ref())
}

pub(crate) fn response_type(_api: &Api, operation: &Operation, module: &str) -> String {
    response_schema(operation)
        .map(|schema| elixir_type(schema, module))
        .unwrap_or_else(|| "nil".into())
}

pub(crate) fn response_decode(
    _api: &Api,
    operation: &Operation,
    module: &str,
    value: &str,
) -> String {
    response_schema(operation)
        .map(|schema| decode_value(value, schema, module))
        .unwrap_or_else(|| value.into())
}

pub(crate) fn decode_value(value: &str, schema: &SchemaValue, module: &str) -> String {
    match &schema.kind {
        SchemaKind::Reference { reference } => format!(
            "case {value} do nil -> nil; map when is_map(map) or is_list(map) -> {module}.Models.{}.from_map(map); other -> other end",
            pascal_case(reference.rsplit('/').next().unwrap_or(reference))
        ),
        SchemaKind::Array { items } => format!(
            "case {value} do nil -> nil; items when is_list(items) -> Enum.map(items, fn item -> {} end); other -> other end",
            decode_value("item", items, module)
        ),
        _ => value.into(),
    }
}

pub(crate) fn elixir_type(value: &SchemaValue, module: &str) -> String {
    let base = match &value.kind {
        SchemaKind::Any | SchemaKind::Not { .. } => "term()".into(),
        SchemaKind::Null => "nil".into(),
        SchemaKind::Boolean => "boolean()".into(),
        SchemaKind::Integer => "integer()".into(),
        SchemaKind::Number => "number()".into(),
        // Elixir typespecs cannot express binary literal unions. Keep wire
        // enums as strings rather than inventing atom values for the API.
        SchemaKind::String => "String.t()".into(),
        SchemaKind::Array { items } => format!("[{}]", elixir_type(items, module)),
        SchemaKind::Object {
            additional_properties: AdditionalProperties::Schema { value },
            ..
        } => format!(
            "%{{optional(String.t()) => {}}}",
            elixir_type(value, module)
        ),
        SchemaKind::Object { .. } => "map()".into(),
        SchemaKind::Reference { reference } => format!(
            "{module}.Models.{}.t()",
            pascal_case(reference.rsplit('/').next().unwrap_or(reference))
        ),
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => variants
            .iter()
            .map(|variant| elixir_type(variant, module))
            .collect::<Vec<_>>()
            .join(" | "),
        SchemaKind::AllOf { .. } => "map()".into(),
    };
    if value.nullable && base != "nil" {
        format!("{base} | nil")
    } else {
        base
    }
}
