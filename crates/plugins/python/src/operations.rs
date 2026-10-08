use super::*;

pub(super) fn render_operation(api: &Api, operation: &Operation) -> String {
    let name = python_identifier(&snake_case(&operation.id));
    let response = response_type(operation);
    let has_url_pagination = url_pagination(operation).is_some();
    let mut args = Vec::new();
    for parameter in &operation.parameters {
        let field_type = if parameter.location == "querystring" {
            "str".to_owned()
        } else {
            parameter
                .schema
                .as_ref()
                .map(python_type)
                .unwrap_or_else(|| "Any".into())
        };
        let parameter_name = python_parameter_identifier(operation, parameter);
        if parameter.required {
            args.push(format!("{parameter_name}: {field_type}"));
        } else {
            args.push(format!("{parameter_name}: {field_type} | None = None"));
        }
    }
    if let Some(body) = &operation.request_body {
        let field_type = request_body_type(operation);
        if body.required {
            args.push(format!("body: {field_type}"));
        } else {
            args.push(format!("body: {field_type} | None = None"));
        }
    }

    if has_url_pagination {
        // The continuation is intentionally private to the generated client:
        // public callers use `{operation}_pages`, while this preserves the
        // ordinary operation's complete serialization/auth path.
        args.push("_poolster_pagination_url: str | None = None".into());
    }
    let signature = if args.is_empty() {
        String::new()
    } else {
        format!(", *, {}", args.join(", "))
    };
    let mut output = format!("    def {name}(self{signature}) -> {response}:\n");
    if let Some(policy) = idempotency_annotation(operation) {
        if policy
            .get("auto_generate")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            if let Some(parameter) = policy
                .get("parameter_name")
                .and_then(serde_json::Value::as_str)
            {
                let variable = operation
                    .parameters
                    .iter()
                    .find(|item| item.name == parameter && item.location == "header")
                    .map(|item| python_parameter_identifier(operation, item))
                    .unwrap_or_else(|| python_identifier(parameter));
                let _ = writeln!(
                    output,
                    "        if {variable} is None:\n            {variable} = str(uuid4())"
                );
            }
        }
    }
    for parameter in operation
        .parameters
        .iter()
        .filter(|p| p.location != "querystring")
    {
        if let Ok(content) = poolster_core::openapi32::parameter_content(parameter) {
            if let Some(media) = content.first() {
                let name = python_parameter_identifier(operation, parameter);
                let value = if media
                    .content_type
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .contains("json")
                {
                    format!(
                        "_poolster_json.dumps(to_wire({name}), separators=(\",\", \":\"), allow_nan=False)"
                    )
                } else {
                    format!("str({name})")
                };
                let serialized = if parameter.required && media.content_type.contains("json") {
                    value
                } else {
                    format!("{value} if {name} is not None else None")
                };
                let _ = writeln!(
                    output,
                    "        import json as _poolster_json\n        _poolster_content_{name} = {serialized}"
                );
            }
        }
    }
    let _ = writeln!(output, "        _poolster_path = {:?}", operation.path);
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "path")
    {
        let value = python_parameter_value_name(operation, parameter);
        let _ = writeln!(
            output,
            "        _poolster_path = _poolster_path.replace({:?}, quote(str({value}), safe=\"\"))",
            format!("{{{}}}", parameter.name)
        );
    }
    let query = operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "query")
        .collect::<Vec<_>>();
    if query.is_empty() {
        output.push_str("        _poolster_query: dict[str, Any] | None = None\n");
    } else {
        output.push_str("        _poolster_query: dict[str, Any] | None = {}\n");
        for parameter in query {
            let value = python_parameter_value_name(operation, parameter);
            let _ = writeln!(
                output,
                "        if {value} is not None:\n            _poolster_query[{name:?}] = {value}",
                name = parameter.name
            );
        }
    }
    for parameter in operation
        .parameters
        .iter()
        .filter(|p| p.location == "querystring")
    {
        let value = python_parameter_value_name(operation, parameter);
        let _ = writeln!(
            output,
            "        if {value} is not None:\n            if any(character in {value} for character in (\"#\", \"?\", \"\\r\", \"\\n\")):\n                raise ValueError(\"Whole-query value must be serialized without a URL fragment or query delimiter\")\n            _poolster_path += (\"&\" if \"?\" in _poolster_path else \"?\") + {value}"
        );
    }
    let headers = operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "header")
        .collect::<Vec<_>>();
    if headers.is_empty() {
        output.push_str("        _poolster_headers: dict[str, str] | None = None\n");
    } else {
        output.push_str("        _poolster_headers: dict[str, str] | None = {}\n");
        for parameter in headers {
            let value = python_parameter_value_name(operation, parameter);
            let _ = writeln!(
                output,
                "        if {value} is not None:\n            _poolster_headers[{name:?}] = str({value})",
                name = parameter.name
            );
        }
    }
    for parameter in operation
        .parameters
        .iter()
        .filter(|p| p.location == "cookie")
    {
        let value = python_parameter_value_name(operation, parameter);
        let _ = writeln!(
            output,
            "        if {value} is not None:\n            _poolster_headers = _poolster_headers or {{}}\n            _poolster_cookie = {:?} + \"=\" + quote(str({value}), safe=\"\")\n            _poolster_headers[\"Cookie\"] = (_poolster_headers[\"Cookie\"] + \"; \" if _poolster_headers.get(\"Cookie\") else \"\") + _poolster_cookie",
            parameter.name
        );
    }
    if let Ok(content) = poolster_core::openapi32::request_content(operation) {
        if let Some(media) = content.iter().find(|m| {
            m.content_type.starts_with("multipart/")
                && (!m.prefix_encoding.is_empty()
                    || m.item_encoding.is_some()
                    || m.encoding.values().any(|e| {
                        !e.encoding.is_empty()
                            || !e.prefix_encoding.is_empty()
                            || e.item_encoding.is_some()
                    }))
        }) {
            let plan = serde_json::to_string(media).expect("multipart plan");
            let _ = writeln!(
                output,
                "        import json as _poolster_json\n        _poolster_plan = _poolster_json.loads({plan:?})\n        body = (body if isinstance(body, MultipartBody) else MultipartBody.positional(body) if isinstance(body, list) else MultipartBody(body)).with_encoding(_poolster_plan)"
            );
        }
    }
    let body = if operation.request_body.is_some() {
        "body=body"
    } else {
        "body=None"
    };
    let body_kind = operation_body_kind(operation);
    let error_types = operation_error_types(api, operation);
    let retryable = if matches!(analyze_operation(operation, None).retry, RetryClass::Unsafe)
        && idempotency_annotation(operation).is_none()
    {
        "False"
    } else {
        "True"
    };
    let idempotency_argument = idempotency_annotation(operation)
        .and_then(|policy| policy.get("header"))
        .and_then(serde_json::Value::as_str)
        .map(|header| format!(", idempotency_header={header:?}"))
        .unwrap_or_default();
    let pagination_argument = if has_url_pagination && !operation_is_sse_response(operation) {
        ", pagination_url=_poolster_pagination_url"
    } else {
        ""
    };
    let transport = if operation_is_sse_response(operation) {
        "_event_stream"
    } else {
        "_request"
    };
    let response_argument = if transport == "_request" {
        format!(", response_operation={:?}", operation.id)
    } else {
        String::new()
    };
    let _ = writeln!(
        output,
        "        result = self.{transport}({:?}, _poolster_path, query=_poolster_query, headers=_poolster_headers, {body}, body_kind={body_kind:?}, error_types={error_types}, retryable={retryable}{pagination_argument}{response_argument}{idempotency_argument})",
        operation.method.as_str()
    );
    if let Some(model) = response_object_model(api, operation) {
        let _ = writeln!(
            output,
            "        return {model}.from_dict(result) if isinstance(result, dict) else cast({model}, result)\n"
        );
    } else {
        let _ = writeln!(output, "        return cast({response}, result)\n");
    }
    output
}

