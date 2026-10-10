//! Request_body planning and emission for Elixir HTTP.
use crate::*;

pub(crate) fn render_operation_body(
    output: &mut String,
    module: &str,
    api: &Api,
    operation: &Operation,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let _ = writeln!(
        output,
        "{pad}path = \"{}\"",
        escape_elixir_string(&operation.path)
    );
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "path")
    {
        let value = elixir_parameter_identifier(operation, parameter);
        let value = if parameter.required {
            value
        } else {
            format!("Keyword.get(options, :{value})")
        };
        let value = parameter_content_value(parameter, &value);
        let _ = writeln!(
            output,
            "{pad}path = String.replace(path, \"{{{}}}\", URI.encode(to_string({value}), &URI.char_unreserved?/1))",
            escape_elixir_string(&parameter.name),
        );
    }
    if poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        == Some("url")
    {
        let _ = writeln!(
            output,
            "{pad}path = case Keyword.fetch(options, :_poolster_pagination_url) do {{:ok, url}} -> {{:poolster_url, url}}; :error -> path end"
        );
    }
    let query = operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "query")
        .map(|parameter| {
            let variable = elixir_parameter_identifier(operation, parameter);
            let value = if parameter.required {
                variable
            } else {
                format!("Keyword.get(options, :{variable})")
            };
            let value = parameter_content_value(parameter, &value);
            format!("{{\"{}\", {value}}}", escape_elixir_string(&parameter.name))
        })
        .collect::<Vec<_>>();
    let _ = writeln!(output, "{pad}query = [{}]", query.join(", "));
    if let Some(parameter) = operation
        .parameters
        .iter()
        .find(|parameter| parameter.location == "querystring")
    {
        let name = elixir_parameter_identifier(operation, parameter);
        let value = if parameter.required {
            name.clone()
        } else {
            format!("Keyword.get(options, :{name})")
        };
        let content = poolster_core::openapi32::parameter_content(parameter)
            .ok()
            .and_then(|items| items.into_iter().next())
            .map(|item| item.content_type)
            .unwrap_or_else(|| "text/plain".into());
        let allow_null = parameter.required
            && is_json_parameter_content(parameter)
            && json_content_allows_null(parameter);
        let _ = writeln!(
            output,
            "{pad}query = Client.whole_query({value}, \"{}\", {allow_null})",
            escape_elixir_string(&content)
        );
    }
    let headers = operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "header")
        .map(|parameter| {
            let variable = elixir_parameter_identifier(operation, parameter);
            let value = if parameter.required {
                variable
            } else {
                format!("Keyword.get(options, :{variable})")
            };
            let value = parameter_content_value(parameter, &value);
            let value = if poolster_core::poolster_extension(
                &operation.annotations,
                "idempotency-resolved",
            )
            .is_some_and(|policy| {
                policy
                    .get("auto_generate")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                    && policy
                        .get("parameter_name")
                        .and_then(serde_json::Value::as_str)
                        == Some(parameter.name.as_str())
            }) {
                format!("case {value} do nil -> Client.idempotency_key(); provided -> provided end")
            } else {
                value
            };
            format!("{{\"{}\", {value}}}", escape_elixir_string(&parameter.name))
        })
        .collect::<Vec<_>>();
    let _ = writeln!(output, "{pad}headers = [{}]", headers.join(", "));
    let cookies = operation.parameters.iter().filter(|parameter| parameter.location == "cookie").map(|parameter| {
        let id = elixir_parameter_identifier(operation, parameter);
        let value = if parameter.required { id } else { format!("Keyword.get(options, :{id})") };
        let value = parameter_content_value(parameter, &value);
        format!("case {value} do nil -> nil; value -> \"{}=\" <> URI.encode(to_string(value), &URI.char_unreserved?/1) end", escape_elixir_string(&parameter.name))
    }).collect::<Vec<_>>();
    if !cookies.is_empty() {
        let _ = writeln!(
            output,
            "{pad}headers = headers ++ [{{\"cookie\", [{}] |> Enum.reject(&is_nil/1) |> Enum.join(\"; \")}}]",
            cookies.join(", ")
        );
    }

    let _ = writeln!(
        output,
        "{pad}headers = headers |> Enum.reject(fn {{_name, value}} -> is_nil(value) end) |> Enum.map(fn {{name, value}} -> {{name, to_string(value)}} end)"
    );
    let body: String = if operation.request_body.is_some() {
        if request_body_required(operation) {
            "body".into()
        } else {
            "Keyword.get(options, :body)".into()
        }
    } else {
        "nil".into()
    };
    let body = if let Some(content) = poolster_core::openapi32::request_content(operation)
        .ok()
        .and_then(|items| {
            items
                .into_iter()
                .find(|content| content.content_type.starts_with("multipart/"))
        }) {
        let definition = serde_json::to_string(&content).expect("multipart content metadata");
        let _ = writeln!(
            output,
            "{pad}body = case {body} do %{}.MultipartBody{{}} = value -> {}.MultipartBody.with_encoding(value, Jason.decode!(\"{}\")); other -> other end",
            module,
            module,
            escape_elixir_string(&definition)
        );
        "body".to_owned()
    } else {
        body
    };
    let response = response_decode(api, operation, module, "response");
    let body_kind = operation_body_kind(operation);
    let response_kind = operation_response_kind(operation);
    let error_types = operation_error_types(module, operation);
    let idempotency_argument =
        poolster_core::poolster_extension(&operation.annotations, "idempotency-resolved")
            .and_then(|policy| policy.get("header"))
            .and_then(serde_json::Value::as_str)
            .map(|header| format!(", \"{}\"", escape_elixir_string(header)))
            .unwrap_or_else(|| ", nil".into());
    let native_method = if matches!(
        &operation.method,
        poolster_core::HttpMethod::Query | poolster_core::HttpMethod::Custom(_)
    ) {
        format!("\"{}\"", escape_elixir_string(operation.method.as_str()))
    } else {
        format!(":{}", operation.method.as_str().to_ascii_lowercase())
    };
    if operation_is_sse(operation) {
        let _ = writeln!(
            output,
            "{pad}Client.event_stream(client, {}, path, query, headers, {body}, :{body_kind})",
            native_method,
        );
        return;
    }
    let _ = writeln!(
        output,
        "{pad}case Client.request(client, {}, path, query, headers, {body}, :{body_kind}, :{response_kind}, {error_types}{idempotency_argument}) do",
        native_method,
    );
    let _ = writeln!(output, "{pad}  {{:ok, response}} -> {{:ok, {response}}}");
    for response in operation
        .responses
        .iter()
        .filter(|response| is_error_status(&response.status))
    {
        let Some(schema) = response
            .media_types
            .iter()
            .find(|media| media.content_type.contains("json"))
            .or_else(|| response.media_types.first())
            .and_then(|media| media.schema.as_ref())
        else {
            continue;
        };
        let error = declared_error_name(operation, &response.status);
        let decoded = decode_value("reason.body", schema, module);
        let _ = writeln!(
            output,
            "{pad}  {{:error, %{module}.Errors.{error}{{}} = reason}} -> {{:error, %{{reason | body: {decoded}}}}}"
        );
    }
    let _ = writeln!(output, "{pad}  {{:error, reason}} -> {{:error, reason}}");
    let _ = writeln!(output, "{pad}end");
}
