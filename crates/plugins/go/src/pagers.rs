use super::*;

pub(super) fn render_cursor_pager(output: &mut String, api: &Api, operation: &Operation) {
    let Some(pagination) = cursor_pagination(api, operation) else {
        return;
    };
    let GoResponseKind::Json(response_type) = operation_response_kind(operation) else {
        return;
    };
    let operation_name = go_type_name(&operation.id);
    let request_name = operation_request_name(api, operation);
    let pager_name = format!("{operation_name}Pager");
    let cursor_update = match &pagination.location {
        GoCursorLocation::Parameter { name, optional } => {
            let field = name;
            if *optional {
                format!("\tif pager.started {{ pager.input.{field} = &pager.cursor }}\n")
            } else {
                format!("\tif pager.started {{ pager.input.{field} = pager.cursor }}\n")
            }
        }
        GoCursorLocation::RequestBody {
            body_type,
            field_name,
            body_required,
        } => {
            if *body_required {
                format!(
                    "\tif pager.started {{\n\t\tbodyCopy := pager.input.Body\n\t\tbodyCopy.{field_name} = &pager.cursor\n\t\tpager.input.Body = bodyCopy\n\t}}\n"
                )
            } else {
                format!(
                    "\tif pager.started {{\n\t\tif pager.input.Body == nil {{ return nil, fmt.Errorf(\"poolster: cursor pagination requires a {body_type} body\") }}\n\t\tbodyCopy := *pager.input.Body\n\t\tbodyCopy.{field_name} = &pager.cursor\n\t\tpager.input.Body = &bodyCopy\n\t}}\n"
                )
            }
        }
    };
    let body_preflight = match &pagination.location {
        GoCursorLocation::RequestBody {
            body_type,
            body_required: false,
            ..
        } => format!(
            "\tif pager.input.Body == nil {{ return nil, fmt.Errorf(\"poolster: cursor pagination requires a {body_type} body\") }}\n"
        ),
        _ => String::new(),
    };
    let _ = writeln!(
        output,
        "// {pager_name} iterates pages returned by {operation_name}."
    );
    let _ = writeln!(output, "type {pager_name} struct {{");
    let _ = writeln!(
        output,
        "\tclient *Client\n\tinput *{request_name}\n\tcursor string\n\tstarted bool\n\tdone bool\n}}"
    );
    let _ = writeln!(
        output,
        "// {operation_name}Pages starts cursor pagination without guessing fields from a response."
    );
    let _ = writeln!(
        output,
        "func (client *Client) {operation_name}Pages(input *{request_name}) *{pager_name} {{"
    );
    let _ = writeln!(output, "\tcopyInput := &{request_name}{{}}");
    output.push_str("\tif input != nil { *copyInput = *input }\n");
    let _ = writeln!(
        output,
        "\treturn &{pager_name}{{client: client, input: copyInput}}\n}}"
    );
    let _ = writeln!(
        output,
        "// Next returns the next full response page, or io.EOF after the final page."
    );
    let _ = writeln!(
        output,
        "func (pager *{pager_name}) Next(ctx context.Context) (*{response_type}, error) {{"
    );
    output.push_str("\tif pager.done { return nil, io.EOF }\n");
    output.push_str(&body_preflight);
    output.push_str(&cursor_update);
    let _ = writeln!(
        output,
        "\tresponse, err := pager.client.{operation_name}(ctx, pager.input)"
    );
    output.push_str("\tif err != nil { return nil, err }\n");
    let _ = writeln!(
        output,
        "\tcursor, ok := poolsterPaginationString(response, {:?})",
        pagination.next_cursor_path
    );
    output.push_str("\tif !ok { pager.done = true; return response, nil }\n\tpager.cursor = cursor\n\tpager.started = true\n\treturn response, nil\n}\n\n");
}

pub(super) fn render_offset_pager(output: &mut String, api: &Api, operation: &Operation) {
    let Some(pagination) = offset_pagination(api, operation) else {
        return;
    };
    let GoResponseKind::Json(response_type) = operation_response_kind(operation) else {
        return;
    };
    let operation_name = go_type_name(&operation.id);
    let request_name = operation_request_name(api, operation);
    let pager_name = format!("{operation_name}Pager");
    let offset_field = &pagination.offset_name;
    let limit_field = &pagination.limit_name;
    let _ = writeln!(
        output,
        "// {pager_name} iterates declared offset/limit pages returned by {operation_name}."
    );
    let _ = writeln!(output, "type {pager_name} struct {{");
    let _ = writeln!(
        output,
        "\tclient *Client\n\tinput *{request_name}\n\toffset int64\n\tlimit *int64\n\tdone bool\n}}"
    );
    let _ = writeln!(
        output,
        "// {operation_name}Pages starts declared offset/limit pagination."
    );
    let _ = writeln!(
        output,
        "func (client *Client) {operation_name}Pages(input *{request_name}) *{pager_name} {{"
    );
    let _ = writeln!(output, "\tcopyInput := &{request_name}{{}}");
    output.push_str("\tif input != nil { *copyInput = *input }\n");
    let _ = writeln!(output, "\toffset := int64(0)");
    let _ = writeln!(
        output,
        "\tif copyInput.{offset_field} != nil {{ offset = *copyInput.{offset_field} }}"
    );
    let _ = writeln!(
        output,
        "\treturn &{pager_name}{{client: client, input: copyInput, offset: offset, limit: copyInput.{limit_field}}}\n}}"
    );
    let _ = writeln!(
        output,
        "// Next returns the next full response page, or io.EOF after the final page."
    );
    let _ = writeln!(
        output,
        "func (pager *{pager_name}) Next(ctx context.Context) (*{response_type}, error) {{"
    );
    output.push_str("\tif pager.done { return nil, io.EOF }\n");
    let _ = writeln!(output, "\tpager.input.{offset_field} = &pager.offset");
    let _ = writeln!(
        output,
        "\tresponse, err := pager.client.{operation_name}(ctx, pager.input)"
    );
    output.push_str("\tif err != nil { return nil, err }\n");
    let _ = writeln!(
        output,
        "\tcount, ok := poolsterPaginationArrayLen(response, {:?})",
        pagination.results_path
    );
    output.push_str("\tif !ok || count == 0 { pager.done = true; return response, nil }\n");
    output.push_str("\tpager.offset += int64(count)\n");
    output
        .push_str("\tif pager.limit != nil && int64(count) < *pager.limit { pager.done = true }\n");
    output.push_str("\treturn response, nil\n}\n\n");
}

