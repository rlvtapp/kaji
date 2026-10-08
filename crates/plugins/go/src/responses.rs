use super::*;

pub(super) fn operation_response_kind(operation: &Operation) -> GoResponseKind {
    let media = operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .or_else(|| {
            operation
                .responses
                .iter()
                .find(|response| response.status == "default")
        })
        .and_then(|response| response.media_types.first());
    let Some(media) = media else {
        return operation_response_type(operation)
            .map(GoResponseKind::Json)
            .unwrap_or(GoResponseKind::None);
    };
    let content_type = media.content_type.to_ascii_lowercase();
    if content_type.starts_with("multipart/") {
        return GoResponseKind::Binary;
    }
    if content_type.starts_with("text/event-stream") {
        return GoResponseKind::EventStream;
    }
    if content_type.contains("json") {
        return GoResponseKind::Json(
            media
                .schema
                .as_ref()
                .map(go_type)
                .or_else(|| operation_response_type(operation))
                .unwrap_or_else(|| "any".into()),
        );
    }
    if content_type.starts_with("text/") {
        return GoResponseKind::Text;
    }
    if content_type.starts_with("application/octet-stream")
        || media.schema.as_ref().is_some_and(|schema| {
            matches!(&schema.kind, SchemaKind::String)
                && matches!(schema.format.as_deref(), Some("binary") | Some("byte"))
        })
    {
        return GoResponseKind::Binary;
    }
    media
        .schema
        .as_ref()
        .map(go_type)
        .map(GoResponseKind::Json)
        .unwrap_or(GoResponseKind::None)
}

pub(super) fn error_responses(
    operation: &Operation,
) -> impl Iterator<Item = &poolster_core::OperationResponse> {
    operation.responses.iter().filter(|response| {
        response.status == "default"
            || response
                .status
                .parse::<u16>()
                .is_ok_and(|status| (400..600).contains(&status))
    })
}

pub(super) fn declared_error_name(operation: &Operation, status: &str) -> String {
    let suffix = if status == "default" {
        "Default".to_owned()
    } else {
        status.to_owned()
    };
    format!("{}Error{suffix}", go_type_name(&operation.id))
}

pub(super) fn render_declared_errors(output: &mut String, operation: &Operation) {
    for response in error_responses(operation) {
        let name = declared_error_name(operation, &response.status);
        let media = response.media_types.first();
        let body_type = media.and_then(|media| media.schema.as_ref()).map(go_type);
        let _ = writeln!(
            output,
            "// {name} is returned when {} responds with status {}.",
            go_type_name(&operation.id),
            response.status
        );
        let _ = writeln!(output, "type {name} struct {{");
        output.push_str("\tStatusCode int\n\tRawBody    []byte\n");
        if let Some(body_type) = &body_type {
            let _ = writeln!(output, "\tBody       {body_type}");
        }
        output.push_str("}\n\n");
        let _ = writeln!(output, "func (errorResponse *{name}) Error() string {{");
        let _ = writeln!(
            output,
            "\treturn fmt.Sprintf(\"poolster: {} failed with status %d\", errorResponse.StatusCode)",
            go_type_name(&operation.id)
        );
        output.push_str("}\n\n");
    }
    if error_responses(operation).next().is_none() {
        return;
    }
    let decoder = format!("decode{}Error", go_type_name(&operation.id));
    let _ = writeln!(output, "func {decoder}(requestError error) error {{");
    output.push_str("\tvar transportError *poolsterAPIError\n\tif !errors.As(requestError, &transportError) { return requestError }\n\tswitch transportError.StatusCode {\n");
    for response in error_responses(operation) {
        if response.status == "default" {
            continue;
        }
        let name = declared_error_name(operation, &response.status);
        let _ = writeln!(output, "\tcase {}:", response.status);
        let _ = writeln!(
            output,
            "\t\tresult := &{name}{{StatusCode: transportError.StatusCode, RawBody: transportError.Body}}"
        );
        render_error_body_decode(output, response, "result");
        output.push_str("\t\treturn result\n");
    }
    if let Some(default_response) =
        error_responses(operation).find(|response| response.status == "default")
    {
        let name = declared_error_name(operation, "default");
        output.push_str("\tdefault:\n");
        let _ = writeln!(
            output,
            "\t\tresult := &{name}{{StatusCode: transportError.StatusCode, RawBody: transportError.Body}}"
        );
        render_error_body_decode(output, default_response, "result");
        output.push_str("\t\treturn result\n");
    } else {
        output.push_str("\tdefault:\n\t\treturn requestError\n");
    }
    output.push_str("\t}\n}\n\n");
}

pub(super) fn render_error_body_decode(
    output: &mut String,
    response: &poolster_core::OperationResponse,
    target: &str,
) {
    let Some(schema) = response
        .media_types
        .first()
        .and_then(|media| media.schema.as_ref())
    else {
        return;
    };
    let content_type = response
        .media_types
        .first()
        .map(|media| media.content_type.as_str())
        .unwrap_or_default();
    if content_type.contains("json") {
        let _ = writeln!(
            output,
            "\t\tif len(transportError.Body) > 0 {{ _ = json.Unmarshal(transportError.Body, &{target}.Body) }}"
        );
    } else if content_type.starts_with("text/") && matches!(&schema.kind, SchemaKind::String) {
        let _ = writeln!(output, "\t\t{target}.Body = string(transportError.Body)");
    } else if matches!(&schema.kind, SchemaKind::String)
        && matches!(schema.format.as_deref(), Some("binary") | Some("byte"))
    {
        let _ = writeln!(
            output,
            "\t\t{target}.Body = append([]byte(nil), transportError.Body...)"
        );
    }
}

pub(super) fn operation_error_expression(operation: &Operation, error: &str) -> String {
    if error_responses(operation).next().is_some() {
        format!("decode{}Error({error})", go_type_name(&operation.id))
    } else {
        error.to_owned()
    }
}

/// Allocate operation input types separately from schema types. Reserve every
/// preferred input name first so adding a collision never steals another
/// operation's existing public name.
pub(super) fn operation_request_name(api: &Api, operation: &Operation) -> String {
    let preferred = format!("{}Request", go_type_name(&operation.id));
    let mut reserved: BTreeSet<String> = api
        .schemas
        .iter()
        .map(|schema| go_type_name(&schema.name))
        .collect();
    if !reserved.contains(&preferred) {
        return preferred;
    }
    reserved.extend(
        api.operations
            .iter()
            .map(|op| format!("{}Request", go_type_name(&op.id))),
    );
    let model_names: BTreeSet<String> = api
        .schemas
        .iter()
        .map(|schema| go_type_name(&schema.name))
        .collect();
    let mut collisions = api
        .operations
        .iter()
        .filter(|op| model_names.contains(&format!("{}Request", go_type_name(&op.id))))
        .collect::<Vec<_>>();
    collisions.sort_by(|a, b| a.id.cmp(&b.id));
    for op in collisions {
        let base = format!("{}OperationRequest", go_type_name(&op.id));
        let mut name = base.clone();
        let mut suffix = 2;
        while !reserved.insert(name.clone()) {
            name = format!("{base}{suffix}");
            suffix += 1;
        }
        if op.id == operation.id {
            return name;
        }
    }
    unreachable!("colliding operation belongs to API")
}
