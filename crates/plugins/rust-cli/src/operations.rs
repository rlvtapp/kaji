//! Operations implementation for generated rust-cli packages.
use super::*;

pub(super) fn render_operation(api: &Api, op: &Operation, command_name: &str) -> String {
    let mut command = command_parts(op);
    if command.len() > 1
        && command
            .first()
            .is_some_and(|part| part == &kebab_case(command_name))
    {
        command.remove(0);
    }
    let command = command
        .iter()
        .map(|part| literal(part))
        .collect::<Vec<_>>()
        .join(", ");
    let parameters = op
        .parameters
        .iter()
        .map(|parameter| {
            format!(
                "Parameter {{ name: {}, option: {}, location: {}, required: {} }}",
                literal(&parameter.name),
                literal(&kebab_case(&parameter.name)),
                literal(&parameter.location),
                parameter.required
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let body = body_flags(api, op)
        .iter()
        .map(|(name, kind, array, file, required)| {
            format!(
                "BodyField {{ name: {}, option: {}, kind: {}, array: {}, file: {}, required: {} }}",
                literal(name),
                literal(&kebab_case(name)),
                literal(kind),
                array,
                file,
                required
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let security = op
        .security
        .iter()
        .map(|requirement| {
            let schemes = requirement
                .schemes
                .keys()
                .map(|name| literal(name))
                .collect::<Vec<_>>()
                .join(", ");
            format!("&[{schemes}]")
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Operation {{ id: {}, command: &[{}], method: {}, path: {}, parameters: &[{}], body: &[{}], body_required: {}, security: &[{}] }}",
        literal(&op.id),
        command,
        literal(op.method.as_str()),
        literal(&op.path),
        parameters,
        body,
        op.request_body.as_ref().is_some_and(|body| body.required),
        security,
    )
}

pub(super) fn render_security_schemes(catalog: Option<&SecuritySchemeCatalog>) -> String {
    catalog
        .map(|catalog| {
            catalog
                .schemes
                .iter()
                .map(|scheme| {
                    let (kind, name, location) = match &scheme.kind {
                        SecuritySchemeKind::ApiKey { name, location } => (
                            "api-key",
                            name.as_deref().unwrap_or(&scheme.name),
                            location.as_deref().unwrap_or("header"),
                        ),
                        SecuritySchemeKind::Http { .. }
                        | SecuritySchemeKind::OAuth2 { .. }
                        | SecuritySchemeKind::OpenIdConnect { .. }
                        | SecuritySchemeKind::Other { .. } => ("bearer", "authorization", "header"),
                    };
                    format!(
                        "SecurityScheme {{ id: {}, kind: {}, name: {}, location: {} }}",
                        literal(&scheme.name),
                        literal(kind),
                        literal(name),
                        literal(location)
                    )
                })
                .collect::<Vec<_>>()
                .join(",\n    ")
        })
        .unwrap_or_default()
}
pub(super) fn command_parts(op: &Operation) -> Vec<String> {
    let groups = op
        .path
        .split('/')
        .filter(|part| !part.is_empty() && !part.starts_with('{'))
        .map(kebab_case)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let name = kebab_case(&op.id);
    let action = name.split('-').next().filter(|action| {
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
    });
    if groups.is_empty() {
        vec![name]
    } else if let Some(action) = action {
        let mut parts = groups;
        parts.push(action.to_owned());
        parts
    } else {
        vec![name]
    }
}
pub(super) fn body_flags(
    api: &Api,
    op: &Operation,
) -> Vec<(String, &'static str, bool, bool, bool)> {
    let Some(schema) = op
        .request_schema()
        .and_then(|schema| resolve_schema(api, schema))
    else {
        return vec![];
    };
    let SchemaKind::Object { fields, .. } = &schema.kind else {
        return vec![];
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
            Some((
                field.name.clone(),
                kind,
                array,
                !array && kind == "string" && content_field(&field.name),
                field.required,
            ))
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
