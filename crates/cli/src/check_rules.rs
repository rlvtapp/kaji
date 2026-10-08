//! API contract diagnostics independent of report output.

use std::collections::{BTreeMap, BTreeSet};

use poolster_core::Api;
use serde::Deserialize;

use super::CheckSeverity;

#[derive(Debug, Deserialize)]
pub(super) struct CheckSidecarOperation {
    #[serde(default)]
    pub(super) operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CheckDiagnostic {
    pub(super) code: &'static str,
    pub(super) severity: CheckSeverity,
    pub(super) method: String,
    pub(super) path: String,
    pub(super) message: String,
    pub(super) hint: String,
}

impl CheckDiagnostic {
    pub(super) fn fingerprint(&self) -> String {
        format!("{}:{}:{}", self.code, self.method, self.path)
    }

    pub(super) fn render(&self) {
        eprintln!(
            "{}[{}] {} {}: {}\n  help: {}",
            self.severity.label(),
            self.code,
            self.method,
            self.path,
            self.message,
            self.hint
        );
    }
}

pub(super) const CHECK_RULES: &[&str] = &[
    "missing-operation-id",
    "duplicate-operation-id",
    "ambiguous-operation-id",
    "unsafe-path",
    "ambiguous-path",
    "missing-path-parameter",
    "optional-path-parameter",
    "missing-success-response",
];

fn operation_symbol(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn path_template_parameters(path: &str) -> Vec<&str> {
    path.split('/')
        .filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
        .filter(|name| !name.is_empty())
        .collect()
}

pub(super) fn check_api(api: &Api, source: &[CheckSidecarOperation]) -> Vec<CheckDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut ids = BTreeMap::<String, (String, String)>::new();
    let mut symbols = BTreeMap::<String, (String, String, String)>::new();

    for (index, operation) in api.operations.iter().enumerate() {
        let source_operation = source.get(index);
        let method = operation.method.as_str().to_owned();
        let path = operation.path.clone();
        let operation_id = source_operation
            .map(|operation| operation.operation_id.trim())
            .unwrap_or("");

        if operation_id.is_empty() {
            diagnostics.push(CheckDiagnostic {
                code: "missing-operation-id",
                severity: CheckSeverity::Error,
                method: method.clone(),
                path: path.clone(),
                message: "operationId is missing; Poolster can infer a name, but the inferred public SDK and CLI name may change when the path changes".into(),
                hint: "add a stable, unique operationId such as listUsers".into(),
            });
        } else if let Some((previous_method, previous_path)) =
            ids.insert(operation_id.to_owned(), (method.clone(), path.clone()))
        {
            diagnostics.push(CheckDiagnostic {
                code: "duplicate-operation-id",
                severity: CheckSeverity::Error,
                method: method.clone(),
                path: path.clone(),
                message: format!(
                    "operationId {operation_id:?} is also used by {previous_method} {previous_path}"
                ),
                hint: "give every operation a unique operationId".into(),
            });
        } else {
            let symbol = operation_symbol(operation_id);
            if !symbol.is_empty() {
                if let Some((previous_id, previous_method, previous_path)) = symbols.insert(
                    symbol,
                    (operation_id.to_owned(), method.clone(), path.clone()),
                ) {
                    if previous_id != operation_id {
                        diagnostics.push(CheckDiagnostic {
                            code: "ambiguous-operation-id",
                            severity: CheckSeverity::Error,
                            method: method.clone(),
                            path: path.clone(),
                            message: format!(
                                "operationId {operation_id:?} normalizes to the same generated symbol as {previous_id:?} on {previous_method} {previous_path}"
                            ),
                            hint: "rename one operationId so it stays distinct after case and punctuation normalization".into(),
                        });
                    }
                }
            }
        }

        if !path.starts_with('/') {
            diagnostics.push(CheckDiagnostic {
                code: "unsafe-path",
                severity: CheckSeverity::Error,
                method: method.clone(),
                path: path.clone(),
                message: "path is not absolute".into(),
                hint: "start every OpenAPI path with `/`".into(),
            });
        }
        if path.is_empty() || path.contains("//") {
            diagnostics.push(CheckDiagnostic {
                code: "ambiguous-path",
                severity: CheckSeverity::Error,
                method: method.clone(),
                path: path.clone(),
                message: "path contains an empty segment, which makes generated command grouping ambiguous".into(),
                hint: "use a single slash between path segments".into(),
            });
        }

        let path_parameters = path_template_parameters(&path)
            .into_iter()
            .collect::<BTreeSet<_>>();
        for name in &path_parameters {
            match operation
                .parameters
                .iter()
                .find(|parameter| parameter.location == "path" && parameter.name == *name)
            {
                None => diagnostics.push(CheckDiagnostic {
                    code: "missing-path-parameter",
                    severity: CheckSeverity::Error,
                    method: method.clone(),
                    path: path.clone(),
                    message: format!("path parameter {{{name}}} is not declared as a parameter"),
                    hint: format!("declare `{name}` with `in: path` and `required: true`"),
                }),
                Some(parameter) if !parameter.required => diagnostics.push(CheckDiagnostic {
                    code: "optional-path-parameter",
                    severity: CheckSeverity::Error,
                    method: method.clone(),
                    path: path.clone(),
                    message: format!("path parameter {{{name}}} is not required"),
                    hint: "OpenAPI path parameters must set `required: true`".into(),
                }),
                Some(_) => {}
            }
        }

        if !operation
            .responses
            .iter()
            .any(|response| response.status.starts_with('2'))
        {
            diagnostics.push(CheckDiagnostic {
                code: "missing-success-response",
                severity: CheckSeverity::Error,
                method,
                path,
                message: "operation declares no 2xx success response".into(),
                hint: "declare the successful status code, for example `200`, `201`, or `204`"
                    .into(),
            });
        }
    }
    diagnostics
}
