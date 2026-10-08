use super::*;

/// The Python target intentionally emits a paginator only when a declared
/// cursor maps to a real operation parameter. That keeps the public method
/// type-safe and prevents a generator from inventing a query convention.
#[derive(Clone, Debug)]
pub(super) struct CursorPagination {
    input: CursorInput,
    next_cursor_path: String,
}

#[derive(Clone, Debug)]
pub(super) enum CursorInput {
    Parameter(String),
    Body(String),
}

/// Offset/limit pagination is only generated when both declared inputs map to
/// optional integer operation parameters.  That means a generated iterator
/// can advance without manufacturing an undocumented request shape.
#[derive(Clone, Debug)]
pub(super) struct OffsetPagination {
    offset_name: String,
    limit_name: String,
    results_path: String,
}

/// URL pagination only uses the URL as a same-origin continuation. The
/// generated operation remains responsible for method, auth, headers and body
/// serialization, so a pagination link can never switch credentials or HTTP
/// semantics.
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
    Some(python_parameter_identifier(operation, parameter))
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
                    // An optional path parameter is invalid in OpenAPI. Do
                    // not bless an invalid AST by generating a pager for it.
                    && (parameter.location != "path" || parameter.required)
            })
            .map(|parameter| {
                CursorInput::Parameter(python_parameter_identifier(operation, parameter))
            })?,
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

/// Body cursors require a required JSON body with an explicit top-level field.
/// That lets Poolster clone the caller's generated request type without inventing
/// a nested-pointer, form, binary, or optional-body continuation policy.
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

