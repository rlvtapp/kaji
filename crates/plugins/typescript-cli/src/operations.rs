//! Operations implementation for generated typescript-cli packages.
use super::*;

pub(super) fn operation_json(api: &Api, operation: &Operation, command_name: &str) -> Value {
    let parameters = operation
        .parameters
        .iter()
        .map(|parameter| {
            json!({
                "name": parameter.name,
                "option": kebab_case(&parameter.name),
                "key": camel_case(&parameter.name),
                "location": parameter.location,
                "required": parameter.required,
                "description": parameter.description,
            })
        })
        .collect::<Vec<_>>();
    let security = operation
        .security
        .iter()
        .map(|alternative| alternative.schemes.clone())
        .collect::<Vec<_>>();
    let mut command = command_parts(operation);
    if command.len() > 1
        && command
            .first()
            .is_some_and(|part| part == &kebab_case(command_name))
    {
        command.remove(0);
    }
    json!({
        "id": operation.id,
        "command": command,
        "method": operation.method.as_str(),
        "path": operation.path,
        "parameters": parameters,
        "bodyFields": body_flags(api, operation),
        "requestBodyRequired": operation.request_body.as_ref().is_some_and(|body| body.required),
        "security": security,
    })
}

pub(super) fn command_parts(operation: &Operation) -> Vec<String> {
    let derived_group = operation
        .path
        .split('/')
        .filter(|segment| !segment.is_empty() && !segment.starts_with('{'))
        .map(kebab_case)
        .filter(|group| !group.is_empty())
        .collect::<Vec<_>>();
    let operation_name = kebab_case(&operation.id);
    let derived_action = operation_name
        .split('-')
        .next()
        .filter(|action| {
            matches!(
                *action,
                "list"
                    | "get"
                    | "create"
                    | "update"
                    | "delete"
                    | "send"
                    | "archive"
                    | "restore"
                    | "verify"
                    | "cancel"
                    | "retry"
            )
        })
        .map(str::to_owned);
    let options =
        poolster_core::poolster_extension(&operation.annotations, "cli").and_then(Value::as_object);
    let group = options
        .and_then(|options| options.get("group"))
        .and_then(Value::as_str)
        .map(command_segments)
        .filter(|parts| !parts.is_empty());
    let command = options
        .and_then(|options| options.get("command"))
        .and_then(Value::as_str)
        .map(command_segments)
        .filter(|parts| !parts.is_empty());

    match (group, command) {
        (Some(mut group), Some(command)) => {
            group.extend(command);
            group
        }
        (Some(mut group), None) => {
            group.push(derived_action.unwrap_or(operation_name));
            group
        }
        (None, Some(command)) if command.len() > 1 => command,
        (None, Some(command)) => {
            if !derived_group.is_empty() {
                let mut parts = derived_group;
                parts.extend(command);
                parts
            } else {
                command
            }
        }
        (None, None) if !derived_group.is_empty() => {
            if let Some(action) = derived_action {
                let mut parts = derived_group;
                parts.push(action);
                parts
            } else {
                vec![operation_name]
            }
        }
        (None, None) => vec![operation_name],
    }
}

pub(super) fn command_segments(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .map(kebab_case)
        .filter(|part| !part.is_empty())
        .collect()
}

pub(super) fn body_flags(api: &Api, operation: &Operation) -> Vec<Value> {
    let Some(schema) = operation.request_schema() else {
        return Vec::new();
    };
    let Some(schema) = resolve_schema(api, schema) else {
        return Vec::new();
    };
    let SchemaKind::Object { fields, .. } = &schema.kind else {
        return Vec::new();
    };
    fields
        .iter()
        .filter_map(|field| {
            let value = resolve_schema(api, &field.value)?;
            let (kind, array) = match &value.kind {
                SchemaKind::String => ("string", false),
                SchemaKind::Integer => ("integer", false),
                SchemaKind::Number => ("number", false),
                SchemaKind::Boolean => ("boolean", false),
                SchemaKind::Array { items } => match resolve_schema(api, items)?.kind {
                    SchemaKind::String => ("string", true),
                    SchemaKind::Integer => ("integer", true),
                    SchemaKind::Number => ("number", true),
                    SchemaKind::Boolean => ("boolean", true),
                    _ => return None,
                },
                _ => return None,
            };
            Some(json!({
                "name": field.name,
                "option": kebab_case(&field.name),
                "key": camel_case(&field.name),
                "kind": kind,
                "array": array,
                "file": !array && kind == "string" && content_field(&field.name),
                "required": field.required,
                "description": value.description,
            }))
        })
        .collect()
}

pub(super) fn resolve_schema<'a>(api: &'a Api, schema: &'a SchemaValue) -> Option<&'a SchemaValue> {
    match &schema.kind {
        SchemaKind::Reference { reference } => api
            .schemas
            .iter()
            .find(|candidate| candidate.name == schema.kind.reference_name().unwrap_or(reference))
            .map(|candidate| &candidate.value),
        _ => Some(schema),
    }
}

pub(super) fn content_field(name: &str) -> bool {
    matches!(
        kebab_case(name).as_str(),
        "html" | "text" | "body" | "content" | "markdown" | "template"
    )
}
