//! Explicit, reviewable compatibility with vendor OpenAPI annotations.
//! Wire paths, property names and authentication contracts are never renamed.
use crate::Api;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MigrationReport {
    pub converted: Vec<String>,
    pub manual: Vec<String>,
}

fn normalize_operation(
    id: &mut String,
    annotations: &mut BTreeMap<String, Value>,
    root: &Value,
    report: &mut MigrationReport,
) -> bool {
    let original = id.clone();
    for key in [
        "x-fern-sdk-group-name",
        "x-fern-sdk-method-name",
        "x-speakeasy-group",
        "x-speakeasy-name-override",
    ] {
        if annotations.get(key).is_some_and(|value| !value.is_string()) {
            report.manual.push(format!(
                "{original}: {key} requires an explicit string binding"
            ));
        }
    }
    for key in ["x-fern-ignore", "x-speakeasy-ignore", "x-fern-idempotent"] {
        if annotations
            .get(key)
            .is_some_and(|value| !value.is_boolean())
        {
            report
                .manual
                .push(format!("{original}: {key} requires a boolean"));
        }
    }
    if let Some(object) = annotations
        .get("x-stainless-method")
        .and_then(Value::as_object)
    {
        for key in object.keys().filter(|key| key.as_str() != "path") {
            report.manual.push(format!(
                "{original}: x-stainless-method.{key} is preserved but not translated"
            ));
        }
    }
    let mut methods = Vec::new();
    if let Some(value) = annotations.get("x-stainless-method") {
        if let Some(path) = value
            .as_str()
            .or_else(|| value.get("path").and_then(Value::as_str))
        {
            methods.push(path.trim_start_matches("$client.").replace('.', "_"));
        } else {
            report.manual.push(format!(
                "{original}: x-stainless-method has an unsupported shape"
            ));
        }
    }
    let group = annotations
        .get("x-fern-sdk-group-name")
        .and_then(Value::as_str)
        .or_else(|| annotations.get("x-speakeasy-group").and_then(Value::as_str));
    let method = annotations
        .get("x-fern-sdk-method-name")
        .and_then(Value::as_str)
        .or_else(|| {
            annotations
                .get("x-speakeasy-name-override")
                .and_then(Value::as_str)
        });
    if let Some(method) = method {
        methods.push(group.map_or_else(|| method.to_owned(), |group| format!("{group}_{method}")));
    }
    methods.sort();
    methods.dedup();
    if methods.len() == 1 && !methods[0].is_empty() {
        *id = methods[0].clone();
        report
            .converted
            .push(format!("{original}: SDK operation name -> {id}"));
    } else if methods.len() > 1 {
        report
            .manual
            .push(format!("{original}: conflicting vendor method names"));
    }
    if let Some(group) = group {
        annotations.insert("tags".into(), json!([group]));
    }
    if !annotations.contains_key("x-poolster-pagination") {
        if let Some(value) = annotations.get("x-speakeasy-pagination").cloned() {
            // Validation remains owned by the portable pagination contract.
            annotations.insert("x-poolster-pagination".into(), value);
            report
                .converted
                .push(format!("{original}: Speakeasy pagination"));
        } else if let Some(value) = annotations.get("x-fern-pagination") {
            if let Some(rule) = fern_pagination(value) {
                annotations.insert("x-poolster-pagination".into(), rule);
                report
                    .converted
                    .push(format!("{original}: Fern cursor pagination"));
            } else {
                report.manual.push(format!("{original}: pagination requires explicit x-poolster-pagination (unsupported Fern binding)"));
            }
        }
    }
    if annotations.get("x-fern-idempotent") == Some(&Value::Bool(true))
        && !annotations.contains_key("x-poolster-idempotency")
    {
        let headers = root
            .get("x-fern-idempotency-headers")
            .and_then(Value::as_array);
        if let Some(header) = headers
            .filter(|h| h.len() == 1)
            .and_then(|h| h[0].get("header"))
            .and_then(Value::as_str)
        {
            annotations.insert(
                "x-poolster-idempotency".into(),
                json!({"header":header,"auto_generate":false}),
            );
            report
                .converted
                .push(format!("{original}: idempotency header"));
        } else {
            report.manual.push(format!(
                "{original}: idempotency requires one explicit header"
            ));
        }
    }
    for key in annotations.keys().filter(|key| vendor_key(key)) {
        if !matches!(
            key.as_str(),
            "x-stainless-method"
                | "x-fern-sdk-group-name"
                | "x-fern-sdk-method-name"
                | "x-speakeasy-group"
                | "x-speakeasy-name-override"
                | "x-speakeasy-pagination"
                | "x-fern-pagination"
                | "x-fern-idempotent"
                | "x-fern-ignore"
                | "x-speakeasy-ignore"
        ) {
            report
                .manual
                .push(format!("{original}: {key} is preserved but not translated"));
        }
    }
    let ignored = ["x-fern-ignore", "x-speakeasy-ignore"]
        .iter()
        .any(|key| annotations.get(*key) == Some(&Value::Bool(true)));
    if ignored {
        report
            .converted
            .push(format!("{original}: excluded operation"));
    }
    ignored
}
fn vendor_key(key: &str) -> bool {
    ["x-fern-", "x-stainless-", "x-speakeasy-"]
        .iter()
        .any(|prefix| key.starts_with(prefix))
}
fn fern_pagination(value: &Value) -> Option<Value> {
    // Only bindings whose semantics are exactly representable are automatic.
    if value.get("offset").is_some()
        || value.get("next_path").is_some()
        || value.get("has-next-page").is_some()
    {
        return None;
    }
    let cursor = value.get("cursor")?.as_str()?.strip_prefix("$request.")?;
    if cursor.is_empty() || cursor.contains('.') {
        return None;
    }
    let results = value.get("results")?.as_str()?.strip_prefix("$response.")?;
    let next = value
        .get("next_cursor")?
        .as_str()?
        .strip_prefix("$response.")?;
    Some(
        json!({"type":"cursor","inputs":[{"name":cursor,"type":"cursor","in":"parameters"}],"outputs":{"results":format!("$.{results}"),"nextCursor":format!("$.{next}")}}),
    )
}

