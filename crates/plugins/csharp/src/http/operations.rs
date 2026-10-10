//! Operations emission for the csharp HTTP SDK.
use crate::*;

pub(crate) fn render_operation(output: &mut String, operation: &Operation) {
    let operation_start = output.len();
    let name = pascal_case(&operation.id);
    let response = operation_response_surface(operation);
    let mut required_parameters = Vec::new();
    let mut optional_parameters = Vec::new();
    for parameter in &operation.parameters {
        let parameter_name = parameter_name(parameter);
        let parameter_type = if parameter.location == "querystring" {
            if parameter.required {
                "string".into()
            } else {
                "string?".into()
            }
        } else {
            parameter
                .schema
                .as_ref()
                .map(|schema| csharp_type(schema, !parameter.required))
                .unwrap_or_else(|| "JsonElement".into())
        };
        if parameter.required {
            required_parameters.push(format!("{parameter_type} {parameter_name}"));
        } else {
            optional_parameters.push(format!("{parameter_type} {parameter_name} = default"));
        }
    }
    let binary_body = request_body_is_binary(operation);
    let body = operation_request_type(operation)
        .map(|body| if binary_body { "byte[]".into() } else { body });
    if let Some(body_type) = &body {
        let required = operation
            .request_body
            .as_ref()
            .is_none_or(|body| body.required);
        let body_type = nullable_type(body_type.clone(), !required);
        if required {
            required_parameters.push(format!("{body_type} body"));
        } else {
            optional_parameters.push(format!("{body_type} body = default"));
        }
    }
    let mut parameters = required_parameters;
    parameters.extend(optional_parameters);
    parameters.push(if matches!(response, DotnetResponseSurface::Sse) { "[System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken = default".into() } else { "CancellationToken cancellationToken = default".into() });
    let return_type = match &response {
        DotnetResponseSurface::Json(value) => format!("Task<{value}>"),
        DotnetResponseSurface::Binary => "Task<byte[]>".into(),
        DotnetResponseSurface::Sse => "IAsyncEnumerable<string>".into(),
        DotnetResponseSurface::Empty => "Task".into(),
    };
    let declaration = if matches!(response, DotnetResponseSurface::Sse) {
        "async IAsyncEnumerable<string>".to_owned()
    } else {
        format!("async {return_type}")
    };
    let _ = writeln!(
        output,
        "    /// <summary>Invokes {} {}.</summary>\n    public {} {name}Async({})\n    {{\n  ",
        operation.method.as_str(),
        xml_escape(&operation.path),
        declaration,
        parameters.join(", "),
    );
    let _ = writeln!(
        output,
        "        var poolsterRequestPath = {:?};",
        operation.path
    );
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "path")
    {
        let name = parameter_name(parameter);
        let serialized = if parameter_json_content(parameter) {
            format!("JsonSerializer.Serialize({name},JsonOptions)")
        } else {
            format!("ParameterString({name})")
        };
        let _ = writeln!(
            output,
            "        poolsterRequestPath = poolsterRequestPath.Replace({:?}, Uri.EscapeDataString({serialized}), StringComparison.Ordinal);",
            format!("{{{}}}", parameter.name),
        );
    }
    output.push_str("        var query = new List<KeyValuePair<string, string?>>();\n");
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "query")
    {
        let name = parameter_name(parameter);
        if parameter_json_content(parameter) {
            let guard = if parameter.required {
                "true".to_owned()
            } else {
                format!("{name} is not null")
            };
            let _ = writeln!(
                output,
                "        if ({guard}) query.Add(new KeyValuePair<string,string?>({:?},JsonSerializer.Serialize({name},JsonOptions)));",
                parameter.name
            );
            continue;
        }
        if parameter
            .schema
            .as_ref()
            .is_some_and(|schema| matches!(schema.kind, SchemaKind::Array { .. }))
        {
            let _ = writeln!(
                output,
                "        if ({name} is not null) foreach (var poolsterQueryItem in {name}) query.Add(new KeyValuePair<string, string?>({:?}, ParameterString(poolsterQueryItem)));",
                parameter.name
            );
        } else {
            let value = if parameter.required {
                format!("ParameterString({name})")
            } else {
                format!("{name} is null ? null : ParameterString({name})")
            };
            let _ = writeln!(
                output,
                "        query.Add(new KeyValuePair<string, string?>({:?}, {value}));",
                parameter.name
            );
        }
    }
    for parameter in operation
        .parameters
        .iter()
        .filter(|p| p.location == "querystring")
    {
        let name = parameter_name(parameter);
        let _ = writeln!(
            output,
            "        if (!string.IsNullOrEmpty({name})) {{ ValidateWholeQuery({name}); poolsterRequestPath += \"?\" + {name}; }}"
        );
    }
    let _ = writeln!(
        output,
        "        var request = CreateRequest({}, poolsterRequestPath, query);",
        http_method_name(operation.method.as_str()),
    );
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "header")
    {
        let name = parameter_name(parameter);
        let serialized = if parameter_json_content(parameter) {
            format!("JsonSerializer.Serialize({name},JsonOptions)")
        } else {
            format!("ParameterString({name})")
        };
        let guard = if parameter.required {
            "true".to_owned()
        } else {
            format!("{name} is not null")
        };
        let _ = writeln!(
            output,
            "        if ({guard}) request.Headers.TryAddWithoutValidation({:?}, {serialized});",
            parameter.name,
        );
    }
    for parameter in operation
        .parameters
        .iter()
        .filter(|p| p.location == "cookie")
    {
        let name = parameter_name(parameter);
        let serialized = if parameter_json_content(parameter) {
            format!("JsonSerializer.Serialize({name},JsonOptions)")
        } else {
            format!("ParameterString({name})")
        };
        let guard = if parameter.required {
            "true".to_owned()
        } else {
            format!("{name} is not null")
        };
        let _ = writeln!(
            output,
            "        if ({guard}) request.Headers.TryAddWithoutValidation(\"Cookie\",{:?}+\"=\"+Uri.EscapeDataString({serialized}));",
            parameter.name
        );
    }
    if let Some(policy) = poolster_core::idempotency::resolved(operation) {
        let name = operation
            .parameters
            .iter()
            .find(|parameter| {
                parameter.location == "header"
                    && parameter.name.eq_ignore_ascii_case(&policy.parameter_name)
            })
            .map(parameter_name)
            .unwrap_or_else(|| camel_case(&policy.parameter_name));
        if policy.auto_generate {
            let _ = writeln!(
                output,
                "        if ({name} is null) request.Headers.TryAddWithoutValidation({:?}, Guid.NewGuid().ToString(\"D\"));",
                policy.header
            );
        }
        let _ = writeln!(
            output,
            "        request.Options.Set(new HttpRequestOptionsKey<string>(\"Poolster.IdempotencyHeader\"), {:?});",
            policy.header
        );
    }
    if multipart::selected(operation) && operation.request_body.as_ref().is_some_and(|b| b.required)
    {
        output.push_str("        ArgumentNullException.ThrowIfNull(body);\n");
    }
    if body.is_some() {
        if multipart::mixed(operation) {
            output.push_str(&format!("        if (body is {} multipart) request.Content = multipart.ToContent(); else if (body is PoolsterRawBody raw) request.Content = raw.ToContent(); else if (body is not null) request.Content = JsonContent.Create(body, options: JsonOptions);\n",multipart::body_name(operation)));
        } else if multipart::selected(operation) {
            output.push_str("        if (body is not null) request.Content = body.ToContent();\n");
        } else if let Some(media) = operation
            .request_body
            .as_ref()
            .and_then(|body| body.media_types.first())
            .map(|media| media.content_type.as_str())
            .filter(|media| {
                matches!(
                    *media,
                    "application/x-ndjson"
                        | "application/ndjson"
                        | "application/jsonl"
                        | "application/json-seq"
                )
            })
        {
            let _ = writeln!(
                output,
                "        if (body is not null) request.Content=EncodeSequentialJson(body,{media:?});"
            );
        } else if binary_body {
            output.push_str("        if (body is not null)\n        {\n            request.Content = new ByteArrayContent(body);\n            request.Content.Headers.ContentType = new MediaTypeHeaderValue(\"application/octet-stream\");\n        }\n");
        } else {
            if operation
                .request_body
                .as_ref()
                .is_some_and(|body| body.required)
            {
                output.push_str(
                    "        request.Content = JsonContent.Create(body, options: JsonOptions);\n",
                );
            } else {
                output.push_str("        if (body is not null)\n        {\n            request.Content = JsonContent.Create(body, options: JsonOptions);\n        }\n");
            }
        }
    }
    // C# iterators cannot yield from a try/catch block. SSE errors stay as the
    // common ApiException for now; normal request methods get status mapping.
    let maps_declared_errors = !declared_error_responses(operation).is_empty()
        && !matches!(response, DotnetResponseSurface::Sse);
    if maps_declared_errors {
        output.push_str("        try\n        {\n");
    }
    match response {
        DotnetResponseSurface::Json(response) => {
            let _ = writeln!(
                output,
                "        return await SendWithRetryAsync<{response}>(request, cancellationToken).ConfigureAwait(false);"
            );
        }
        DotnetResponseSurface::Binary => output.push_str(
            "        return await SendBytesAsync(request, cancellationToken).ConfigureAwait(false);\n",
        ),
        DotnetResponseSurface::Sse => output.push_str(
            "        await foreach (var item in StreamSseAsync(request, cancellationToken).ConfigureAwait(false))\n        {\n            yield return item;\n        }\n",
        ),
        DotnetResponseSurface::Empty => output.push_str(
            "        await SendWithRetryAsync(request, cancellationToken).ConfigureAwait(false);\n",
        ),
    }
    if maps_declared_errors {
        let map = dotnet_error_mapper_name(operation);
        let _ = writeln!(
            output,
            "        }}\n        catch (ApiException error)\n        {{\n            throw {map}(error);\n        }}"
        );
    }
    output.push_str("    }\n\n");
    if poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))
        .and_then(|value| value.get("type"))
        .and_then(serde_json::Value::as_str)
        == Some("url")
        && matches!(
            operation_response_surface(operation),
            DotnetResponseSurface::Json(_)
        )
    {
        let original = output[operation_start..].to_owned();
        let mut helper = original.clone();
        let end = helper.find("\n    {").unwrap();
        helper.insert_str(end - 1, ", string? poolsterURL = null");
        helper = helper.replacen(
            &format!("public async {return_type} {name}Async("),
            &format!("private async {return_type} {name}PoolsterURLAsync("),
            1,
        );
        let request = helper.find("        var request = CreateRequest").unwrap();
        helper.insert_str(request,&format!("        var paginationURL = poolsterURL is null ? null : {name}PoolsterURLTarget(poolsterURL);\n"));
        let lineend = helper[request..].find(";\n").unwrap() + request + 2;
        // The inserted validation line comes first; locate the actual request line afterwards.
        let request = helper[lineend..]
            .find("        var request = CreateRequest")
            .unwrap()
            + lineend;
        let lineend = helper[request..].find(";\n").unwrap() + request + 2;
        helper.insert_str(
            lineend,
            "        if (paginationURL is not null) request.RequestUri = paginationURL;\n",
        );
        let args = facade_arguments(operation).join(", ");
        let args = if args.is_empty() {
            String::new()
        } else {
            format!("{args}, ")
        };
        output.truncate(operation_start);
        let header = &original[..original.find("\n    {").unwrap()];
        let _ = writeln!(
            output,
            "{header}\n    {{\n        return await {name}PoolsterURLAsync({args}null).ConfigureAwait(false);\n    }}\n{helper}"
        );
    }
    if let Some(next_cursor_path) = dotnet_cursor_pagination(operation) {
        render_cursor_pager(output, operation, &next_cursor_path);
    }
}