pub(super) fn render_cursor_paginator(
    api: &Api,
    operation: &Operation,
    pagination: &CursorPagination,
) -> String {
    let name = python_identifier(&snake_case(&operation.id));
    let response = response_type(operation);
    let (signature, arguments) = operation_signature(operation);
    let (state, forwarded, update) = match &pagination.input {
        CursorInput::Parameter(parameter_name) => {
            let forwarded = arguments
                .iter()
                .map(|argument| {
                    if argument == parameter_name {
                        format!("{argument}=poolster_cursor")
                    } else {
                        format!("{argument}={argument}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            (
                format!("        poolster_cursor = {parameter_name}\n"),
                forwarded,
                "            poolster_cursor = next_cursor\n".to_owned(),
            )
        }
        CursorInput::Body(wire_name) => {
            let forwarded = arguments
                .iter()
                .map(|argument| {
                    if argument == "body" {
                        "body=poolster_body".to_owned()
                    } else {
                        format!("{argument}={argument}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            (
                "        poolster_body = body\n".to_owned(),
                forwarded,
                format!(
                    "            poolster_body = _poolster_with_body_value(poolster_body, {wire_name:?}, next_cursor)\n"
                ),
            )
        }
    };
    let call = if forwarded.is_empty() {
        format!("self.{name}()")
    } else {
        format!("self.{name}({forwarded})")
    };
    let _ = api; // keeps this renderer symmetric with ordinary operations.
    format!(
        "    def {name}_pages(self{signature}) -> Iterator[{response}]:\n        \"\"\"Yield declared cursor pages for `{}`.\"\"\"\n{state}        while True:\n            response = {call}\n            yield response\n            next_cursor = _poolster_json_path(response, {:?})\n            if next_cursor is None or next_cursor == \"\":\n                return\n{update}\n",
        operation.id, pagination.next_cursor_path
    )
}

pub(super) fn render_page_paginator(api: &Api, operation: &Operation) -> Option<String> {
    let extension = pagination_annotation(operation)?;
    if extension.get("type").and_then(Value::as_str) != Some("page") {
        return None;
    }
    let plan = poolster_core::pagination::normalize_pagination(api, operation, None).ok()??;
    let page = plan.inputs.iter().find(|input| input.role == "page")?;
    let limit = plan.inputs.iter().find(|input| input.role == "limit");
    let (signature, arguments) = operation_signature(operation);
    let name = python_identifier(&snake_case(&operation.id));
    let page_argument = python_pagination_argument(operation, page);
    let state = if page.location == "requestBody" {
        "        poolster_page = _poolster_json_path(body, PAGE_POINTER)\n        poolster_page = 1 if poolster_page is None else poolster_page\n        poolster_body = _poolster_with_body_value(body, PAGE_WIRE, poolster_page)\n".replace("PAGE_WIRE",&format!("{:?}",page.name)).replace("PAGE_POINTER", &format!("{:?}", format!("/{}", page.name.replace('~',"~0").replace('/',"~1"))))
    } else {
        format!("        poolster_page = 1 if {page_argument} is None else {page_argument}\n")
    };
    let forwarded = arguments
        .iter()
        .map(|arg| {
            if page.location == "requestBody" && arg == "body" {
                "body=poolster_body".into()
            } else if page.location != "requestBody" && arg == &page_argument {
                format!("{arg}=poolster_page")
            } else {
                format!("{arg}={arg}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let update = if page.location == "requestBody" {
        format!(
            "            poolster_body = _poolster_with_body_value(poolster_body, {:?}, poolster_page)\n",
            page.name
        )
    } else {
        String::new()
    };
    let limit = limit
        .map(|input| {
            if input.location == "requestBody" {
                format!(
                    "_poolster_json_path(poolster_body, {:?})",
                    format!("/{}", input.name.replace('~', "~0").replace('/', "~1"))
                )
            } else {
                python_pagination_argument(operation, input)
            }
        })
        .unwrap_or("None".into());
    let results = &plan.results?.expression;
    Some(format!(
        "    def {name}_pages(self{signature}) -> Iterator[{}]:\n{state}        if type(poolster_page) is not int or poolster_page < 0:\n            raise ValueError('page must be a nonnegative integer')\n        if {limit} is not None and (type({limit}) is not int or {limit} <= 0):\n            raise ValueError('limit must be a positive integer')\n        for _poolster_page_count in range(10000):\n            response = self.{name}({forwarded})\n            items = _poolster_json_path(response, {results:?})\n            if not isinstance(items, list):\n                raise ValueError('pagination results must be an array')\n            yield response\n            if not items or (type({limit}) is int and {limit} > 0 and len(items) < {limit}):\n                return\n            poolster_page += 1\n{update}        raise ValueError('pagination exceeded 10000 pages')\n\n",
        response_type(operation)
    ))
}

pub(super) fn render_offset_paginator(
    operation: &Operation,
    pagination: &OffsetPagination,
) -> String {
    let name = python_identifier(&snake_case(&operation.id));
    let response = response_type(operation);
    let (signature, arguments) = operation_signature(operation);
    let forwarded = arguments
        .iter()
        .map(|argument| {
            if argument == &pagination.offset_name {
                format!("{argument}=poolster_offset")
            } else if argument == &pagination.limit_name {
                format!("{argument}=poolster_limit")
            } else {
                format!("{argument}={argument}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let call = if forwarded.is_empty() {
        format!("self.{name}()")
    } else {
        format!("self.{name}({forwarded})")
    };
    format!(
        "    def {name}_pages(self{signature}) -> Iterator[{response}]:\n        \"\"\"Yield declared offset/limit pages for `{}`.\"\"\"\n        poolster_offset = 0 if {} is None else {}\n        poolster_limit = {}\n        while True:\n            response = {call}\n            yield response\n            results = _poolster_json_path(response, {:?})\n            if not isinstance(results, list) or not results:\n                return\n            poolster_offset += len(results)\n            if poolster_limit is not None and len(results) < poolster_limit:\n                return\n\n",
        operation.id,
        pagination.offset_name,
        pagination.offset_name,
        pagination.limit_name,
        pagination.results_path,
    )
}

pub(super) fn render_url_paginator(operation: &Operation, pagination: &UrlPagination) -> String {
    let name = python_identifier(&snake_case(&operation.id));
    let response = response_type(operation);
    let (signature, arguments) = operation_signature(operation);
    let forwarded = arguments
        .iter()
        .map(|argument| format!("{argument}={argument}"))
        .collect::<Vec<_>>();
    let call_args = if forwarded.is_empty() {
        "_poolster_pagination_url=poolster_url".to_owned()
    } else {
        format!(
            "{}, _poolster_pagination_url=poolster_url",
            forwarded.join(", ")
        )
    };
    format!(
        "    def {name}_pages(self{signature}) -> Iterator[{response}]:\n        \"\"\"Yield declared URL pages for `{}` while preserving the generated operation's auth and serialization.\"\"\"\n        poolster_url: str | None = None\n        while True:\n            response = self.{name}({call_args})\n            yield response\n            next_url = _poolster_json_path(response, {:?})\n            if not isinstance(next_url, str) or next_url == \"\":\n                return\n            poolster_url = next_url\n\n",
        operation.id, pagination.next_url_path
    )
}

pub(super) fn python_parameter_value_name(
    operation: &Operation,
    parameter: &poolster_core::OperationParameter,
) -> String {
    let name = python_parameter_identifier(operation, parameter);
    if parameter.location != "querystring"
        && poolster_core::openapi32::parameter_content(parameter)
            .ok()
            .is_some_and(|content| !content.is_empty())
    {
        format!("_poolster_content_{name}")
    } else {
        name
    }
}
