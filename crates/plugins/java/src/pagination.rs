//! Java pagination declarations and generated continuation methods.

use super::*;

/// Pagination replays the normal generated operation. Cursor fields can live
/// in query, headers, or a required path parameter because the operation keeps
/// ownership of serialization, auth, retries, hooks, and error handling.
#[derive(Clone, Debug)]
pub(super) enum JavaPagination {
    Url {
        next_url_path: String,
    },
    Cursor {
        field: String,
        next_cursor_path: String,
    },
    OffsetLimit {
        step: JavaOffsetStep,
        limit_field: Option<String>,
        results_path: Option<String>,
        num_pages_path: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub(super) enum JavaOffsetStep {
    Page { field: String },
    Offset { field: String },
}

pub(super) fn java_pagination(operation: &Operation) -> Option<JavaPagination> {
    if !matches!(response_surface(operation), ResponseSurface::Json(_)) {
        return None;
    }
    let extension = poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))?
        .as_object()?;
    let inputs = extension.get("inputs")?.as_array()?;
    let outputs = extension.get("outputs")?.as_object()?;
    for (role, selector) in outputs {
        if matches!(
            role.as_str(),
            "nextUrl" | "nextCursor" | "results" | "numPages"
        ) {
            poolster_core::pagination::Selector::parse(selector.as_str()?).ok()?;
        }
    }
    match extension.get("type").and_then(Value::as_str) {
        Some("url") => Some(JavaPagination::Url {
            next_url_path: outputs.get("nextUrl")?.as_str()?.to_owned(),
        }),
        Some("cursor") => Some(JavaPagination::Cursor {
            field: java_cursor_parameter_field(operation, inputs)?,
            next_cursor_path: outputs.get("nextCursor")?.as_str()?.to_owned(),
        }),
        Some("offsetLimit" | "page") => {
            let page = java_pagination_query_field(operation, inputs, "page", true);
            let offset = java_pagination_query_field(operation, inputs, "offset", true);
            let step = match (page, offset) {
                (Some(field), _) => JavaOffsetStep::Page { field },
                (None, Some(field)) => JavaOffsetStep::Offset { field },
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
            match &step {
                JavaOffsetStep::Page { .. }
                    if num_pages_path.is_none() && results_path.is_none() =>
                {
                    return None;
                }
                JavaOffsetStep::Offset { .. } if results_path.is_none() => return None,
                _ => {}
            }
            Some(JavaPagination::OffsetLimit {
                step,
                limit_field: java_pagination_query_field(operation, inputs, "limit", true),
                results_path,
                num_pages_path,
            })
        }
        _ => None,
    }
}

fn java_cursor_parameter_field(operation: &Operation, inputs: &[Value]) -> Option<String> {
    let declared = inputs
        .iter()
        .find(|input| input.get("type").and_then(Value::as_str) == Some("cursor"))?
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
    Some(parameter_name(parameter))
}

/// Find an optional scalar query parameter that the generated Java record can
/// copy into the next request. The extension's `in: parameters` is only a
/// pointer to an actual OpenAPI query parameter; it is never enough by itself.
fn java_pagination_query_field(
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
        parameter.name == name
            && parameter.location == "query"
            && (!parameter.required
                || poolster_core::poolster_extension(&operation.annotations, "pagination")
                    .or_else(|| operation.annotations.get("x-speakeasy-pagination"))
                    .and_then(|extension| extension.get("type"))
                    .and_then(Value::as_str)
                    == Some("page"))
    })?;
    match (
        integer,
        parameter.schema.as_ref().map(|schema| &schema.kind),
    ) {
        (false, Some(SchemaKind::String)) | (true, Some(SchemaKind::Integer)) => {
            Some(field_name(name))
        }
        _ => None,
    }
}

