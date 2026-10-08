use super::*;

/// Cursor pagination is opt-in. A declaration has to name an existing
/// operation parameter; Poolster never assumes a `cursor` query argument.
#[derive(Clone, Debug)]
pub(super) struct CursorPagination {
    pub(super) input: CursorInput,
    pub(super) next_cursor_path: String,
}

#[derive(Clone, Debug)]
pub(super) enum CursorInput {
    Parameter(String),
    Body(String),
}

/// Explicit offset/limit support deliberately requires optional integer
/// parameters. This avoids generating a paginator that guesses where a
/// continuation belongs or how a caller's request body should be mutated.
#[derive(Clone, Debug)]
pub(super) struct OffsetPagination {
    pub(super) offset_name: String,
    pub(super) limit_name: String,
    pub(super) results_path: String,
}

#[derive(Clone, Debug)]
pub(super) struct UrlPagination {
    pub(super) next_url_path: String,
}

pub(super) fn pagination_parameter(
    operation: &Operation,
    inputs: &[Value],
    kind: &str,
) -> Option<String> {
    let input = inputs
        .iter()
        .find(|input| input.get("type").and_then(Value::as_str) == Some(kind))?
        .as_object()?;
    if !matches!(
        input.get("in").and_then(Value::as_str),
        None | Some("parameters")
    ) {
        return None;
    }
    let name = input.get("name")?.as_str()?;
    let parameter = operation.parameters.iter().find(|parameter| {
        parameter.name == name
            && !parameter.required
            && matches!(
                parameter.schema.as_ref().map(|schema| &schema.kind),
                Some(SchemaKind::Integer)
            )
    })?;
    Some(property_name(&parameter.name))
}

pub(super) fn offset_pagination(operation: &Operation) -> Option<OffsetPagination> {
    let extension = pagination_annotation(operation)?.as_object()?;
    if extension.get("type").and_then(Value::as_str) != Some("offsetLimit") {
        return None;
    }
    let inputs = extension.get("inputs")?.as_array()?;
    Some(OffsetPagination {
        offset_name: pagination_parameter(operation, inputs, "offset")?,
        limit_name: pagination_parameter(operation, inputs, "limit")?,
        results_path: extension
            .get("outputs")?
            .get("results")?
            .as_str()?
            .to_owned(),
    })
}

pub(super) fn cursor_pagination(api: &Api, operation: &Operation) -> Option<CursorPagination> {
    let extension = pagination_annotation(operation)?.as_object()?;
    if extension.get("type").and_then(Value::as_str) != Some("cursor") {
        return None;
    }
    let input = extension
        .get("inputs")
        .and_then(Value::as_array)?
        .iter()
        .find(|input| input.get("type").and_then(Value::as_str) == Some("cursor"))?
        .as_object()?;
    let parameter_name = input.get("name")?.as_str()?;
    let input = match input.get("in").and_then(Value::as_str) {
        None | Some("parameters") => operation
            .parameters
            .iter()
            .find(|parameter| {
                parameter.name == parameter_name
                    && matches!(parameter.location.as_str(), "query" | "header" | "path")
                    && parameter
                        .schema
                        .as_ref()
                        .is_some_and(|schema| matches!(schema.kind, SchemaKind::String))
                    // Optional path parameters are invalid in OpenAPI and
                    // cannot be made safe by a generated continuation.
                    && (parameter.location != "path" || parameter.required)
            })
            .map(|parameter| CursorInput::Parameter(property_name(&parameter.name)))?,
        Some("requestBody") if request_body_has_json_field(api, operation, parameter_name) => {
            CursorInput::Body(parameter_name.to_owned())
        }
        _ => return None,
    };
    Some(CursorPagination {
        input,
        next_cursor_path: extension
            .get("outputs")?
            .get("nextCursor")?
            .as_str()?
            .to_owned(),
    })
}

pub(super) fn url_pagination(operation: &Operation) -> Option<UrlPagination> {
    if operation_is_sse_response(operation) || operation_is_binary_response(operation) {
        return None;
    }
    let extension = pagination_annotation(operation)?.as_object()?;
    if extension.get("type").and_then(Value::as_str) != Some("url") {
        return None;
    }
    Some(UrlPagination {
        next_url_path: extension
            .get("outputs")?
            .get("nextUrl")?
            .as_str()?
            .to_owned(),
    })
}

/// A body cursor is emitted only for a required JSON body with a declared
/// top-level field. This preserves the caller's generated request type and
/// avoids guessing nested, form, binary, or absent body behaviour.
pub(super) fn request_body_has_json_field(api: &Api, operation: &Operation, name: &str) -> bool {
    let Some(body) = operation.request_body.as_ref().filter(|body| body.required) else {
        return false;
    };
    let Some(media) = body.media_types.first() else {
        return false;
    };
    if !(media.content_type == "application/json" || media.content_type.ends_with("+json")) {
        return false;
    }
    let Some(schema) = media.schema.as_ref() else {
        return false;
    };
    matches!(
        &resolve_schema(api, schema).kind,
        SchemaKind::Object { fields, .. } if fields.iter().any(|field| field.name == name)
    )
}

pub(super) fn resolve_schema<'a>(api: &'a Api, schema: &'a SchemaValue) -> &'a SchemaValue {
    let mut current = schema;
    for _ in 0..16 {
        let SchemaKind::Reference { reference } = &current.kind else {
            break;
        };
        let Some(name) = reference.rsplit('/').next() else {
            break;
        };
        let Some(next) = api.schemas.iter().find(|candidate| candidate.name == name) else {
            break;
        };
        current = &next.value;
    }
    current
}
