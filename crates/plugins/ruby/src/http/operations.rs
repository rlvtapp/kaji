use super::*;

pub(crate) fn ruby_resource_attribute(api: &Api, resource: &str) -> String {
    let mut name = ruby_identifier(resource);
    while matches!(
        name.as_str(),
        "initialize"
            | "request"
            | "retry_delay"
            | "check_cancellation"
            | "retry_pause"
            | "execute_with_retry"
    ) || api
        .operations
        .iter()
        .any(|operation| ruby_identifier(&operation.id) == name)
    {
        name.push_str("_resource");
    }
    name
}

pub(crate) fn ruby_request_options_name(operation: &Operation) -> String {
    let mut name = "request_options".to_owned();
    while operation
        .parameters
        .iter()
        .any(|parameter| ruby_parameter_identifier(operation, parameter) == name)
    {
        name.insert(0, '_');
    }
    name
}

pub(crate) fn json_content_allows_null(parameter: &poolster_core::OperationParameter) -> bool {
    fn allows(value: &SchemaValue) -> bool {
        value.nullable
            || matches!(value.kind, SchemaKind::Any | SchemaKind::Null)
            || match &value.kind {
                SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => {
                    variants.iter().any(allows)
                }
                _ => false,
            }
    }
    parameter.schema.as_ref().is_none_or(allows)
}

pub(crate) fn is_json_parameter_content(parameter: &poolster_core::OperationParameter) -> bool {
    poolster_core::openapi32::parameter_content(parameter)
        .ok()
        .and_then(|items| items.into_iter().next())
        .is_some_and(|content| {
            content.content_type == "application/json" || content.content_type.ends_with("+json")
        })
}

pub(crate) fn render_operation(api: &Api, operation: &Operation) -> String {
    let name = ruby_identifier(&snake_case(&operation.id));
    let mut args = Vec::new();
    for parameter in &operation.parameters {
        let name = ruby_parameter_identifier(operation, parameter);
        args.push(if parameter.required {
            format!("{name}:")
        } else {
            format!("{name}: nil")
        });
    }
    if let Some(body) = &operation.request_body {
        args.push(if body.required {
            "body:".into()
        } else {
            "body: nil".into()
        });
    }
    let request_options_name = ruby_request_options_name(operation);
    args.push(format!("{request_options_name}: nil"));
    let signature = args.join(", ");
    let mut out = format!(
        "    def {name}({signature})\n      path = {}\n",
        ruby_string(&operation.path)
    );
    if let Some(policy) = operation.annotations.get("x-poolster-idempotency-resolved") {
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
                    .map(|item| ruby_parameter_identifier(operation, item))
                    .unwrap_or_else(|| ruby_identifier(parameter));
                let _ = writeln!(
                    out,
                    "      require 'securerandom'\n      {variable} = SecureRandom.uuid if {variable}.nil?"
                );
            }
        }
    }
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location != "querystring")
    {
        if let Some(content) = poolster_core::openapi32::parameter_content(parameter)
            .ok()
            .and_then(|items| items.into_iter().next())
        {
            let id = ruby_parameter_identifier(operation, parameter);
            let required_json = parameter.required && is_json_parameter_content(parameter);
            if required_json && !json_content_allows_null(parameter) {
                let _ = writeln!(
                    out,
                    "      raise ArgumentError, 'required JSON parameter is not nullable' if {id}.nil?"
                );
            }
            let guard = if required_json {
                String::new()
            } else {
                format!(" unless {id}.nil?")
            };
            let _ = writeln!(
                out,
                "      {id} = poolster_parameter_content({id}, {}){guard}",
                ruby_string(&content.content_type)
            );
        }
    }
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "path")
    {
        let id = ruby_parameter_identifier(operation, parameter);
        let _ = writeln!(
            out,
            "      path = path.gsub({}, CGI.escape({id}.to_s).gsub(\"+\", \"%20\"))",
            ruby_string(&format!("{{{}}}", parameter.name))
        );
    }
    out.push_str("      query = {}\n");
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "query")
    {
        let id = ruby_parameter_identifier(operation, parameter);
        let _ = writeln!(
            out,
            "      query[{}] = {id} unless {id}.nil?",
            ruby_string(&parameter.name)
        );
    }
    if let Some(parameter) = operation
        .parameters
        .iter()
        .find(|parameter| parameter.location == "querystring")
    {
        let id = ruby_parameter_identifier(operation, parameter);
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
                out,
                "      raise ArgumentError, 'required JSON query is not nullable' if {id}.nil?"
            );
        }
        let allow_null = parameter.required
            && is_json_parameter_content(parameter)
            && json_content_allows_null(parameter);
        let _ = writeln!(
            out,
            "      query = poolster_whole_query({id}, {}, {allow_null})",
            ruby_string(&content)
        );
    }
    out.push_str("      headers = {}\n");
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "header")
    {
        let id = ruby_parameter_identifier(operation, parameter);
        let _ = writeln!(
            out,
            "      headers[{}] = {id}.to_s unless {id}.nil?",
            ruby_string(&parameter.name)
        );
    }
    let cookies = operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "cookie")
        .map(|parameter| {
            let id = ruby_parameter_identifier(operation, parameter);
            format!(
                "({id}.nil? ? nil : {} + '=' + CGI.escape({id}.to_s).gsub('+', '%20'))",
                ruby_string(&parameter.name)
            )
        })
        .collect::<Vec<_>>();
    if !cookies.is_empty() {
        let _ = writeln!(
            out,
            "      headers['Cookie'] = [{}].compact.join('; ')",
            cookies.join(", ")
        );
    }
    let response = response_model(api, operation);
    let body = if operation.request_body.is_some() {
        "body"
    } else {
        "nil"
    };
    let multipart = operation.request_body.as_ref().is_some_and(|body| {
        body.media_types
            .iter()
            .any(|media| media.content_type.starts_with("multipart/"))
    });
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
            out,
            "      body = body.with_encoding(JSON.parse({})) if body.is_a?(MultipartBody)",
            ruby_string(&definition)
        );
    }
    let retry_header = poolster_core::idempotency::resolved(operation)
        .map(|p| ruby_string(&p.header))
        .unwrap_or_else(|| "nil".into());
    let _ = writeln!(
        out,
        "      result = request({}, path, query: query, headers: headers, body: {body}, response_schemas: ResponseShapes[\"operations\"][{}], idempotency_header: {retry_header}, request_options: {request_options_name}, multipart: {multipart})",
        ruby_string(operation.method.as_str()),
        ruby_string(&operation.id)
    );
    match response.as_deref() {
        Some(model) => {
            let _ = writeln!(
                out,
                "      result.is_a?(Hash) ? Models::{model}.from_hash(result) : result\n    end\n\n"
            );
        }
        None if operation
            .success_schema()
            .is_some_and(|schema| matches!(schema.kind, SchemaKind::Array { .. })) =>
        {
            let shape = ruby_decode_shape(operation.success_schema().unwrap());
            let _ = writeln!(
                out,
                "      Models.decode_model_value(result, {shape})\n    end\n"
            );
        }
        None => out.push_str("      result\n    end\n\n"),
    }
    out
}
