use super::*;

/// Poolster routes Server-Sent Events through the event-stream client helper,
/// selected from the response media type rather than an operation-name rule.
pub(crate) fn render_event_stream_operation(
    operation: &Operation,
    throw_on_error: bool,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> String {
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str().replace('\'', "\\'");
    let mut metadata = String::new();
    if let Some(parameter) = operation
        .parameters
        .iter()
        .find(|p| p.location == "querystring")
    {
        let content_type = parameter
            .annotations
            .get("poolster.parameter_content")
            .and_then(Value::as_array)
            .and_then(|m| m.first())
            .and_then(|m| m.get("content_type"))
            .and_then(Value::as_str)
            .unwrap_or("application/x-www-form-urlencoded");
        metadata.push_str(&format!(
            "      wholeQuery: {{ name: {}, contentType: {} }},\n",
            serde_json::to_string(&parameter.name).expect("parameter name"),
            serde_json::to_string(content_type).expect("media type")
        ));
    }
    if let Ok(content) = poolster_core::openapi32::request_content(operation) {
        if let Some(media) = content.iter().find(|m| {
            !m.prefix_encoding.is_empty()
                || m.item_encoding.is_some()
                || m.encoding.values().any(|e| {
                    !e.encoding.is_empty()
                        || !e.prefix_encoding.is_empty()
                        || e.item_encoding.is_some()
                })
        }) {
            metadata.push_str(&format!(
                "      multipartPlan: {},\n",
                serde_json::to_string(media).expect("multipart plan")
            ));
        }
    }
    if let Some(content_type) = request_content_type(operation) {
        metadata.push_str(&format!(
            "      contentType: {{ request: '{content_type}' }},\n"
        ));
    }
    if let Some(styles) = render_parameter_styles(operation) {
        metadata.push_str(&format!("      styles: {styles},\n"));
    }
    if let Some(form_encodings) = render_form_encodings(operation) {
        metadata.push_str(&format!("      formEncodings: {form_encodings},\n"));
    }
    if let Some(security) = render_security(operation, security_schemes) {
        metadata.push_str(&format!("      security: {security},\n"));
    }
    let options_default = if operation
        .parameters
        .iter()
        .any(|parameter| parameter.required)
        || operation.request_body.is_some()
    {
        ""
    } else {
        " = {}"
    };
    format!(
        "{ESLINT_HEADER}import type {{ Options, EventStreamResult, SuccessOf }} from './.poolster/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, toEventStream }} from './.poolster/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError>{options_default},\n): Promise<EventStreamResult<SuccessOf<{type_name}Responses>>> {{\n  const {{ client: request = client, ...config }} = options\n\n  return toEventStream<SuccessOf<{type_name}Responses>>(\n    request({{\n      method: '{method}',\n      url: '{}',\n      responseType: 'stream',\n{metadata}      ...config,\n      throwOnError: config.throwOnError ?? {throw_on_error},\n    }}),\n  )\n}}\n",
        operation.path,
    )
}

pub(crate) fn request_content_type(operation: &Operation) -> Option<&str> {
    operation
        .request_body
        .as_ref()?
        .media_types
        .iter()
        .find_map(|media_type| {
            (media_type.content_type.starts_with("multipart/")
                || matches!(
                    media_type.content_type.as_str(),
                    "application/x-www-form-urlencoded"
                        | "application/json-seq"
                        | "application/x-ndjson"
                        | "application/ndjson"
                        | "application/jsonl"
                ))
            .then_some(media_type.content_type.as_str())
        })
}

pub(crate) fn is_event_stream(operation: &Operation) -> bool {
    operation.responses.iter().any(|response| {
        response
            .media_types
            .iter()
            .any(|media_type| media_type.content_type == "text/event-stream")
    })
}