fn report_root(root: &Value, report: &mut MigrationReport) {
    if let Some(root) = root.as_object() {
        for key in root
            .keys()
            .filter(|key| vendor_key(key) && key.as_str() != "x-fern-idempotency-headers")
        {
            report
                .manual
                .push(format!("document: {key} is preserved but not translated"));
        }
    }
}
fn report_schema_annotations(value: Option<&Value>, report: &mut MigrationReport) {
    fn visit(value: &Value, keys: &mut std::collections::BTreeSet<String>) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    if vendor_key(key) {
                        keys.insert(key.clone());
                    } else if !matches!(
                        key.as_str(),
                        "example" | "examples" | "default" | "enum" | "const"
                    ) {
                        visit(value, keys);
                    }
                }
            }
            Value::Array(array) => {
                for value in array {
                    visit(value, keys);
                }
            }
            _ => {}
        }
    }
    let mut keys = std::collections::BTreeSet::new();
    if let Some(value) = value {
        visit(value, &mut keys);
    }
    for key in keys {
        report
            .manual
            .push(format!("components: {key} is preserved but not translated"));
    }
}

/// Normalize annotations for ordinary generation through the sidecar adapter.
pub fn normalize_api(api: &mut Api, root: &Value) -> MigrationReport {
    let mut report = MigrationReport::default();
    report_root(root, &mut report);
    api.operations.retain_mut(|operation| {
        !normalize_operation(
            &mut operation.id,
            &mut operation.annotations,
            root,
            &mut report,
        )
    });
    // Collisions fail explicitly rather than silently overwriting generated files.
    let mut ids = std::collections::BTreeSet::new();
    for operation in &api.operations {
        if !ids.insert(&operation.id) {
            report.manual.push(format!(
                "duplicate SDK operation name {:?}; set unique operationId values",
                operation.id
            ));
        }
    }
    report
}