/// Keeps a namespaced resource method's public signature identical to its
/// corresponding direct `Client` method. Resource facades are part of the
/// public Python API, so forwarding via `Any` would discard IDE completion
/// and static type checking at the point callers actually use the SDK.
pub(super) fn operation_signature(operation: &Operation) -> (String, Vec<String>) {
    let mut args = Vec::new();
    let mut names = Vec::new();
    for parameter in &operation.parameters {
        let field_type = parameter
            .schema
            .as_ref()
            .map(python_type)
            .unwrap_or_else(|| "Any".into());
        let name = python_parameter_identifier(operation, parameter);
        names.push(name.clone());
        if parameter.required {
            args.push(format!("{name}: {field_type}"));
        } else {
            args.push(format!("{name}: {field_type} | None = None"));
        }
    }
    if let Some(body) = &operation.request_body {
        let field_type = request_body_type(operation);
        names.push("body".into());
        if body.required {
            args.push(format!("body: {field_type}"));
        } else {
            args.push(format!("body: {field_type} | None = None"));
        }
    }
    let signature = if args.is_empty() {
        String::new()
    } else {
        format!(", *, {}", args.join(", "))
    };
    (signature, names)
}

pub(super) fn request_body_type(operation: &Operation) -> String {
    let kind = operation_body_kind(operation);
    if kind == "multipart" {
        return "MultipartBody | dict[str, Any]".into();
    }
    let value = if kind == "binary" || kind == "binary_or_multipart" {
        "bytes".into()
    } else {
        operation
            .request_body
            .as_ref()
            .and_then(|body| {
                body.media_types
                    .iter()
                    .find(|media| media.content_type != "multipart/form-data")
            })
            .and_then(|media| media.schema.as_ref())
            .map(python_type)
            .unwrap_or_else(|| "Any".into())
    };
    if kind.ends_with("_or_multipart") {
        format!("{value} | MultipartBody")
    } else {
        value
    }
}

/// Mixed-media inputs use the ordinary representation unless explicitly wrapped.
pub(super) fn operation_body_kind(operation: &Operation) -> &'static str {
    let Some(body) = operation.request_body.as_ref() else {
        return "json";
    };
    let multipart = body
        .media_types
        .iter()
        .any(|media| media.content_type.starts_with("multipart/"));
    let other = body
        .media_types
        .iter()
        .find(|media| !media.content_type.starts_with("multipart/"));
    match (multipart, other.map(|media| media.content_type.as_str())) {
        (false, Some("application/json-seq")) => "application/json-seq",
        (true, Some("application/json-seq")) => "application/json-seq_or_multipart",
        (false, Some("application/jsonl")) => "application/jsonl",
        (true, Some("application/jsonl")) => "application/jsonl_or_multipart",
        (false, Some("application/ndjson")) => "application/ndjson",
        (true, Some("application/ndjson")) => "application/ndjson_or_multipart",
        (false, Some("application/x-ndjson")) => "application/x-ndjson",
        (true, Some("application/x-ndjson")) => "application/x-ndjson_or_multipart",
        (true, None) => "multipart",
        (true, Some("application/x-www-form-urlencoded")) => "form_or_multipart",
        (true, Some("application/octet-stream")) => "binary_or_multipart",
        (true, Some(_)) => "json_or_multipart",
        (false, Some("application/x-www-form-urlencoded")) => "form",
        (false, Some("application/octet-stream")) => "binary",
        _ => "json",
    }
}
