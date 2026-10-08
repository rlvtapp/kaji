//! Allocate normalized model symbols without changing JSON wire names.
use super::*;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
};
fn model_names(api: &Api) -> BTreeMap<String, String> {
    let reserved: BTreeSet<String> = RESERVED
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let mut counts = BTreeMap::new();
    for schema in &api.schemas {
        *counts
            .entry(pascal_case(&schema.name).to_ascii_lowercase())
            .or_insert(0usize) += 1;
    }
    let mut used: BTreeSet<String> = api
        .schemas
        .iter()
        .map(|schema| pascal_case(&schema.name).to_ascii_lowercase())
        .collect();
    used.extend(reserved.iter().cloned());
    let mut names = BTreeMap::new();
    for schema in &api.schemas {
        let symbol = pascal_case(&schema.name);
        let key = symbol.to_ascii_lowercase();
        if counts[&key] == 1 && !reserved.contains(&key) {
            if schema.name != symbol {
                names.insert(schema.name.clone(), symbol);
            }
            continue;
        }
        let hash = schema
            .name
            .bytes()
            .fold(0xcbf29ce484222325u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
        let mut name = format!("{symbol}Model{hash:016x}");
        while !used.insert(pascal_case(&name).to_ascii_lowercase()) {
            name.push('X');
        }
        names.insert(schema.name.clone(), name);
    }
    names
}
const RESERVED: &[&str] = &[
    "Models",
    "JSON",
    "Class",
    "Object",
    "String",
    "Integer",
    "Float",
    "Array",
    "Hash",
    "NilClass",
    "TrueClass",
    "FalseClass",
];
fn value(value: &mut SchemaValue, names: &BTreeMap<String, String>) {
    match &mut value.kind {
        SchemaKind::Reference { reference } => {
            if let Some((prefix, name)) = reference.rsplit_once('/') {
                let decoded = name.replace("~1", "/").replace("~0", "~");
                if let Some(new) = names.get(&decoded) {
                    *reference = format!("{prefix}/{}", new.replace('~', "~0").replace('/', "~1"));
                }
            } else if let Some(new) = names.get(reference) {
                *reference = new.clone();
            }
        }
        SchemaKind::Array { items } => self::value(items, names),
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                self::value(&mut field.value, names);
            }
            if let AdditionalProperties::Schema { value } = additional_properties {
                self::value(value, names);
            }
        }
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            for variant in variants {
                self::value(variant, names);
            }
        }
        SchemaKind::Not { schema } => self::value(schema, names),
        _ => {}
    }
    if let Some(discriminator) = &mut value.discriminator {
        for reference in discriminator.mapping.values_mut() {
            if let Some((prefix, name)) = reference.rsplit_once('/') {
                if let Some(new) = names.get(&name.replace("~1", "/").replace("~0", "~")) {
                    *reference = format!("{prefix}/{}", new.replace('~', "~0").replace('/', "~1"));
                }
            } else if let Some(new) = names.get(reference) {
                *reference = new.clone();
            }
        }
    }
}
fn operation_names(api: &Api) -> BTreeMap<String, String> {
    let mut counts = BTreeMap::new();
    for operation in &api.operations {
        *counts
            .entry(ruby_identifier(&operation.id).to_ascii_lowercase())
            .or_insert(0usize) += 1;
    }
    let mut used: BTreeSet<String> = api
        .operations
        .iter()
        .map(|operation| ruby_identifier(&operation.id).to_ascii_lowercase())
        .collect();
    let reserved: BTreeSet<String> = [
        "initialize",
        "request",
        "retry_delay",
        "check_cancellation",
        "retry_pause",
        "execute_with_retry",
        "poolster_parameter_content",
        "poolster_whole_query",
        "poolster_sequential_json",
        "poolster_json_path",
        "poolster_with_body_value",
    ]
    .iter()
    .map(|name| name.to_ascii_lowercase())
    .collect();
    used.extend(reserved.iter().cloned());
    let mut names = BTreeMap::new();
    for operation in &api.operations {
        let symbol = ruby_identifier(&operation.id);
        if counts[&symbol.to_ascii_lowercase()] == 1
            && !reserved.contains(&symbol.to_ascii_lowercase())
        {
            continue;
        }
        let hash = operation
            .id
            .bytes()
            .fold(0xcbf29ce484222325u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
        let mut name = format!("{symbol}Operation{hash:016x}");
        while !used.insert(ruby_identifier(&name).to_ascii_lowercase()) {
            name.push('X');
        }
        names.insert(operation.id.clone(), name);
    }
    names
}
pub(super) fn prepare(api: &Api) -> Cow<'_, Api> {
    let models = model_names(api);
    let operations = operation_names(api);
    if models.is_empty() && operations.is_empty() {
        return Cow::Borrowed(api);
    }
    let mut prepared = api.clone();
    for schema in &mut prepared.schemas {
        if let Some(name) = models.get(&schema.name) {
            schema.name = name.clone();
        }
        value(&mut schema.value, &models);
    }
    for operation in &mut prepared.operations {
        if let Some(name) = operations.get(&operation.id) {
            operation.id = name.clone();
        }
        for parameter in &mut operation.parameters {
            if let Some(schema) = &mut parameter.schema {
                value(schema, &models);
            }
        }
        if let Some(body) = &mut operation.request_body {
            for media in &mut body.media_types {
                if let Some(schema) = &mut media.schema {
                    value(schema, &models);
                }
            }
        }
        for response in &mut operation.responses {
            for media in &mut response.media_types {
                if let Some(schema) = &mut media.schema {
                    value(schema, &models);
                }
            }
        }
    }
    Cow::Owned(prepared)
}
pub(super) fn model_symbols(api: &Api) -> BTreeMap<String, String> {
    let names = model_names(api);
    api.schemas
        .iter()
        .map(|schema| {
            (
                schema.name.clone(),
                pascal_case(names.get(&schema.name).unwrap_or(&schema.name)),
            )
        })
        .collect()
}

pub(super) fn operation_symbols(api: &Api) -> BTreeMap<String, String> {
    let names = operation_names(api);
    api.operations
        .iter()
        .map(|operation| {
            (
                operation.id.clone(),
                ruby_identifier(names.get(&operation.id).unwrap_or(&operation.id)),
            )
        })
        .collect()
}
