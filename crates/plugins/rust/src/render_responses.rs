//! HTTP responses rendering.
use super::*;

/// The Rust target intentionally preserves binary and SSE responses instead
/// of attempting to deserialize them as JSON. SSE is returned as a live
/// `reqwest::Response`, which lets callers select their preferred event-stream
/// parser without Poolster silently buffering an unbounded stream.
pub(crate) fn response_kind(operation: &Operation) -> ResponseKind {
    let media = operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .and_then(|response| {
            response
                .media_types
                .iter()
                .find(|media| {
                    media.content_type == "application/json"
                        || media.content_type.ends_with("+json")
                })
                .or_else(|| response.media_types.first())
        });
    let Some(content_type) = media.map(|media| media.content_type.as_str()) else {
        return ResponseKind::Empty;
    };
    match content_type {
        "text/event-stream" => ResponseKind::ServerSentEvents,
        content_type if content_type.starts_with("multipart/") => ResponseKind::Binary,
        "application/octet-stream" | "application/pdf" | "image/png" | "image/jpeg" => {
            ResponseKind::Binary
        }
        content_type if content_type.starts_with("text/") => ResponseKind::Text,
        _ => ResponseKind::Json,
    }
}

pub(crate) fn operation_response_type(operation: &Operation) -> String {
    match response_kind(operation) {
        ResponseKind::Empty => "()".into(),
        ResponseKind::Binary => "Vec<u8>".into(),
        ResponseKind::ServerSentEvents => "reqwest::Response".into(),
        ResponseKind::Text => "String".into(),
        ResponseKind::Json => operation
            .success_schema()
            .map(rust_type)
            .unwrap_or_else(|| "serde_json::Value".into()),
    }
}

/// Determines whether a generated operation may participate in automatic
/// retries. POST is deliberately opt-in through a declared idempotency key;
/// optional keys only enable retries when the caller supplied one.
pub(crate) fn rust_retry_allowed(operation: &Operation) -> String {
    match operation.method {
        poolster_core::ast::HttpMethod::Get
        | poolster_core::ast::HttpMethod::Head
        | poolster_core::ast::HttpMethod::Options
        | poolster_core::ast::HttpMethod::Trace
        | poolster_core::ast::HttpMethod::Query
        | poolster_core::ast::HttpMethod::Put
        | poolster_core::ast::HttpMethod::Delete => "true".into(),
        poolster_core::ast::HttpMethod::Post | poolster_core::ast::HttpMethod::Patch | poolster_core::ast::HttpMethod::Custom(_) => operation
            .parameters
            .iter()
            .find(|parameter| {
                parameter.location == "header"
                    && (parameter.name.eq_ignore_ascii_case("idempotency-key")
                        || poolster_core::idempotency::resolved(operation).is_some_and(|policy| {
                            parameter.name.eq_ignore_ascii_case(&policy.header)
                        }))
            })
            .map(|parameter| {
                if parameter.required {
                    format!(
                        "!input.{}.trim().is_empty()",
                        parameter_name(parameter)
                    )
                } else {
                    format!(
                        "input.{}.as_ref().is_some_and(|key| !poolster_query_value(key).trim().is_empty())",
                        parameter_name(parameter)
                    )
                }
            })
            .unwrap_or_else(|| "false".into()),
    }
}

pub(crate) fn multipart_alternative(operation: &Operation) -> Option<Operation> {
    let body = operation.request_body.as_ref()?;
    if body.media_types.len() < 2
        || !body.media_types.iter().any(|media| {
            media.content_type == "application/json" || media.content_type.ends_with("+json")
        })
    {
        return None;
    }
    let media = body
        .media_types
        .iter()
        .find(|media| media.content_type.starts_with("multipart/"))?
        .clone();
    let mut form = operation.clone();
    form.request_body.as_mut().unwrap().media_types = vec![media];
    Some(form)
}
pub(crate) fn multipart_method_name(
    api: &Api,
    operation: &Operation,
    options: &RenderOptions,
) -> String {
    let mut name = format!("{}_multipart", direct_method_name(operation, options));
    while api
        .operations
        .iter()
        .any(|other| direct_method_name(other, options) == name)
    {
        name.push_str("_body");
    }
    name
}

