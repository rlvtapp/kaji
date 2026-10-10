//! HTTP pagination contract rendering.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResponseKind {
    Empty,
    Json,
    Text,
    Binary,
    ServerSentEvents,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestMediaKind<'a> {
    Json,
    Unknown,
    Multipart,
    Unsupported(&'a str),
}

/// Pagination that can be expressed without changing the generated operation
/// contract. Every continuation re-enters the normal generated operation, so
/// path escaping, header serialization, auth, retries, and hooks stay intact.
#[derive(Clone, Debug)]
pub(crate) enum RustPagination {
    Url {
        next_url_path: String,
    },
    Page {
        field: RustPaginationField,
        limit: Option<RustPaginationField>,
        results_path: String,
    },
    Cursor {
        field: RustPaginationField,
        next_cursor_path: String,
    },
    OffsetLimit {
        step: RustOffsetStep,
        limit_field: Option<String>,
        results_path: Option<String>,
        num_pages_path: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum RustOffsetStep {
    Page { field: String },
    Offset { field: String },
}

/// A scalar request field Poolster can replace without reconstructing the request.
/// Required fields deliberately remain required: the caller supplies the first
/// value and the pager replaces it only after receiving a declared next value.
#[derive(Clone, Debug)]
pub(crate) struct RustPaginationField {
    pub(crate) name: String,
    pub(crate) optional: bool,
}

/// Read Poolster's native pagination declaration and its Speakeasy-compatible
/// spelling. Header and path cursor fields are safe because the generated
/// operation remains the only place that serializes them. OpenAPI requires
/// path parameters to be required, so Poolster never invents a first path value.
pub(crate) fn rust_pagination(operation: &Operation) -> Option<RustPagination> {
    if response_kind(operation) != ResponseKind::Json {
        return None;
    }
    let extension = poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))?
        .as_object()?;
    let empty_inputs = Vec::new();
    let inputs = extension
        .get("inputs")
        .and_then(Value::as_array)
        .unwrap_or(&empty_inputs);
    let outputs = extension.get("outputs")?.as_object()?;
    match extension.get("type").and_then(Value::as_str) {
        Some("url") => {
            if operation.request_body.is_some() {
                return None;
            }
            let path = outputs.get("nextUrl")?.as_str()?;
            poolster_core::pagination::Selector::parse(path).ok()?;
            Some(RustPagination::Url {
                next_url_path: path.to_owned(),
            })
        }
        Some("cursor") => {
            let mut scalar_operation = operation.clone();
            // This catalog-independent projection validates scalar cursor inputs
            // and portable selectors; response references remain renderer-owned.
            scalar_operation.responses.clear();
            let plan = poolster_core::pagination::normalize_pagination(
                &Api::default(),
                &scalar_operation,
                None,
            )
            .ok()
            .flatten()?;
            let path = plan.continuation?.expression;
            let field = rust_cursor_parameter_field(operation, inputs, "cursor")?;
            Some(RustPagination::Cursor {
                field,
                next_cursor_path: path,
            })
        }
        Some("page") => {
            let mut projected = operation.clone();
            projected.responses.clear();
            let plan =
                poolster_core::pagination::normalize_pagination(&Api::default(), &projected, None)
                    .ok()??;
            let scalar_field = |role: &str| -> Option<RustPaginationField> {
                let input = plan.inputs.iter().find(|input| input.role == role)?;
                if input.location == "requestBody" {
                    return None;
                }
                let parameter = operation.parameters.iter().find(|p| p.name == input.name)?;
                if !matches!(parameter.schema.as_ref()?.kind, SchemaKind::Integer) {
                    return None;
                }
                Some(RustPaginationField {
                    name: rust_field_name(&input.name),
                    optional: !input.required,
                })
            };
            let field = scalar_field("page")?;
            let limit = if plan.inputs.iter().any(|input| input.role == "limit") {
                Some(scalar_field("limit")?)
            } else {
                None
            };
            Some(RustPagination::Page {
                field,
                limit,
                results_path: plan.results?.expression,
            })
        }
        Some("offsetLimit") => {
            let page = rust_pagination_query_field(operation, inputs, "page", true);
            let offset = rust_pagination_query_field(operation, inputs, "offset", true);
            let step = match (page, offset) {
                (Some(field), _) => RustOffsetStep::Page { field },
                (None, Some(field)) => RustOffsetStep::Offset { field },
                (None, None) => return None,
            };
            let results_path = outputs
                .get("results")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let num_pages_path = outputs
                .get("numPages")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if matches!(&step, RustOffsetStep::Page { .. }) && num_pages_path.is_none() {
                // Page numbers without a declared final page are ambiguous;
                // don't replace that ambiguity with a guessed empty-result rule.
                return None;
            }
            if matches!(&step, RustOffsetStep::Offset { .. }) && results_path.is_none() {
                return None;
            }
            Some(RustPagination::OffsetLimit {
                step,
                limit_field: rust_pagination_query_field(operation, inputs, "limit", true),
                results_path,
                num_pages_path,
            })
        }
        _ => None,
    }
}

/// Finds a declared string cursor in a query, header, or path parameter.
/// A path cursor must be required under OpenAPI; optional path parameters are
/// invalid and are intentionally rejected even if they reach the AST.
pub(crate) fn rust_cursor_parameter_field(
    operation: &Operation,
    inputs: &[Value],
    kind: &str,
) -> Option<RustPaginationField> {
    let declared = inputs
        .iter()
        .find(|input| input.get("type").and_then(Value::as_str) == Some(kind))?
        .as_object()?;
    if !matches!(
        declared.get("in").and_then(Value::as_str),
        Some("parameters") | None
    ) {
        return None;
    }
    let name = declared.get("name")?.as_str()?;
    let parameter = operation.parameters.iter().find(|parameter| {
        parameter.name == name
            && matches!(parameter.location.as_str(), "query" | "header" | "path")
            && parameter
                .schema
                .as_ref()
                .is_some_and(|schema| matches!(schema.kind, SchemaKind::String))
    })?;
    if parameter.location == "path" && !parameter.required {
        return None;
    }
    Some(RustPaginationField {
        name: parameter_name(parameter),
        optional: !parameter.required,
    })
}

/// Returns the generated Rust request field for a compatible optional query
/// parameter. `integer` accepts JSON Schema's common integer representation;
/// cursor inputs require strings because OpenAPI cursor output is a string.
pub(crate) fn rust_pagination_query_field(
    operation: &Operation,
    inputs: &[Value],
    kind: &str,
    integer: bool,
) -> Option<String> {
    let declared = inputs
        .iter()
        .find(|input| input.get("type").and_then(Value::as_str) == Some(kind))?
        .as_object()?;
    if !matches!(
        declared.get("in").and_then(Value::as_str),
        Some("parameters") | None
    ) {
        return None;
    }
    let name = declared.get("name")?.as_str()?;
    let parameter = operation.parameters.iter().find(|parameter| {
        parameter.name == name && parameter.location == "query" && !parameter.required
    })?;
    let kind = parameter.schema.as_ref().map(|schema| &schema.kind)?;
    match (integer, kind) {
        (false, SchemaKind::String) | (true, SchemaKind::Integer) => Some(rust_field_name(name)),
        _ => None,
    }
}