/// Emit a lazy `Iterable` rather than a transport-specific publisher. Each
/// call invokes the normal generated operation, keeping authentication,
/// headers, retries, hooks, error mapping, and request serialization intact.
pub(super) fn render_pagination_operation(
    output: &mut String,
    operation: &Operation,
    pagination: &JavaPagination,
) {
    if let JavaPagination::Url { next_url_path } = pagination {
        render_url_pagination_operation(output, operation, next_url_path);
        return;
    }
    let operation_name = type_name(&operation.id);
    let request_name = format!("{operation_name}Request");
    let method = method_name(&operation.id);
    let response = match response_surface(operation) {
        ResponseSurface::Json(schema) => operation_response_type(schema),
        _ => return,
    };
    let parameters = operation_parameters(operation);
    let has_input = !parameters.is_empty() || request_body_schema(operation).is_some();
    if !has_input {
        return;
    }
    let pages_method = format!("{method}Pages");
    let (field, continuation) = match pagination {
        JavaPagination::Url { .. } => unreachable!("URL pagination returns above"),
        JavaPagination::Cursor {
            field,
            next_cursor_path,
        } => {
            let copy = format!("{request_name}With{}", type_name(field));
            (
                field.as_str(),
                format!(
                    "                var cursor = poolsterJsonPath(mapper.valueToTree(page), {next_cursor_path:?});\n                if (cursor == null || !cursor.isTextual() || cursor.asText().isEmpty() || cursor.asText().equals(current.{field}())) {{ done = true; return page; }}\n                current = {copy}(current, cursor.asText());\n                return page;"
                ),
            )
        }
        JavaPagination::OffsetLimit {
            step,
            limit_field,
            results_path,
            num_pages_path,
        } => match step {
            JavaOffsetStep::Page { field } => {
                let copy = format!("{request_name}With{}", type_name(field));
                if num_pages_path.is_none() {
                    let results = results_path.as_deref().expect("validated page results");
                    let limit = limit_field
                        .as_deref()
                        .map_or_else(|| "null".to_owned(), |limit| format!("current.{limit}()"));
                    (
                        field.as_str(),
                        format!(
                            "                var results = poolsterJsonPath(mapper.valueToTree(page), {results:?});\n                Long limit = {limit};\n                if (results == null || !results.isArray() || results.size() == 0 || (limit != null && results.size() < limit) || current.{field}() == Long.MAX_VALUE) {{ done = true; return page; }}\n                current = {copy}(current, current.{field}() + 1L);\n                return page;"
                        ),
                    )
                } else {
                    (
                        field.as_str(),
                        format!(
                            "                var currentValue = current.{field}();\n                var numPages = poolsterJsonPath(mapper.valueToTree(page), {:?});\n                if (currentValue == null || currentValue == Long.MAX_VALUE || numPages == null || !numPages.isIntegralNumber() || !numPages.canConvertToLong()) {{ done = true; return page; }}\n                var nextValue = currentValue + 1L;\n                if (nextValue > numPages.asLong()) {{ done = true; return page; }}\n                current = {copy}(current, nextValue);\n                return page;",
                            num_pages_path
                                .as_deref()
                                .expect("validated page pagination")
                        ),
                    )
                }
            }
            JavaOffsetStep::Offset { field } => {
                let limit = limit_field
                    .as_deref()
                    .map_or_else(|| "null".to_owned(), |limit| format!("current.{limit}()"));
                let copy = format!("{request_name}With{}", type_name(field));
                (
                    field.as_str(),
                    format!(
                        "                var currentValue = current.{field}();\n                var results = poolsterJsonPath(mapper.valueToTree(page), {:?});\n                if (currentValue == null || results == null || !results.isArray()) {{ done = true; return page; }}\n                var resultCount = results.size();\n                Long limit = {limit};\n                if (resultCount == 0 || (limit != null && resultCount < limit)) {{ done = true; return page; }}\n                if (currentValue > Long.MAX_VALUE - resultCount) {{ done = true; return page; }}\n                current = {copy}(current, currentValue + resultCount);\n                return page;",
                        results_path
                            .as_deref()
                            .expect("validated offset pagination")
                    ),
                )
            }
        },
    };
    let field_type = operation_parameters(operation)
        .into_iter()
        .find(|parameter| parameter_name(parameter) == field)
        .map(parameter_type)
        .expect("validated pagination field is an operation parameter");
    let initial = match pagination {
        JavaPagination::OffsetLimit {
            step, limit_field, ..
        } => {
            let default = if matches!(step, JavaOffsetStep::Page { .. }) {
                1
            } else {
                0
            };
            let copy = format!("{request_name}With{}", type_name(field));
            let mut checks = format!(
                "        if (input.{field}() != null && input.{field}() < 0) throw new IllegalArgumentException(\"pagination must be nonnegative\");\n"
            );
            if let Some(limit) = limit_field {
                checks.push_str(&format!("        if (input.{limit}() != null && input.{limit}() <= 0) throw new IllegalArgumentException(\"pagination limit must be positive\");\n"));
            }
            (
                checks,
                format!("input.{field}() == null ? {copy}(input, {default}L) : input"),
            )
        }
        _ => (String::new(), "input".into()),
    };
    let checks = initial.0;
    let initial = initial.1;
    let _ = writeln!(
        output,
        "    /** Lazily fetches normal response pages using this operation's declared pagination contract. */\n    public java.lang.Iterable<{response}> {pages_method}({request_name} input) {{\n        Objects.requireNonNull(input, \"input\");\n{checks}        return () -> new java.util.Iterator<>() {{\n            private {request_name} current = {initial};\n            private boolean done;\n            private int pageCount;\n\n            @Override public boolean hasNext() {{ return !done; }}\n\n            @Override public {response} next() {{\n                if (done) throw new java.util.NoSuchElementException();\n                if (++pageCount >= 10000) done = true;\n                var page = {method}(current);\n{continuation}\n            }}\n        }};\n    }}\n"
    );
    render_pagination_request_copy(output, operation, &request_name, field, &field_type);
}

