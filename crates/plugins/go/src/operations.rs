use super::*;

pub(super) fn render_operation(output: &mut String, api: &Api, operation: &Operation) {
    let operation_name = go_type_name(&operation.id);
    let request_name = operation_request_name(api, operation);
    let has_input = !operation.parameters.is_empty() || operation.request_body.is_some();
    if has_input {
        let _ = writeln!(
            output,
            "// {request_name} contains the inputs for {operation_name}."
        );
        let _ = writeln!(output, "type {request_name} struct {{");
        for parameter in &operation.parameters {
            render_parameter_field(
                output,
                parameter,
                &parameter_field_name(operation, parameter),
            );
        }
        if let Some(body) = &operation.request_body {
            let body_type = body
                .media_types
                .iter()
                .find(|media_type| media_type.content_type.contains("json"))
                .or_else(|| body.media_types.first())
                .and_then(|media_type| media_type.schema.as_ref())
                .map(go_type)
                .unwrap_or_else(|| "any".into());
            let body_type = if has_multipart(operation) {
                if body
                    .media_types
                    .iter()
                    .any(|media| media.content_type.contains("json"))
                {
                    "any".to_owned()
                } else {
                    "*PoolsterMultipartBody".to_owned()
                }
            } else if body.required || body_type.starts_with('*') {
                body_type
            } else {
                format!("*{body_type}")
            };
            let _ = writeln!(output, "\tBody {body_type} `json:\"body,omitempty\"`");
        }
        output.push_str("}\n\n");
    }

    render_declared_errors(output, operation);
    let response_kind = operation_response_kind(operation);
    let parameters = if has_input {
        format!(", input *{request_name}")
    } else {
        String::new()
    };
    let result = match &response_kind {
        GoResponseKind::None => "error".to_owned(),
        GoResponseKind::Json(response) => format!("(*{response}, error)"),
        GoResponseKind::Text => "(string, error)".to_owned(),
        GoResponseKind::Binary => "([]byte, error)".to_owned(),
        GoResponseKind::EventStream => "(io.ReadCloser, error)".to_owned(),
    };
    let _ = writeln!(
        output,
        "// {operation_name} invokes {} {}.\nfunc (client *Client) {operation_name}(ctx context.Context{parameters}) {result} {{",
        operation.method.as_str(),
        operation.path
    );
    if has_input {
        output.push_str("\tif input == nil {\n\t\tinput = &");
        output.push_str(&request_name);
        output.push_str("{}\n\t}\n");
    }
    let _ = writeln!(output, "\tpath := {:?}", operation.path);
    output.push_str("\tquery := url.Values{}\n\theaders := http.Header{}\n");
    if operation
        .parameters
        .iter()
        .any(|p| p.location == "querystring")
    {
        output.push_str("\tvar wholeQuery string\n");
    }
    for parameter in &operation.parameters {
        render_parameter_use(
            output,
            parameter,
            &parameter_field_name(operation, parameter),
            &response_kind,
        );
    }
    let idempotency = operation
        .annotations
        .get("x-poolster-idempotency-resolved")
        .or_else(|| operation.annotations.get("x-kaji-idempotency-resolved"));
    if let Some(rule) = idempotency {
        if rule
            .get("auto_generate")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            let header = rule
                .get("header")
                .and_then(serde_json::Value::as_str)
                .unwrap();
            let _ = writeln!(
                output,
                "\tif len(headers.Values({header:?})) == 0 {{\n\t\tkey, err := poolsterNewIdempotencyKey()\n\t\tif err != nil {{"
            );
            render_error_return(
                output,
                &response_kind,
                &operation_error_expression(operation, "err"),
            );
            let _ = writeln!(output, "\t\t}}\n\t\theaders.Set({header:?}, key)\n\t}}");
        }
    }
    if let Some(body_config) = &operation.request_body {
        if has_multipart(operation) && !body_config.required {
            output.push_str("\tvar body any\n\tif input.Body != nil { body = input.Body }\n");
        } else {
            if body_config
                .media_types
                .first()
                .is_some_and(|media| sequential_media(&media.content_type))
            {
                output.push_str("\tvar body any = input.Body\n");
            } else {
                output.push_str("\tbody := input.Body\n");
            }
        }
    } else {
        output.push_str("\tvar body any\n");
    }
    if has_multipart(operation) {
        let media = operation
            .request_body
            .as_ref()
            .unwrap()
            .media_types
            .iter()
            .find(|m| {
                m.content_type
                    .to_ascii_lowercase()
                    .starts_with("multipart/")
            })
            .unwrap();
        let metadata = poolster_core::openapi32::request_content(operation)
            .expect("validated content metadata")
            .into_iter()
            .find(|m| m.content_type == media.content_type)
            .unwrap_or_else(|| poolster_core::openapi32::ContentDefinition {
                content_type: media.content_type.clone(),
                ..Default::default()
            });
        let encoded = serde_json::to_string(&metadata).expect("typed multipart metadata");
        let _ = writeln!(
            output,
            "\tif multipartBody, ok := any(body).(*PoolsterMultipartBody); ok && multipartBody != nil {{\n\t\tpreparedMultipart, preparationError := poolsterDeclaredMultipart(multipartBody, {:?})\n\t\tif preparationError != nil {{",
            encoded
        );
        render_error_return(output, &response_kind, "preparationError");
        output.push_str("\t\t}\n\t\tbody = preparedMultipart\n\t}\n");
    }
    if let Some(media) = operation
        .request_body
        .as_ref()
        .and_then(|b| b.media_types.first())
    {
        if sequential_media(&media.content_type) {
            let _ = writeln!(
                output,
                "\tif body != nil {{ body = poolsterSequentialBody{{ContentType:{:?},Value:body}} }}",
                media.content_type
            );
        }
    }
    let _ = writeln!(
        output,
        "\trequest, err := client.newRequest(ctx, {:?}, path, query, headers, body)",
        operation.method.as_str()
    );
    output.push_str("\tif err != nil {\n");
    render_error_return(
        output,
        &response_kind,
        &operation_error_expression(operation, "err"),
    );
    output.push_str("\t}\n");
    if operation
        .parameters
        .iter()
        .any(|p| p.location == "querystring")
    {
        output.push_str("\trequest.URL.RawQuery = wholeQuery\n");
    }
    if let Some(header) = idempotency
        .and_then(|rule| rule.get("header"))
        .and_then(serde_json::Value::as_str)
    {
        let _ = writeln!(
            output,
            "\trequest = request.WithContext(context.WithValue(request.Context(), poolsterIdempotencyContextKey{{}}, {header:?}))"
        );
    }
    match response_kind {
        GoResponseKind::None => {
            output.push_str("\tif err := client.do(request, nil); err != nil {\n");
            render_error_return(
                output,
                &GoResponseKind::None,
                &operation_error_expression(operation, "err"),
            );
            output.push_str("\t}\n\treturn nil\n}\n\n");
        }
        GoResponseKind::Json(response) => {
            let _ = writeln!(output, "\tvar response {response}");
            output.push_str("\tif err := client.do(request, &response); err != nil {\n");
            render_error_return(
                output,
                &GoResponseKind::Json(response.clone()),
                &operation_error_expression(operation, "err"),
            );
            output.push_str("\t}\n\treturn &response, nil\n}\n\n");
        }
        GoResponseKind::Text => {
            output.push_str(
                "\tvar response string\n\tif err := client.do(request, &response); err != nil {\n",
            );
            render_error_return(
                output,
                &GoResponseKind::Text,
                &operation_error_expression(operation, "err"),
            );
            output.push_str("\t}\n\treturn response, nil\n}\n\n");
        }
        GoResponseKind::Binary => {
            output.push_str(
                "\tvar response []byte\n\tif err := client.do(request, &response); err != nil {\n",
            );
            render_error_return(
                output,
                &GoResponseKind::Binary,
                &operation_error_expression(operation, "err"),
            );
            output.push_str("\t}\n\treturn response, nil\n}\n\n");
        }
        GoResponseKind::EventStream => {
            output.push_str("\tresponse, err := client.stream(request)\n\tif err != nil {\n");
            render_error_return(
                output,
                &GoResponseKind::EventStream,
                &operation_error_expression(operation, "err"),
            );
            output.push_str("\t}\n\treturn response, nil\n}\n\n");
        }
    }
}
