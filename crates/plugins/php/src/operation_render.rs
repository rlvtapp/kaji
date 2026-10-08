use super::*;

pub(super) fn render_operation(
    operation: &Operation,
    namespace: &str,
    named_types: &NamedTypes,
) -> String {
    let response = response_schema(operation);
    let is_sse = operation_is_sse_response(operation);
    let has_url_pagination = url_pagination(operation).is_some();
    let return_type = if is_sse {
        "StreamInterface".into()
    } else if operation_is_binary_response(operation) {
        "string".into()
    } else {
        response
            .map(|schema| php_type(schema, named_types))
            .unwrap_or_else(|| "void".into())
    };
    let return_type = if return_type == "null" {
        "void"
    } else {
        &return_type
    };
    let body_schema = operation
        .request_body
        .as_ref()
        .and_then(|body| body.media_types.first())
        .and_then(|media| media.schema.as_ref());
    let body_parameter = body_schema.map(|schema| poolster_core::OperationParameter {
        name: "body".into(),
        location: "body".into(),
        required: operation
            .request_body
            .as_ref()
            .is_some_and(|body| body.required),
        schema: Some(schema.clone()),
        description: None,
        annotations: BTreeMap::new(),
    });
    let mut parameters = operation.parameters.clone();
    parameters.sort_by_key(|parameter| !parameter.required);
    let mut arguments = Vec::new();
    let mut used = BTreeSet::new();
    if operation.request_body.is_some() {
        used.insert("body".to_owned());
    }
    for parameter in &parameters {
        let variable = unique_name(property_name(&parameter.name), &mut used);
        let type_name = parameter
            .schema
            .as_ref()
            .map(|schema| php_type(schema, named_types))
            .unwrap_or_else(|| "mixed".into());
        let type_name = nullable_type(&type_name, !parameter.required);
        let default = if parameter.required { "" } else { " = null" };
        let declaration = format!("{type_name} ${variable}{default}");
        arguments.push((parameter, variable, declaration));
    }
    if let Some(schema) = body_schema {
        let type_name = if operation_has_multipart(operation) {
            "mixed".into()
        } else {
            php_type(schema, named_types)
        };
        let body_required = operation
            .request_body
            .as_ref()
            .is_some_and(|body| body.required);
        let default = if body_required { "" } else { " = null" };
        let type_name = nullable_type(&type_name, !default.is_empty());
        let body_argument = (
            body_parameter
                .as_ref()
                .expect("body schema has a parameter"),
            "body".into(),
            format!("{type_name} $body{default}"),
        );
        if body_required {
            let first_optional = arguments
                .iter()
                .position(|(parameter, _, _)| !parameter.required)
                .unwrap_or(arguments.len());
            arguments.insert(first_optional, body_argument);
        } else {
            arguments.push(body_argument);
        }
    }
    let method_name = method_name(&operation.id);
    let mut args = arguments
        .iter()
        .map(|(_, _, declaration)| declaration.clone())
        .collect::<Vec<_>>();
    if has_url_pagination {
        // Internal continuation only: callers use `{operation}Pages`, while
        // every continued request still takes this exact operation path.
        args.push("?string $_poolsterPaginationUrl = null".into());
    }
    let args = args.join(", ");
    let mut output = format!("    public function {method_name}({args}): {return_type}\n    {{\n");
    if let Some(policy) = idempotency_annotation(operation) {
        if policy.get("auto_generate").and_then(Value::as_bool) == Some(true) {
            if let Some(parameter_name) = policy.get("parameter_name").and_then(Value::as_str) {
                if let Some((_, variable, _)) = arguments.iter().find(|(parameter, _, _)| {
                    parameter.name == parameter_name && parameter.location == "header"
                }) {
                    let _ = writeln!(
                        output,
                        "        ${variable} ??= self::poolsterIdempotencyKey();"
                    );
                }
            }
        }
    }
    output.push_str(&format!(
        "        $path = {};\n",
        php_string(&operation.path)
    ));
    for (parameter, variable, _) in &arguments {
        if parameter.location != "querystring" {
            if let Some(content) = poolster_core::openapi32::parameter_content(parameter)
                .ok()
                .and_then(|items| items.into_iter().next())
            {
                let required_json = parameter.required && is_json_parameter_content(parameter);
                if required_json && !json_content_allows_null(parameter) {
                    let _ = writeln!(
                        output,
                        "        if (${variable} === null) throw new \\InvalidArgumentException('required JSON parameter is not nullable');"
                    );
                }
                let guard = if required_json {
                    String::new()
                } else {
                    format!("if (${variable} !== null) ")
                };
                let _ = writeln!(
                    output,
                    "        {guard}${variable} = $this->poolsterParameterContent(${variable}, {});",
                    php_string(&content.content_type)
                );
            }
        }
    }
    let mut query = Vec::new();
    let mut headers = Vec::new();
    let mut body = "null";
    for (parameter, variable, _) in &arguments {
        match parameter.location.as_str() {
            "path" => {
                output.push_str(&format!(
                    "        $path = str_replace({}, rawurlencode((string) ${variable}), $path);\n",
                    php_string(&format!("{{{}}}", parameter.name)),
                ));
            }
            "query" => query.push(format!("{} => ${variable}", php_string(&parameter.name))),
            "header" => headers.push(format!("{} => ${variable}", php_string(&parameter.name))),
            "body" => body = "$body",
            _ => {}
        }
    }
    let query = if query.is_empty() {
        "[]".into()
    } else {
        format!("[{}]", query.join(", "))
    };
    let query = if let Some((parameter, variable, _)) = arguments
        .iter()
        .find(|(parameter, _, _)| parameter.location == "querystring")
    {
        let content = poolster_core::openapi32::parameter_content(parameter)
            .ok()
            .and_then(|items| items.into_iter().next())
            .map(|item| item.content_type)
            .unwrap_or_else(|| "text/plain".into());
        if parameter.required
            && is_json_parameter_content(parameter)
            && !json_content_allows_null(parameter)
        {
            let _ = writeln!(
                output,
                "        if (${variable} === null) throw new \\InvalidArgumentException('required JSON query is not nullable');"
            );
        }
        let allow_null = parameter.required
            && is_json_parameter_content(parameter)
            && json_content_allows_null(parameter);
        format!(
            "$this->poolsterWholeQuery(${variable}, {}, {allow_null})",
            php_string(&content)
        )
    } else {
        query
    };
    let cookies = arguments
        .iter()
        .filter(|(parameter, _, _)| parameter.location == "cookie")
        .map(|(parameter, variable, _)| {
            format!(
                "(${variable} === null ? null : {} . '=' . rawurlencode((string) ${variable}))",
                php_string(&parameter.name)
            )
        })
        .collect::<Vec<_>>();
    if !cookies.is_empty() {
        headers.push(format!(
            "'Cookie' => implode('; ', array_filter([{}], static fn($value) => $value !== null))",
            cookies.join(", ")
        ));
    }
    let headers = if headers.is_empty() {
        if is_sse {
            "[]".into()
        } else {
            "['Accept' => 'application/json']".into()
        }
    } else {
        format!(
            "array_merge({}, [{}])",
            if is_sse {
                "[]"
            } else {
                "['Accept' => 'application/json']"
            },
            headers.join(", ")
        )
    };
    if let Some(content) = poolster_core::openapi32::request_content(operation)
        .ok()
        .and_then(|items| {
            items
                .into_iter()
                .find(|content| content.content_type.starts_with("multipart/"))
        })
    {
        let definition = serde_json::to_string(&content).expect("multipart content metadata");
        let _ = writeln!(
            output,
            "        if ($body instanceof \\{namespace}\\MultipartBody) $body = $body->withEncoding(json_decode({}, true, 512, JSON_THROW_ON_ERROR));",
            php_string(&definition)
        );
    }
    let body_kind = operation_body_kind(operation);
    let pagination_argument = if has_url_pagination {
        ", $_poolsterPaginationUrl"
    } else {
        ", null"
    };
    let retryable = idempotency_annotation(operation).is_some()
        || !matches!(operation.method, poolster_core::HttpMethod::Post)
        || operation.parameters.iter().any(|parameter| {
            parameter.location == "header" && parameter.name.eq_ignore_ascii_case("idempotency-key")
        });
    let idempotency_argument = idempotency_annotation(operation)
        .and_then(|policy| policy.get("header"))
        .and_then(Value::as_str)
        .map(|header| format!(", {}", php_string(header)))
        .unwrap_or_else(|| ", null".into());
    if is_sse {
        output.push_str(&format!(
            "        return $this->eventStream({}, $path, {query}, {headers}, {body}, {body_kind:?});\n",
            php_string(operation.method.as_str()),
        ));
    } else {
        output.push_str(&format!(
            "        $contents = $this->request({}, $path, {query}, {headers}, {body}, {body_kind:?}, {retryable}{pagination_argument}{idempotency_argument});\n",
            php_string(operation.method.as_str()),
        ));
    }
    if is_sse {
        // `eventStream` already returned the PSR-7 body above. Keeping the
        // return surface as `StreamInterface` avoids pretending PSR-18 can
        // universally provide a live decoded event iterator.
    } else if return_type == "void" {
        output.push_str("        return;\n");
    } else if operation_is_binary_response(operation) {
        output.push_str("        return $contents;\n");
    } else {
        if return_type.starts_with('?') || return_type.ends_with("|null") {
            output
                .push_str("        if ($contents === '') {\n            return null;\n        }\n");
        } else {
            output.push_str("        if ($contents === '') {\n            throw new \\UnexpectedValueException('API response body was empty');\n        }\n");
        }
        let sequential = operation
            .responses
            .iter()
            .find(|response| response.status.starts_with('2'))
            .and_then(|response| response.media_types.first())
            .map(|media| media.content_type.as_str())
            .filter(|content| {
                [
                    "application/x-ndjson",
                    "application/ndjson",
                    "application/jsonl",
                    "application/json-seq",
                ]
                .contains(content)
            });
        if let Some(content) = sequential {
            let _ = writeln!(
                output,
                "        $data = $this->poolsterSequentialJson($contents, {});",
                php_string(content)
            );
        } else {
            output.push_str(
                "        $data = json_decode($contents, true, 512, JSON_THROW_ON_ERROR);\n",
            );
        }
        if let Some(schema) = response {
            output.push_str(&format!(
                "        return {};\n",
                from_value("$data", schema, named_types)
            ));
        } else {
            output.push_str("        return $data;\n");
        }
    }
    output.push_str("    }\n\n");
    let _ = namespace; // Keeps the signature symmetrical with model rendering.
    output
}