/// Rewrite operation annotations in a bundled document without changing wire names.
pub fn normalize_openapi(document: &mut Value) -> MigrationReport {
    let root =
        json!({"x-fern-idempotency-headers": document.get("x-fern-idempotency-headers").cloned()});
    let mut report = MigrationReport::default();
    report_root(document, &mut report);
    report_schema_annotations(document.get("components"), &mut report);
    if let Some(paths) = document.get_mut("paths").and_then(Value::as_object_mut) {
        for (path, item) in paths {
            if let Some(item) = item.as_object_mut() {
                let mut remove = Vec::new();
                for (method, operation) in item.iter_mut().filter(|(method, _)| {
                    matches!(
                        method.as_str(),
                        "get"
                            | "put"
                            | "post"
                            | "delete"
                            | "patch"
                            | "head"
                            | "options"
                            | "trace"
                            | "query"
                    )
                }) {
                    if let Some(operation) = operation.as_object_mut() {
                        let mut annotations = operation
                            .iter()
                            .filter(|(key, _)| key.starts_with("x-") || *key == "tags")
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let mut id = operation
                            .get("operationId")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("{method}_{path}"));
                        if normalize_operation(&mut id, &mut annotations, &root, &mut report) {
                            remove.push(method.clone());
                        }
                        if operation.contains_key("operationId") || id != format!("{method}_{path}")
                        {
                            operation.insert("operationId".into(), Value::String(id));
                        }
                        operation.extend(annotations);
                    }
                }
                for method in remove {
                    item.remove(&method);
                }
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_ignore_and_wire_contracts() {
        let mut doc = json!({"paths":{"/users/{user_id}":{"get":{"operationId":"old","x-fern-sdk-group-name":"users","x-fern-sdk-method-name":"get","parameters":[{"name":"user_id","in":"path"}]},"delete":{"x-speakeasy-ignore":true}}}});
        let report = normalize_openapi(&mut doc);
        assert!(report.manual.is_empty());
        assert_eq!(
            doc["paths"]["/users/{user_id}"]["get"]["operationId"],
            "users_get"
        );
        assert_eq!(
            doc["paths"]["/users/{user_id}"]["get"]["parameters"][0]["name"],
            "user_id"
        );
        assert!(doc["paths"]["/users/{user_id}"].get("delete").is_none());
    }
    #[test]
    fn explicit_native_rule_wins_and_unknown_behavior_is_reported() {
        let mut doc = json!({"paths":{"/a":{"get":{"x-stainless-method":"accounts.list","x-poolster-pagination":{"type":"page"},"x-speakeasy-pagination":{"type":"cursor"},"x-speakeasy-retries":{"secret":"never print me"}}}}});
        let report = normalize_openapi(&mut doc);
        assert_eq!(doc["paths"]["/a"]["get"]["operationId"], "accounts_list");
        assert_eq!(
            doc["paths"]["/a"]["get"]["x-poolster-pagination"]["type"],
            "page"
        );
        assert!(
            !serde_json::to_string(&report)
                .unwrap()
                .contains("never print me")
        );
        assert_eq!(report.manual.len(), 1);
    }
    #[test]
    fn fern_cursor_and_idempotency() {
        let mut doc = json!({"x-fern-idempotency-headers":[{"header":"Request-Key"}],"paths":{"/a":{"post":{"x-fern-idempotent":true,"x-fern-pagination":{"cursor":"$request.cursor","next_cursor":"$response.next","results":"$response.data"}}}}});
        assert!(normalize_openapi(&mut doc).manual.is_empty());
        assert_eq!(
            doc["paths"]["/a"]["post"]["x-poolster-idempotency"]["header"],
            "Request-Key"
        );
        assert_eq!(
            doc["paths"]["/a"]["post"]["x-poolster-pagination"]["outputs"]["results"],
            "$.data"
        );
    }
}
