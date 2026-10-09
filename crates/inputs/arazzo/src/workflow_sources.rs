//! Resolve local OpenAPI operations without leaking parser types.
use super::workflow_values::nonempty;
use anyhow::{Context, Result, ensure};
use poolster_core::native::workflows::HttpWorkflowOperation;
use serde_json::Value;
pub(super) fn operations(
    source: &str,
    document: &Value,
) -> Result<Vec<(String, HttpWorkflowOperation, Value)>> {
    ensure!(
        document["openapi"].as_str().is_some_and(|v| matches!(
            v,
            "3.0.0" | "3.0.1" | "3.0.2" | "3.0.3" | "3.0.4" | "3.1.0" | "3.1.1"
        )),
        "workflow source {source:?} requires OpenAPI 3.0 or 3.1"
    );
    let servers = document["servers"]
        .as_array()
        .context("workflow source requires exactly one explicit server URL")?;
    ensure!(
        servers.len() == 1 && servers[0].get("variables").is_none(),
        "workflow source requires one literal server URL"
    );
    let base_url = nonempty(&servers[0], "url")?.to_owned();
    ensure!(
        (base_url.starts_with("http://") || base_url.starts_with("https://"))
            && !base_url.contains('{'),
        "workflow source server must be an absolute HTTP URL"
    );
    let mut result = Vec::new();
    for (path, item) in document["paths"]
        .as_object()
        .context("OpenAPI source requires paths")?
    {
        ensure!(
            item.get("$ref").is_none(),
            "referenced OpenAPI path items are unsupported in workflow generation"
        );
        for method in ["get", "post", "put", "patch", "delete", "head", "options"] {
            if let Some(operation) = item.get(method) {
                ensure!(
                    operation.is_object() && operation["responses"].is_object(),
                    "resolved OpenAPI operation must have responses"
                );
                let mut op = operation.clone();
                let security = op.get("security").or_else(|| document.get("security"));
                if let Some(security) = security {
                    ensure!(security.is_array(), "OpenAPI security must be an array");
                    op["security"] = security.clone();
                }
                let inherited = item["parameters"].as_array().cloned().unwrap_or_default();
                let mut params = inherited;
                params.extend(op["parameters"].as_array().cloned().unwrap_or_default());
                op["parameters"] = Value::Array(params);
                let id = op["operationId"].as_str().unwrap_or_default().to_owned();
                let pointer = format!(
                    "/paths/{}/{}",
                    path.replace('~', "~0").replace('/', "~1"),
                    method
                );
                result.push((
                    pointer,
                    HttpWorkflowOperation {
                        source: source.to_owned(),
                        operation_id: id,
                        method: method.to_uppercase(),
                        path: path.clone(),
                        base_url: base_url.clone(),
                    },
                    op,
                ));
            }
        }
    }
    Ok(result)
}
