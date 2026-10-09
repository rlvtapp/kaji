//! Kafka binding validation and lossless supported schema/reference lowering.
use anyhow::{Context, Result, bail, ensure};
use poolster_core::{AdditionalProperties, Field, SchemaKind, SchemaValue};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn fixed_string(value: Option<&Value>) -> Result<Option<String>> {
    value
        .map(|v| {
            ensure!(
                v["type"] == "string"
                    && v.as_object().is_some_and(|o| o
                        .keys()
                        .all(|k| matches!(k.as_str(), "type" | "enum" | "description" | "title"))),
                "groupId/clientId bindings only support explicit singleton string enums"
            );
            let values = v["enum"]
                .as_array()
                .context("groupId/clientId bindings require singleton string enum")?;
            ensure!(
                values.len() == 1,
                "groupId/clientId bindings require singleton enum"
            );
            Ok(values[0]
                .as_str()
                .context("binding value must be string")?
                .to_owned())
        })
        .transpose()
}
pub(super) fn bindings(value: Option<&Value>, allowed: &[&str]) -> Result<Value> {
    let Some(value) = value else {
        return Ok(json!({}));
    };
    let protocols = value.as_object().context("bindings must be object")?;
    ensure!(
        protocols.keys().all(|k| k == "kafka"),
        "non-Kafka bindings unsupported"
    );
    let kafka = protocols.get("kafka").cloned().unwrap_or(json!({}));
    ensure!(
        kafka
            .as_object()
            .context("Kafka bindings must be object")?
            .keys()
            .all(|k| allowed.contains(&k.as_str())),
        "unsupported Kafka binding field"
    );
    ensure!(
        kafka
            .get("bindingVersion")
            .is_none_or(|v| matches!(v.as_str(), Some("0.4.0" | "0.5.0"))),
        "unsupported Kafka binding version"
    );
    Ok(kafka)
}
pub(crate) fn resolve<'a>(root: &'a Value, mut value: &'a Value) -> Result<&'a Value> {
    let mut seen = BTreeSet::new();
    while let Some(reference) = value["$ref"].as_str() {
        ensure!(seen.insert(reference), "cyclic AsyncAPI reference");
        value = root
            .pointer(
                reference
                    .strip_prefix('#')
                    .context("external references unsupported")?,
            )
            .context("unresolved AsyncAPI reference")?;
    }
    Ok(value)
}
pub(crate) fn schema(
    root: &Value,
    value: &Value,
    seen: &mut BTreeSet<String>,
) -> Result<(SchemaValue, Value)> {
    if let Some(reference) = value["$ref"].as_str() {
        ensure!(
            seen.insert(reference.into()),
            "recursive message schemas unsupported"
        );
        let out = schema(
            root,
            root.pointer(
                reference
                    .strip_prefix('#')
                    .context("external schema refs unsupported")?,
            )
            .context("schema reference missing")?,
            seen,
        );
        seen.remove(reference);
        return out;
    }
    let fields = value
        .as_object()
        .context("message schema must be JSON Schema object")?;
    const ALLOWED: &[&str] = &[
        "type",
        "properties",
        "required",
        "additionalProperties",
        "items",
        "enum",
        "const",
        "default",
        "title",
        "description",
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "minLength",
        "maxLength",
        "pattern",
        "minItems",
        "maxItems",
        "uniqueItems",
        "minProperties",
        "maxProperties",
        "examples",
        "$schema",
    ];
    ensure!(
        fields
            .keys()
            .all(|k| ALLOWED.contains(&k.as_str()) || k.starts_with("x-")),
        "unsupported JSON Schema keyword"
    );
    ensure!(
        value.get("$schema").is_none_or(|v| matches!(
            v.as_str(),
            Some(
                "http://json-schema.org/draft-07/schema#"
                    | "https://json-schema.org/draft-07/schema"
                    | "https://json-schema.org/draft-07/schema#"
            )
        )),
        "event JSON Schema dialect must be draft-07"
    );
    let mut normalized = value.clone();
    let kind = match value["type"]
        .as_str()
        .context("message schema requires explicit type")?
    {
        "string" => SchemaKind::String,
        "integer" => SchemaKind::Integer,
        "number" => SchemaKind::Number,
        "boolean" => SchemaKind::Boolean,
        "null" => SchemaKind::Null,
        "array" => {
            let (items, json) = schema(root, &value["items"], seen)?;
            normalized["items"] = json;
            SchemaKind::Array {
                items: Box::new(items),
            }
        }
        "object" => {
            let required = value["required"]
                .as_array()
                .map(|v| v.iter().filter_map(Value::as_str).collect::<BTreeSet<_>>())
                .unwrap_or_default();
            ensure!(
                required
                    .iter()
                    .all(|name| value["properties"].get(*name).is_some()),
                "required event fields need explicit property schemas"
            );
            let mut properties = Vec::new();
            if let Some(props) = value["properties"].as_object() {
                for (name, v) in props {
                    let (ty, json) = schema(root, v, seen)?;
                    normalized["properties"][name] = json;
                    properties.push(Field {
                        name: name.clone(),
                        required: required.contains(name.as_str()),
                        value: ty,
                        annotations: Default::default(),
                    });
                }
            }
            let additional_properties = match value.get("additionalProperties") {
                None => AdditionalProperties::Unspecified,
                Some(Value::Bool(true)) => AdditionalProperties::Any,
                Some(Value::Bool(false)) => AdditionalProperties::Forbidden,
                Some(v) => {
                    let (ty, json) = schema(root, v, seen)?;
                    normalized["additionalProperties"] = json;
                    AdditionalProperties::Schema {
                        value: Box::new(ty),
                    }
                }
            };
            SchemaKind::Object {
                fields: properties,
                additional_properties,
            }
        }
        other => bail!("unsupported JSON Schema type {other}"),
    };
    let mut out = SchemaValue::new(kind);
    out.enum_values = value["enum"].as_array().cloned().unwrap_or_default();
    out.const_value = value.get("const").cloned();
    out.default = value.get("default").cloned();
    out.description = value["description"].as_str().map(str::to_owned);
    for (key, value) in fields {
        if !matches!(
            key.as_str(),
            "type"
                | "properties"
                | "required"
                | "additionalProperties"
                | "items"
                | "enum"
                | "const"
                | "default"
                | "description"
        ) {
            out.constraints.insert(key.clone(), value.clone());
        }
    }
    Ok((out, normalized))
}

pub(crate) fn pointer_segment(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
pub(crate) fn location<'a>(root: &'a Value, mut value: &'a Value, initial: &str) -> Result<String> {
    let mut pointer = initial.to_owned();
    let mut seen = BTreeSet::new();
    while let Some(reference) = value["$ref"].as_str() {
        ensure!(seen.insert(reference), "cyclic message reference");
        pointer = reference.into();
        value = root
            .pointer(
                reference
                    .strip_prefix('#')
                    .context("external message reference")?,
            )
            .context("missing message location")?;
    }
    Ok(pointer)
}