pub(crate) fn request_media_kind(operation: &Operation) -> RequestMediaKind<'_> {
    match operation
        .request_body
        .as_ref()
        .and_then(|body| {
            body.media_types
                .iter()
                .find(|media| {
                    media.content_type == "application/json"
                        || media.content_type.ends_with("+json")
                })
                .or_else(|| body.media_types.first())
        })
        .map(|media| media.content_type.as_str())
    {
        None => RequestMediaKind::Unknown,
        Some(content_type)
            if content_type == "application/json"
                || content_type.ends_with("+json")
                || matches!(
                    content_type,
                    "application/x-ndjson"
                        | "application/ndjson"
                        | "application/jsonl"
                        | "application/json-seq"
                ) =>
        {
            RequestMediaKind::Json
        }
        Some(content_type) if content_type.starts_with("multipart/") => RequestMediaKind::Multipart,
        Some(content_type) => RequestMediaKind::Unsupported(content_type),
    }
}

pub(crate) fn operation_error_name(operation: &Operation) -> String {
    format!("{}Error", type_name(&operation.id))
}

pub(crate) fn render_operation_error(operation: &Operation) -> String {
    let error = operation_error_name(operation);
    let mut output = format!(
        "/// Errors returned by `{}`. Declared OpenAPI error bodies are decoded into typed variants.\n#[derive(Debug)]\npub enum {error} {{\n    Transport(reqwest::Error),\n    TokenProvider(TokenProviderError),\n    Decode {{ source: serde_json::Error, response: ApiResponse }},\n    UnsupportedRequestMedia(&'static str),\n",
        operation.id,
    );
    if rust_pagination(operation).is_some() {
        output.push_str("    Pagination(serde_json::Error),\n");
    }
    for response in declared_error_responses(operation) {
        if let Some(body_type) = error_body_type(response) {
            let _ = writeln!(
                output,
                "    {} {{ body: {body_type}, response: ApiResponse }},",
                error_variant_name(&response.status),
            );
        }
    }
    output.push_str("    Unexpected(ApiResponse),\n}\n\n");
    output
}

pub(crate) fn declared_error_responses(
    operation: &Operation,
) -> impl Iterator<Item = &poolster_core::ast::OperationResponse> {
    operation.responses.iter().filter(|response| {
        response.status == "default"
            || response
                .status
                .parse::<u16>()
                .is_ok_and(|status| (400..600).contains(&status))
    })
}

pub(crate) fn error_body_type(response: &poolster_core::ast::OperationResponse) -> Option<String> {
    response
        .media_types
        .first()
        .and_then(|media| media.schema.as_ref())
        .map(rust_type)
}

pub(crate) fn error_variant_name(status: &str) -> String {
    match status {
        "400" => "BadRequest".into(),
        "401" => "Unauthorized".into(),
        "402" => "PaymentRequired".into(),
        "403" => "Forbidden".into(),
        "404" => "NotFound".into(),
        "409" => "Conflict".into(),
        "422" => "UnprocessableEntity".into(),
        "429" => "TooManyRequests".into(),
        "500" => "InternalServerError".into(),
        "501" => "NotImplemented".into(),
        "502" => "BadGateway".into(),
        "503" => "ServiceUnavailable".into(),
        "504" => "GatewayTimeout".into(),
        "default" => "Default".into(),
        status => format!("Status{}", type_name(status)),
    }
}

pub(crate) fn render_error_response(operation: &Operation, error: &str) -> String {
    let mut output = String::from("match status.as_u16() {\n");
    for response in declared_error_responses(operation) {
        let Ok(status) = response.status.parse::<u16>() else {
            continue;
        };
        let Some(body_type) = error_body_type(response) else {
            continue;
        };
        let variant = error_variant_name(&response.status);
        let _ = writeln!(
            output,
            "                {status} => match serde_json::from_slice::<{body_type}>(&body) {{ Ok(decoded_body) => {error}::{variant} {{ body: decoded_body, response: ApiResponse {{ status, headers, body }} }}, Err(_) => {error}::Unexpected(ApiResponse {{ status, headers, body }}) }},"
        );
    }
    let default_body_type = declared_error_responses(operation)
        .find(|response| response.status == "default")
        .and_then(error_body_type);
    if let Some(body_type) = default_body_type {
        let _ = writeln!(
            output,
            "                _ => match serde_json::from_slice::<{body_type}>(&body) {{ Ok(decoded_body) => {error}::Default {{ body: decoded_body, response: ApiResponse {{ status, headers, body }} }}, Err(_) => {error}::Unexpected(ApiResponse {{ status, headers, body }}) }},"
        );
    } else {
        output.push_str(&format!(
            "                _ => {error}::Unexpected(ApiResponse {{ status, headers, body }}),\n"
        ));
    }
    output.push_str("            }");
    output
}
