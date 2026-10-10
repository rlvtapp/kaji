//! Operations emission for the elixir HTTP SDK.
use crate::*;

pub(crate) fn operation_is_sse(operation: &Operation) -> bool {
    operation.responses.iter().any(|response| {
        response.status.starts_with('2')
            && response
                .media_types
                .iter()
                .any(|media| media.content_type.eq_ignore_ascii_case("text/event-stream"))
    })
}

pub(crate) fn render_operation(module: &str, api: &Api, operation: &Operation) -> String {
    let name = elixir_identifier(&operation.id);
    let return_type = if operation_is_sse(operation) {
        "Enumerable.t()".into()
    } else {
        response_type(api, operation, module)
    };
    let mut output = String::new();
    let _ = writeln!(
        output,
        "  @type {name}_option :: {}",
        operation_option_types(operation, module)
    );
    let _ = writeln!(
        output,
        "  @spec {name}(Client.t(), [{name}_option()]) :: {{:ok, {return_type}}} | {{:error, term()}}"
    );
    let _ = writeln!(
        output,
        "  def {name}(client, options \\\\ []) when is_list(options) do"
    );

    let required = operation
        .parameters
        .iter()
        .filter(|parameter| parameter.required)
        .collect::<Vec<_>>();
    if required.is_empty() && !request_body_required(operation) {
        render_operation_body(&mut output, module, api, operation, 4);
    } else {
        output.push_str("    with ");
        let mut checks = required
            .iter()
            .map(|parameter| {
                let variable = elixir_parameter_identifier(operation, parameter);
                let helper = if is_json_parameter_content(parameter)
                    && json_content_allows_null(parameter)
                {
                    "required_present"
                } else {
                    "required"
                };
                format!("{{:ok, {variable}}} <- Client.{helper}(options, :{variable})")
            })
            .collect::<Vec<_>>();
        if request_body_required(operation) {
            checks.push("{:ok, body} <- Client.required(options, :body)".into());
        }
        output.push_str(&checks.join(",\n         "));
        output.push_str(" do\n");
        render_operation_body(&mut output, module, api, operation, 6);
        output.push_str("    end\n");
    }
    output.push_str("  end\n\n");
    if let Some(page) = page_pagination::render(api, operation).ok().flatten() {
        output.push_str(&page);
    }
    if let Some(pagination) = cursor_pagination(operation) {
        output.push_str(&render_cursor_paginator(operation, &pagination));
    }
    output
}
