//! Types emission for the csharp HTTP SDK.
use crate::*;

pub(crate) fn parameter_json_content(parameter: &OperationParameter) -> bool {
    poolster_core::openapi32::parameter_content(parameter)
        .ok()
        .and_then(|items| items.into_iter().next())
        .is_some_and(|content| {
            content.content_type == "application/json" || content.content_type.ends_with("+json")
        })
}

pub(crate) enum DotnetResponseSurface {
    Json(String),
    Binary,
    Sse,
    Empty,
}

pub(crate) fn is_json_media(content_type: &str) -> bool {
    content_type.contains("json") || content_type.ends_with("+json")
}

pub(crate) fn is_sse_media(content_type: &str) -> bool {
    content_type.eq_ignore_ascii_case("text/event-stream")
}

pub(crate) fn request_body_is_binary(operation: &Operation) -> bool {
    if multipart::selected(operation) {
        return false;
    }
    operation.request_body.as_ref().is_some_and(|body| {
        body.media_types
            .iter()
            .find(|media| is_json_media(&media.content_type))
            .or_else(|| body.media_types.first())
            .is_some_and(|media| !is_json_media(&media.content_type))
    })
}

pub(crate) fn operation_response_surface(operation: &Operation) -> DotnetResponseSurface {
    operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .or_else(|| {
            operation
                .responses
                .iter()
                .find(|response| response.status == "default")
        })
        .and_then(|response| {
            response
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
        })
        .map(|media| {
            if is_sse_media(&media.content_type) {
                DotnetResponseSurface::Sse
            } else if is_json_media(&media.content_type) {
                media
                    .schema
                    .as_ref()
                    .map(|schema| DotnetResponseSurface::Json(csharp_type(schema, false)))
                    .unwrap_or(DotnetResponseSurface::Empty)
            } else {
                DotnetResponseSurface::Binary
            }
        })
        .unwrap_or(DotnetResponseSurface::Empty)
}

pub(crate) fn operation_request_type(operation: &Operation) -> Option<String> {
    if multipart::mixed(operation) {
        return Some("object".into());
    }
    if multipart::selected(operation) {
        return Some(multipart::body_name(operation));
    }
    operation
        .request_body
        .as_ref()
        .and_then(|body| {
            body.media_types
                .iter()
                .find(|media| media.content_type.contains("json"))
                .or_else(|| body.media_types.first())
        })
        .and_then(|media| media.schema.as_ref())
        .map(|schema| csharp_type(schema, false))
}

pub(crate) fn csharp_type(value: &SchemaValue, optional: bool) -> String {
    let base = match &value.kind {
        SchemaKind::Any | SchemaKind::Null | SchemaKind::Not { .. } => "JsonElement".into(),
        SchemaKind::Boolean => "bool".into(),
        SchemaKind::Integer => match value.format.as_deref() {
            Some("int32") => "int".into(),
            _ => "long".into(),
        },
        SchemaKind::Number => match value.format.as_deref() {
            Some("float") | Some("float32") => "float".into(),
            Some("decimal") => "decimal".into(),
            _ => "double".into(),
        },
        SchemaKind::String => match value.format.as_deref() {
            Some("date-time") => "DateTimeOffset".into(),
            Some("date") => "DateOnly".into(),
            Some("uuid") => "Guid".into(),
            Some("binary") | Some("byte") => "byte[]".into(),
            _ => "string".into(),
        },
        SchemaKind::Array { items } => format!("List<{}>", csharp_type(items, false)),
        SchemaKind::Object {
            additional_properties,
            ..
        } => match additional_properties {
            AdditionalProperties::Schema { value } => {
                format!("Dictionary<string, {}>", csharp_type(value, false))
            }
            _ => "JsonElement".into(),
        },
        SchemaKind::Reference { reference } => {
            pascal_case(reference.rsplit('/').next().unwrap_or(reference))
        }
        SchemaKind::OneOf { .. } | SchemaKind::AnyOf { .. } | SchemaKind::AllOf { .. } => {
            "JsonElement".into()
        }
    };
    nullable_type(
        base,
        optional || value.nullable || value.optional || value.nullish,
    )
}

pub(crate) fn nullable_type(base: String, nullable: bool) -> String {
    if !nullable || base.ends_with('?') {
        return base;
    }
    format!("{base}?")
}

pub(crate) fn is_reference_type(value: &SchemaValue) -> bool {
    !matches!(
        value.kind,
        SchemaKind::Boolean
            | SchemaKind::Integer
            | SchemaKind::Number
            | SchemaKind::Null
            | SchemaKind::Any
            | SchemaKind::Not { .. }
            | SchemaKind::OneOf { .. }
            | SchemaKind::AnyOf { .. }
            | SchemaKind::AllOf { .. }
    )
}

pub(crate) fn http_method_name(method: &str) -> String {
    let known = match method {
        "GET" => Some("Get"),
        "POST" => Some("Post"),
        "PUT" => Some("Put"),
        "PATCH" => Some("Patch"),
        "DELETE" => Some("Delete"),
        "HEAD" => Some("Head"),
        "OPTIONS" => Some("Options"),
        "TRACE" => Some("Trace"),
        _ => None,
    };
    known
        .map(|name| format!("HttpMethod.{name}"))
        .unwrap_or_else(|| format!("new HttpMethod({method:?})"))
}