pub(super) fn render_url_pager(output: &mut String, api: &Api, operation: &Operation) {
    let Some(pagination) = url_pagination(api, operation) else {
        return;
    };
    let GoResponseKind::Json(response_type) = operation_response_kind(operation) else {
        return;
    };
    let operation_name = go_type_name(&operation.id);
    let request_name = operation_request_name(api, operation);
    let pager_name = format!("{operation_name}Pager");
    let has_input = !operation.parameters.is_empty() || operation.request_body.is_some();
    let input_field = if has_input {
        format!("\tinput *{request_name}\n")
    } else {
        String::new()
    };
    let constructor_input = if has_input {
        format!("input *{request_name}")
    } else {
        String::new()
    };
    let constructor_copy = if has_input {
        format!(
            "\tcopyInput := &{request_name}{{}}\n\tif input != nil {{ *copyInput = *input }}\n\treturn &{pager_name}{{client: client, input: copyInput}}\n"
        )
    } else {
        format!("\treturn &{pager_name}{{client: client}}\n")
    };
    let initial_call = if has_input {
        format!("pager.client.{operation_name}(ctx, pager.input)")
    } else {
        format!("pager.client.{operation_name}(ctx)")
    };
    let mut continuation_prep = String::from("\theaders := http.Header{}\n");
    if has_input {
        render_pagination_headers(&mut continuation_prep, operation);
    }
    if has_input && operation.request_body.is_some() {
        continuation_prep.push_str("\tbody := pager.input.Body\n");
    } else {
        continuation_prep.push_str("\tvar body any\n");
    }
    let error_expression = operation_error_expression(operation, "err");
    let _ = writeln!(
        output,
        "// {pager_name} follows declared same-origin continuation URLs for {operation_name}."
    );
    let _ = writeln!(output, "type {pager_name} struct {{");
    let _ = writeln!(
        output,
        "\tclient *Client\n{input_field}\tnextURL string\n\tstarted bool\n\tdone bool\n}}"
    );
    let _ = writeln!(
        output,
        "// {operation_name}Pages starts URL pagination using the operation's normal typed request.\nfunc (client *Client) {operation_name}Pages({constructor_input}) *{pager_name} {{"
    );
    output.push_str(&constructor_copy);
    output.push_str("}\n");
    let _ = writeln!(
        output,
        "// Next returns the next full response page, or io.EOF after the final page.\nfunc (pager *{pager_name}) Next(ctx context.Context) (*{response_type}, error) {{"
    );
    output.push_str("\tif pager.done { return nil, io.EOF }\n");
    output.push_str("\tif !pager.started {\n");
    let _ = writeln!(output, "\t\tresponse, err := {initial_call}");
    output.push_str("\t\tif err != nil { return nil, err }\n");
    let _ = writeln!(
        output,
        "\t\tnextURL, ok := poolsterPaginationString(response, {:?})",
        pagination.next_url_path
    );
    output.push_str("\t\tpager.started = true\n\t\tif !ok { pager.done = true; return response, nil }\n\t\tpager.nextURL = nextURL\n\t\treturn response, nil\n\t}\n");
    output.push_str(&continuation_prep);
    let _ = writeln!(
        output,
        "\trequest, err := pager.client.newPaginationRequest(ctx, {:?}, pager.nextURL, headers, body)",
        operation.method.as_str()
    );
    let _ = writeln!(
        output,
        "\tif err != nil {{ return nil, {error_expression} }}"
    );
    let _ = writeln!(output, "\tvar response {response_type}");
    let _ = writeln!(
        output,
        "\tif err := pager.client.doWithRetry(request, &response); err != nil {{ return nil, {error_expression} }}"
    );
    let _ = writeln!(
        output,
        "\tnextURL, ok := poolsterPaginationString(&response, {:?})",
        pagination.next_url_path
    );
    output.push_str("\tif !ok { pager.done = true } else { pager.nextURL = nextURL }\n\treturn &response, nil\n}\n\n");
}

pub(super) fn render_pagination_headers(output: &mut String, operation: &Operation) {
    for parameter in &operation.parameters {
        if parameter.location != "header" {
            continue;
        }
        let field = format!("pager.input.{}", parameter_field_name(operation, parameter));
        if parameter.required {
            let _ = writeln!(
                output,
                "\theaders.Set({:?}, fmt.Sprint({field}))",
                parameter.name
            );
        } else {
            let _ = writeln!(
                output,
                "\tif {field} != nil {{ headers.Set({:?}, fmt.Sprint(*{field})) }}",
                parameter.name
            );
        }
    }
}
