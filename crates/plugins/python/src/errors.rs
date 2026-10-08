use super::*;

pub(super) fn is_error_status(status: &str) -> bool {
    status == "default"
        || status
            .parse::<u16>()
            .is_ok_and(|status| (400..600).contains(&status))
}

pub(super) fn error_class_name(operation: &Operation, status: &str) -> String {
    let suffix = if status == "default" {
        "Default".into()
    } else {
        format!("Status{status}")
    };
    format!("{}{}Error", pascal_case(&operation.id), suffix)
}

pub(super) fn error_body_model(
    api: &Api,
    response: &poolster_core::OperationResponse,
) -> Option<String> {
    let reference = response
        .media_types
        .first()
        .and_then(|media| media.schema.as_ref())
        .and_then(|schema| match &schema.kind {
            SchemaKind::Reference { reference } => {
                Some(reference.rsplit('/').next().unwrap_or(reference).to_owned())
            }
            _ => None,
        })?;
    api.schemas
        .iter()
        .find(|schema| schema.name == reference)
        .and_then(|schema| {
            matches!(schema.value.kind, SchemaKind::Object { .. }).then_some(reference)
        })
        .map(|name| python_type_name(&name))
}

pub(super) fn declared_error_class_names(api: &Api) -> Vec<String> {
    api.operations
        .iter()
        .flat_map(|operation| {
            operation
                .responses
                .iter()
                .filter(move |response| is_error_status(&response.status))
                .map(move |response| error_class_name(operation, &response.status))
        })
        .collect()
}

pub(super) fn render_error_declaration(
    api: &Api,
    operation: &Operation,
    response: &poolster_core::OperationResponse,
) -> String {
    let name = error_class_name(operation, &response.status);
    let mut output = format!(
        "class {name}(ApiError):\n    \"\"\"Declared {} error response for `{}`.\"\"\"\n",
        response.status, operation.id
    );
    if let Some(model) = error_body_model(api, response) {
        let _ = writeln!(output, "\n    body: {model}");
    }
    output.push('\n');
    output
}

pub(super) fn error_declarations(api: &Api) -> Vec<(String, String)> {
    api.operations
        .iter()
        .flat_map(|operation| {
            operation
                .responses
                .iter()
                .filter(|response| is_error_status(&response.status))
                .map(move |response| {
                    (
                        error_class_name(operation, &response.status),
                        render_error_declaration(api, operation, response),
                    )
                })
        })
        .collect()
}

pub(super) fn render_declared_error_classes(api: &Api) -> String {
    error_declarations(api)
        .into_iter()
        .map(|(_, value)| value)
        .collect()
}

pub(super) fn partition_python_errors(api: &Api) -> bool {
    render_declared_error_classes(api).len() > 64 * 1024
}

pub(super) fn render_partitioned_errors(api: &Api) -> Vec<(String, String)> {
    if !partition_python_errors(api) {
        return Vec::new();
    }
    let declarations = error_declarations(api);
    let sizes = declarations
        .iter()
        .map(|(name, value)| value.len() + name.len() + 100)
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut entry = NOTICE.to_owned();
    for (index, range) in bounded_source_ranges(&sizes, 512).into_iter().enumerate() {
        let mut source = format!(
            "{NOTICE}from __future__ import annotations\nfrom ..runtime import ApiError\nfrom ..models import *\n\n"
        );
        let mut names = Vec::new();
        for (name, value) in &declarations[range] {
            // Public runtime aliases and pickle identities stay compatible.
            source.push_str(value.trim_end());
            source.push_str("\n    __module__ = __package__.rsplit('.', 1)[0] + '.runtime'\n\n");
            names.push(name);
        }
        let _ = writeln!(
            source,
            "__all__ = {}",
            serde_json::to_string(&names).unwrap()
        );
        files.push((format!("errors/chunk_{index:04}.py"), source));
        let _ = writeln!(entry, "from .chunk_{index:04} import *");
    }
    files.push(("errors/__init__.py".into(), entry));
    files
}

pub(super) fn operation_error_types(api: &Api, operation: &Operation) -> String {
    let entries = operation
        .responses
        .iter()
        .filter(|response| is_error_status(&response.status))
        .filter_map(|response| {
            let status = response.status.parse::<u16>().ok()?;
            let class = error_class_name(operation, &response.status);
            let model = error_body_model(api, response).unwrap_or_else(|| "None".into());
            Some(format!("{status}: ({class}, {model})"))
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        "None".into()
    } else {
        format!("{{{}}}", entries.join(", "))
    }
}
