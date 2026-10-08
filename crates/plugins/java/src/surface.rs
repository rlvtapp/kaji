use super::*;

pub(super) fn operation_parameters(operation: &Operation) -> Vec<&OperationParameter> {
    operation
        .parameters
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.location.as_str(),
                "path" | "query" | "header" | "cookie" | "querystring"
            )
        })
        .collect()
}

pub(super) fn request_body_schema(operation: &Operation) -> Option<&SchemaValue> {
    operation.request_body.as_ref().and_then(|body| {
        body.media_types
            .iter()
            .find(|media| media.content_type.contains("json"))
            .or_else(|| body.media_types.first())
            .and_then(|media| media.schema.as_ref())
    })
}

pub(super) fn request_content_type(operation: &Operation) -> Option<&str> {
    operation
        .request_body
        .as_ref()?
        .media_types
        .iter()
        .find(|media| media.content_type.contains("json"))
        .or_else(|| operation.request_body.as_ref()?.media_types.first())
        .map(|media| media.content_type.as_str())
}

pub(super) fn is_json_media(content_type: &str) -> bool {
    content_type.contains("json") || content_type.ends_with("+json")
}

pub(super) fn is_sse_media(content_type: &str) -> bool {
    content_type.eq_ignore_ascii_case("text/event-stream")
}

pub(super) fn request_body_is_binary(operation: &Operation) -> bool {
    request_content_type(operation).is_some_and(|content_type| !is_json_media(content_type))
}

pub(super) enum ResponseSurface<'a> {
    Json(&'a SchemaValue),
    Binary,
    Sse,
    Empty,
}

/// Pick the actual successful representation rather than treating every
/// OpenAPI response as JSON. This keeps download and event endpoints usable
/// without a hand-written transport escape hatch.
pub(super) fn response_surface(operation: &Operation) -> ResponseSurface<'_> {
    let Some(response) = operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .or_else(|| {
            operation
                .responses
                .iter()
                .find(|response| response.status == "default")
        })
    else {
        return ResponseSurface::Empty;
    };
    let Some(media) = response
        .media_types
        .iter()
        .find(|media| is_sse_media(&media.content_type))
        .or_else(|| {
            response
                .media_types
                .iter()
                .find(|media| is_json_media(&media.content_type))
        })
        .or_else(|| response.media_types.first())
    else {
        return ResponseSurface::Empty;
    };
    if is_sse_media(&media.content_type) {
        ResponseSurface::Sse
    } else if is_json_media(&media.content_type) {
        media
            .schema
            .as_ref()
            .map(ResponseSurface::Json)
            .unwrap_or(ResponseSurface::Empty)
    } else {
        ResponseSurface::Binary
    }
}

pub(super) fn parameter_json_content(parameter: &OperationParameter) -> bool {
    poolster_core::openapi32::parameter_content(parameter)
        .ok()
        .and_then(|items| items.into_iter().next())
        .is_some_and(|content| {
            content.content_type == "application/json" || content.content_type.ends_with("+json")
        })
}
pub(super) fn parameter_type(parameter: &OperationParameter) -> String {
    if parameter.location == "querystring" {
        return "String".into();
    }
    parameter
        .schema
        .as_ref()
        .map(java_type)
        .unwrap_or_else(|| "java.lang.Object".to_owned())
}

pub(super) fn response_class(schema: &SchemaValue) -> String {
    match &schema.kind {
        SchemaKind::Reference { reference } => {
            type_name(reference.rsplit('/').next().unwrap_or(reference))
        }
        SchemaKind::String => "String".into(),
        SchemaKind::Boolean => "Boolean".into(),
        SchemaKind::Integer => "Long".into(),
        SchemaKind::Number => "Double".into(),
        _ => "JsonNode".into(),
    }
}

/// Jackson's class-token API can preserve named models and scalar responses.
/// Arrays and anonymous composite schemas need a `TypeReference`; Poolster emits
/// those as `JsonNode` instead of producing generic Java that cannot compile.
pub(super) fn operation_response_type(schema: &SchemaValue) -> String {
    match &schema.kind {
        SchemaKind::Reference { .. }
        | SchemaKind::String
        | SchemaKind::Boolean
        | SchemaKind::Integer
        | SchemaKind::Number => java_type(schema),
        _ => "JsonNode".into(),
    }
}

pub(super) fn java_type(value: &SchemaValue) -> String {
    match &value.kind {
        SchemaKind::Any
        | SchemaKind::OneOf { .. }
        | SchemaKind::AnyOf { .. }
        | SchemaKind::AllOf { .. }
        | SchemaKind::Not { .. } => "JsonNode".into(),
        SchemaKind::Null => "Void".into(),
        SchemaKind::Boolean => "Boolean".into(),
        SchemaKind::Integer => "Long".into(),
        SchemaKind::Number => "Double".into(),
        SchemaKind::String => "String".into(),
        SchemaKind::Array { items } => format!("List<{}>", java_type(items)),
        SchemaKind::Object {
            additional_properties,
            ..
        } => match additional_properties {
            AdditionalProperties::Schema { value } => format!("Map<String, {}>", java_type(value)),
            _ => "JsonNode".into(),
        },
        SchemaKind::Reference { reference } => {
            type_name(reference.rsplit('/').next().unwrap_or(reference))
        }
    }
}