fn render_pagination_request_copy(
    output: &mut String,
    operation: &Operation,
    request_name: &str,
    target_field: &str,
    target_type: &str,
) {
    let mut values: Vec<String> = operation_parameters(operation)
        .iter()
        .map(|parameter| {
            let field = parameter_name(parameter);
            if field == target_field {
                "value".to_owned()
            } else {
                format!("input.{field}()")
            }
        })
        .collect();
    if request_body_schema(operation).is_some() {
        values.push("input.body()".to_owned());
    }
    let copy_name = format!("{request_name}With{}", type_name(target_field));
    let _ = writeln!(
        output,
        "    private static {request_name} {copy_name}({request_name} input, {target_type} value) {{\n        return new {request_name}({});\n    }}\n",
        values.join(", ")
    );
}

/// URL pagination has no mutable input field: after the first ordinary call,
/// the API-supplied continuation is the complete route and query.  We retain
/// the generated request's headers and body, and use a private same-origin
/// transport path rather than exposing a raw URL override to SDK consumers.
fn render_url_pagination_operation(
    output: &mut String,
    operation: &Operation,
    next_url_path: &str,
) {
    let operation_name = type_name(&operation.id);
    let request_name = format!("{operation_name}Request");
    let method = method_name(&operation.id);
    let ResponseSurface::Json(schema) = response_surface(operation) else {
        return;
    };
    let response = operation_response_type(schema);
    let has_input =
        !operation_parameters(operation).is_empty() || request_body_schema(operation).is_some();
    if !has_input {
        return;
    }
    let headers = operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "header")
        .map(|parameter| {
            let accessor = format!("input.{}()", parameter_name(parameter));
            format!(
                "        if ({accessor} != null) headers.put({:?}, String.valueOf({accessor}));\n",
                parameter.name
            )
        })
        .collect::<String>();
    let body = if request_body_schema(operation).is_some() {
        "input.body()"
    } else {
        "null"
    };
    let continuation = format!("{method}FromPaginationUrl");
    let maps_declared_errors = !declared_error_responses(operation).is_empty();
    let error_mapping = if maps_declared_errors {
        let mapper = java_error_mapper_name(operation);
        format!(
            "        }} catch (ApiException error) {{\n            throw {mapper}(error);\n        }}"
        )
    } else {
        String::new()
    };
    let try_open = if maps_declared_errors {
        "        try {\n"
    } else {
        ""
    };
    let _ = writeln!(
        output,
        "    /** Lazily follows same-origin URL pages from this operation's declared contract. */\n    public java.lang.Iterable<{response}> {method}Pages({request_name} input) {{\n        Objects.requireNonNull(input, \"input\");\n        return () -> new java.util.Iterator<>() {{\n            private boolean first = true;\n            private boolean done;\n            private String nextUrl;\n            private int pageCount;\n\n            @Override public boolean hasNext() {{ return !done; }}\n\n            @Override public {response} next() {{\n                if (done) throw new java.util.NoSuchElementException();\n                if (++pageCount >= 10000) done = true;\n                var page = first ? {method}(input) : {continuation}(input, nextUrl);\n                first = false;\n                var next = poolsterJsonPath(mapper.valueToTree(page), {next_url_path:?});\n                if (next == null || !next.isTextual() || next.asText().isEmpty() || next.asText().equals(nextUrl)) done = true;\n                else nextUrl = next.asText();\n                return page;\n            }}\n        }};\n    }}\n\n    /** Private URL continuation that retains generated request policy. */\n    private {response} {continuation}({request_name} input, String paginationUrl) {{\n{try_open}        var headers = new java.util.LinkedHashMap<String, String>();\n{headers}        var response = requestPaginationUrlWithRetry({:?}, paginationUrl, headers, {body});\n        return decode(response, {}.class);\n{error_mapping}\n    }}\n",
        operation.method.as_str(),
        response_class(schema),
    );
}
