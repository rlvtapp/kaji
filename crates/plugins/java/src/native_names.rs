//! Deterministic native symbols without changing property wire names.
use kaji_core::Api;
use std::collections::{BTreeMap, BTreeSet};
pub(crate) fn prepare(
    api: &Api,
    type_name: fn(&str) -> String,
    method_name: fn(&str) -> String,
    reserved: &[&str],
) -> Api {
    let mut result = api.clone();
    let mut used: BTreeSet<String> = reserved
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    for operation in &api.operations {
        used.insert(format!("{}Request", type_name(&operation.id)).to_ascii_lowercase());
    }
    let preferred: BTreeSet<String> = api
        .schemas
        .iter()
        .map(|schema| type_name(&schema.name).to_ascii_lowercase())
        .collect();
    let mut names = BTreeMap::new();
    for schema in &mut result.schemas {
        let raw = schema.name.clone();
        let base = type_name(&raw);
        let mut candidate = base.clone();
        let mut suffix = 2;
        if candidate.ends_with("Resource") || used.contains(&candidate.to_ascii_lowercase()) {
            candidate = format!("{base}Model");
        }
        while used.contains(&candidate.to_ascii_lowercase())
            || (candidate != base && preferred.contains(&candidate.to_ascii_lowercase()))
        {
            candidate = format!("{base}Model{suffix}");
            suffix += 1;
        }
        used.insert(candidate.to_ascii_lowercase());
        if candidate != base {
            names.insert(raw, candidate.clone());
            schema.name = candidate;
        }
    }
    let mut value = serde_json::to_value(&result).expect("API serialization");
    rewrite(&mut value, &names);
    result = serde_json::from_value(value).expect("native API preserves AST shape");
    let mut resource_names = BTreeMap::<String, String>::new();
    let mut resource_used = BTreeSet::new();
    for operation in &mut result.operations {
        if let Some(tags) = operation
            .annotations
            .get_mut("tags")
            .and_then(serde_json::Value::as_array_mut)
        {
            if let Some(tag) = tags.first_mut() {
                if let Some(original) = tag.as_str() {
                    let original = original.to_owned();
                    let allocated = resource_names.entry(original.clone()).or_insert_with(|| {
                        let base = type_name(&original);
                        let mut candidate = base.clone();
                        let mut suffix = 2;
                        while !resource_used.insert(candidate.to_ascii_lowercase()) {
                            candidate = format!("{base}{suffix}");
                            suffix += 1;
                        }
                        candidate
                    });
                    *tag = serde_json::json!(allocated);
                }
            }
        }
    }
    let mut methods = BTreeSet::from([
        "new".to_owned(),
        "forCall".into(),
        "for_call".into(),
        "with_transport".into(),
        "with_retry".into(),
        "with_hooks".into(),
        "wait".into(),
        "notify".into(),
        "notifyAll".into(),
        "getClass".into(),
        "clone".into(),
        "equals".into(),
        "hashCode".into(),
        "toString".into(),
        "finalize".into(),
        "ForCall".into(),
    ]);
    for operation in &mut result.operations {
        let original = operation.id.clone();
        let base = method_name(&original);
        let mut candidate = original.clone();
        let mut suffix = 2;
        while !methods.insert(method_name(&candidate)) {
            candidate = format!("{original}Operation{suffix}");
            suffix += 1;
        }
        if candidate != original {
            operation.annotations.insert(
                "kaji.source_operation_id".into(),
                serde_json::json!(original),
            );
            operation.id = candidate;
        }
        let mut arguments: BTreeSet<String> = [
            "body",
            "query",
            "request",
            "cancellationToken",
            "callOptions",
            "notify",
            "notifyAll",
            "wait",
            "getClass",
            "clone",
            "hashCode",
            "equals",
            "toString",
            "finalize",
        ]
        .iter()
        .map(|value| method_name(value))
        .collect();
        for parameter in &mut operation.parameters {
            let base = method_name(&parameter.name);
            let mut candidate = base.clone();
            let mut suffix = 2;
            while !arguments.insert(candidate.clone()) {
                candidate = format!("{base}{suffix}");
                suffix += 1;
            }
            parameter
                .annotations
                .insert("kaji.native_argument".into(), serde_json::json!(candidate));
        }
        let _ = base;
    }
    result
}
fn rewrite(value: &mut serde_json::Value, names: &BTreeMap<String, String>) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                if matches!(key.as_str(), "reference" | "$ref") {
                    if let Some(reference) = value.as_str() {
                        if let Some((prefix, tail)) = reference.rsplit_once('/') {
                            let raw = tail.replace("~1", "/").replace("~0", "~");
                            if let Some(name) = names.get(&raw) {
                                *value = serde_json::json!(format!("{prefix}/{name}"));
                            }
                        }
                    }
                } else {
                    rewrite(value, names)
                }
            }
            if let Some(serde_json::Value::Array(fields)) = object.get_mut("fields") {
                let mut unique = Vec::<serde_json::Value>::new();
                for field in fields.drain(..) {
                    let name = field.get("name").cloned();
                    if let Some(existing) = unique
                        .iter_mut()
                        .find(|item| item.get("name") == name.as_ref())
                    {
                        if field.get("required").and_then(|value| value.as_bool()) == Some(true) {
                            existing["required"] = serde_json::json!(true);
                        }
                    } else {
                        unique.push(field)
                    }
                }
                *fields = unique;
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                rewrite(value, names)
            }
        }
        _ => {}
    }
}
pub(crate) fn field_names(
    fields: &[kaji_core::Field],
    name: fn(&str) -> String,
    reserved: &[&str],
) -> BTreeMap<String, String> {
    let preferred: BTreeSet<_> = fields.iter().map(|field| name(&field.name)).collect();
    let mut used: BTreeSet<_> = reserved
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let mut result = BTreeMap::new();
    for field in fields {
        let base = name(&field.name);
        let mut candidate = base.clone();
        let mut suffix = 2;
        while used.contains(&candidate.to_ascii_lowercase())
            || (candidate != base && preferred.contains(&candidate.to_ascii_lowercase()))
        {
            candidate = format!("{base}{suffix}");
            suffix += 1;
        }
        used.insert(candidate.to_ascii_lowercase());
        result.insert(field.name.clone(), candidate);
    }
    result
}
