//! Request planning and emission for Elixir HTTP.
use crate::*;

pub(crate) fn json_content_allows_null(parameter: &poolster_core::OperationParameter) -> bool {
    fn allows(value: &SchemaValue) -> bool {
        value.nullable
            || matches!(value.kind, SchemaKind::Any | SchemaKind::Null)
            || match &value.kind {
                SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => {
                    variants.iter().any(allows)
                }
                _ => false,
            }
    }
    parameter.schema.as_ref().is_none_or(allows)
}

pub(crate) fn is_json_parameter_content(parameter: &poolster_core::OperationParameter) -> bool {
    poolster_core::openapi32::parameter_content(parameter)
        .ok()
        .and_then(|items| items.into_iter().next())
        .is_some_and(|content| {
            content.content_type == "application/json" || content.content_type.ends_with("+json")
        })
}

pub(crate) fn parameter_content_value(
    parameter: &poolster_core::OperationParameter,
    value: &str,
) -> String {
    if let Some(content) = poolster_core::openapi32::parameter_content(parameter)
        .ok()
        .and_then(|items| items.into_iter().next())
    {
        let allow_null = parameter.required
            && is_json_parameter_content(parameter)
            && json_content_allows_null(parameter);
        format!(
            "Client.parameter_content({value}, \"{}\", {allow_null})",
            escape_elixir_string(&content.content_type)
        )
    } else {
        value.to_owned()
    }
}

pub(crate) fn operation_option_types(operation: &Operation, module: &str) -> String {
    let mut items = operation
        .parameters
        .iter()
        .map(|parameter| {
            let value = parameter
                .schema
                .as_ref()
                .map(|schema| elixir_type(schema, module))
                .unwrap_or_else(|| "term()".into());
            format!(
                "{{:{}, {value}}}",
                elixir_parameter_identifier(operation, parameter)
            )
        })
        .collect::<Vec<_>>();
    if let Some(body) = &operation.request_body {
        let value = if operation_body_kind(operation) == "binary" {
            "binary()".into()
        } else {
            body.media_types
                .iter()
                .find(|media| media.content_type.contains("json"))
                .or_else(|| body.media_types.first())
                .and_then(|media| media.schema.as_ref())
                .map(|schema| elixir_type(schema, module))
                .unwrap_or_else(|| "term()".into())
        };
        items.push(format!("{{:body, {value}}}"));
    }
    if items.is_empty() {
        "term()".into()
    } else {
        items.join(" | ")
    }
}

pub(crate) fn operation_body_kind(operation: &Operation) -> &'static str {
    if operation.request_body.as_ref().is_some_and(|body| {
        body.media_types
            .iter()
            .any(|media| media.content_type.starts_with("multipart/"))
    }) {
        return if operation.request_body.as_ref().is_some_and(|body| {
            body.media_types.iter().any(|media| {
                media.content_type == "application/json" || media.content_type.ends_with("+json")
            })
        }) {
            "multipart_json"
        } else {
            "multipart"
        };
    }
    operation
        .request_body
        .as_ref()
        .and_then(|body| body.media_types.first())
        .map(|media| match media.content_type.as_str() {
            "application/x-www-form-urlencoded" => "form",
            "application/octet-stream" => "binary",
            _ => "json",
        })
        .unwrap_or("json")
}

pub(crate) fn operation_response_kind(operation: &Operation) -> &'static str {
    let content_type = operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .and_then(|response| response.media_types.first())
        .map(|media| media.content_type.as_str());
    match content_type {
        Some("application/json-seq") => "json_seq",
        Some("application/x-ndjson" | "application/ndjson" | "application/jsonl") => "ndjson",
        Some(value) if value == "application/octet-stream" || value.starts_with("image/") => {
            "binary"
        }
        Some(value) if value.starts_with("text/") => "text",
        None => "void",
        _ => "json",
    }
}

pub(crate) fn elixir_pagination_argument(
    operation: &Operation,
    input: &poolster_core::pagination::PaginationInput,
) -> String {
    operation
        .parameters
        .iter()
        .find(|parameter| parameter.name == input.name && parameter.location == input.location)
        .map(|parameter| elixir_parameter_identifier(operation, parameter))
        .unwrap_or_else(|| elixir_identifier(&input.name))
}
