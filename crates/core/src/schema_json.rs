//! Neutral schema conversion for JSON-based schema metadata.
use crate::{AdditionalProperties, Discriminator, Field, SchemaKind, SchemaValue};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
/// Converts an OpenAPI Schema Object represented as JSON without rendering a
/// target-language type string or discarding a schema composition keyword.
pub fn convert_value(schema: &Value) -> SchemaValue {
    let Some(object) = schema.as_object() else {
        return SchemaValue::unknown();
    };
    let mut value = SchemaValue::new(convert_kind(object));
    value.nullable = object
        .get("nullable")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || object
            .get("type")
            .and_then(Value::as_array)
            .is_some_and(|types| types.iter().any(|kind| kind.as_str() == Some("null")));
    value.format = object
        .get("format")
        .and_then(Value::as_str)
        .map(str::to_owned);
    value.enum_values = object
        .get("enum")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    value.const_value = object.get("const").cloned();
    value.default = object.get("default").cloned();
    value.title = object
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_owned);
    value.description = object
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_owned);
    value.deprecated = object
        .get("deprecated")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    value.read_only = object
        .get("readOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    value.write_only = object
        .get("writeOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    value.discriminator = object.get("discriminator").and_then(convert_discriminator);
    for (key, raw) in object {
        if key.starts_with("x-") {
            value.extensions.insert(key.clone(), raw.clone());
        } else if is_constraint_key(key) {
            value.constraints.insert(key.clone(), raw.clone());
        }
    }
    value
}

fn convert_kind(schema: &Map<String, Value>) -> SchemaKind {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return SchemaKind::Reference {
            reference: reference.to_owned(),
        };
    }
    if let Some(variants) = schema.get("oneOf").and_then(Value::as_array) {
        return SchemaKind::OneOf {
            variants: variants.iter().map(convert_value).collect(),
        };
    }
    if let Some(variants) = schema.get("anyOf").and_then(Value::as_array) {
        return SchemaKind::AnyOf {
            variants: variants.iter().map(convert_value).collect(),
        };
    }
    if let Some(variants) = schema.get("allOf").and_then(Value::as_array) {
        return SchemaKind::AllOf {
            variants: variants.iter().map(convert_value).collect(),
        };
    }
    if let Some(negated) = schema.get("not") {
        return SchemaKind::Not {
            schema: Box::new(convert_value(negated)),
        };
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        let variants = types
            .iter()
            .filter_map(Value::as_str)
            .filter(|kind| *kind != "null")
            .map(|kind| SchemaValue::new(kind_from_type(kind, schema)))
            .collect::<Vec<_>>();
        if variants.len() > 1 {
            return SchemaKind::AnyOf { variants };
        }
        return variants
            .into_iter()
            .next()
            .map(|value| value.kind)
            .unwrap_or(SchemaKind::Null);
    }
    schema
        .get("type")
        .and_then(Value::as_str)
        .map(|kind| kind_from_type(kind, schema))
        .unwrap_or_else(|| {
            if schema.contains_key("properties") || schema.contains_key("additionalProperties") {
                kind_from_type("object", schema)
            } else if schema.contains_key("items") {
                kind_from_type("array", schema)
            } else {
                SchemaKind::Any
            }
        })
}

fn kind_from_type(kind: &str, schema: &Map<String, Value>) -> SchemaKind {
    match kind {
        "null" => SchemaKind::Null,
        "boolean" => SchemaKind::Boolean,
        "integer" => SchemaKind::Integer,
        "number" => SchemaKind::Number,
        "string" => SchemaKind::String,
        "array" => SchemaKind::Array {
            items: Box::new(
                schema
                    .get("items")
                    .map(convert_value)
                    .unwrap_or_else(SchemaValue::unknown),
            ),
        },
        "object" => SchemaKind::Object {
            fields: object_fields(schema),
            additional_properties: additional_properties(schema.get("additionalProperties")),
        },
        _ => SchemaKind::Any,
    }
}

fn object_fields(schema: &Map<String, Value>) -> Vec<Field> {
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    schema
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, value)| Field {
            name: name.clone(),
            value: convert_value(value),
            required: required.contains(name.as_str()),
            annotations: BTreeMap::new(),
        })
        .collect()
}

fn additional_properties(value: Option<&Value>) -> AdditionalProperties {
    match value {
        None => AdditionalProperties::Unspecified,
        Some(Value::Bool(true)) => AdditionalProperties::Any,
        Some(Value::Bool(false)) => AdditionalProperties::Forbidden,
        Some(value) => AdditionalProperties::Schema {
            value: Box::new(convert_value(value)),
        },
    }
}

fn convert_discriminator(value: &Value) -> Option<Discriminator> {
    let object = value.as_object()?;
    Some(Discriminator {
        property_name: object.get("propertyName")?.as_str()?.to_owned(),
        mapping: object
            .get("mapping")
            .and_then(Value::as_object)
            .map(|mapping| {
                mapping
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn is_constraint_key(key: &str) -> bool {
    matches!(
        key,
        "multipleOf"
            | "maximum"
            | "exclusiveMaximum"
            | "minimum"
            | "exclusiveMinimum"
            | "maxLength"
            | "minLength"
            | "pattern"
            | "maxItems"
            | "minItems"
            | "uniqueItems"
            | "maxProperties"
            | "minProperties"
            | "contentEncoding"
            | "contentMediaType"
            | "example"
            | "examples"
            | "$schema"
            | "$id"
            | "$anchor"
            | "$comment"
            | "unevaluatedProperties"
    )
}
