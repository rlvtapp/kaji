//! Deterministic, bounded wire fixtures for serialization consumer plugins.
//! These are structural round-trip inputs, not a complete JSON Schema validator.
use crate::{AdditionalProperties, Api, SchemaKind, SchemaValue};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug)]
pub struct SampleOptions {
    pub max_depth: usize,
    pub max_samples: usize,
    pub max_array_items: usize,
}
impl Default for SampleOptions {
    fn default() -> Self {
        Self {
            max_depth: 12,
            max_samples: 16,
            max_array_items: 3,
        }
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct WireSample {
    pub name: String,
    pub value: Value,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct SampleReport {
    pub samples: Vec<WireSample>,
    pub diagnostics: Vec<String>,
}

/// Produce minimal, full, nullable and enum/union alternatives. Bounds apply to
/// nesting, sample count and array length. Impossible required recursion yields
/// a diagnostic rather than an invalid null placeholder.
pub fn schema_samples(api: &Api, schema: &SchemaValue, options: SampleOptions) -> SampleReport {
    let mut report = SampleReport::default();
    if options.max_samples == 0 {
        return report;
    }
    for (name, full, nulls, alternative) in [
        ("minimal".to_owned(), false, false, 0),
        ("full".to_owned(), true, false, 0),
        ("nullable".to_owned(), true, true, 0),
    ]
    .into_iter()
    .chain(
        (1..options.max_samples).map(|choice| (format!("variant-{choice}"), true, false, choice)),
    ) {
        let mut seen = BTreeSet::new();
        match build(api, schema, options, 0, full, nulls, alternative, &mut seen) {
            Ok(value) => {
                if !report.samples.iter().any(|sample| sample.value == value) {
                    report.samples.push(WireSample { name, value });
                    if report.samples.len() >= options.max_samples {
                        break;
                    }
                }
            }
            Err(error) => {
                if !report.diagnostics.contains(&error) {
                    report.diagnostics.push(error);
                }
            }
        }
    }
    report
}

#[allow(clippy::too_many_arguments)]
fn build(
    api: &Api,
    schema: &SchemaValue,
    options: SampleOptions,
    depth: usize,
    full: bool,
    nulls: bool,
    choice: usize,
    seen: &mut BTreeSet<String>,
) -> Result<Value, String> {
    if let Some(value) = &schema.const_value {
        return Ok(value.clone());
    }
    if !schema.enum_values.is_empty() {
        return Ok(schema.enum_values[choice % schema.enum_values.len()].clone());
    }
    if (nulls || depth >= options.max_depth) && (schema.nullable || schema.nullish) {
        return Ok(Value::Null);
    }
    if depth > options.max_depth {
        return Err("sample nesting bound reached".into());
    }
    // Never claim fixtures satisfy constraints we do not implement.
    for keyword in schema.constraints.keys() {
        if !matches!(
            keyword.as_str(),
            "minimum"
                | "maximum"
                | "exclusiveMinimum"
                | "exclusiveMaximum"
                | "multipleOf"
                | "minLength"
                | "maxLength"
                | "minItems"
                | "maxItems"
                | "uniqueItems"
                | "minProperties"
                | "maxProperties"
        ) {
            return Err(format!(
                "sample generation does not support constraint {keyword}"
            ));
        }
    }
    let recurse = |value: &SchemaValue, seen: &mut BTreeSet<String>| {
        build(api, value, options, depth + 1, full, nulls, choice, seen)
    };
    let value = match &schema.kind {
        SchemaKind::Any => json!({"example": "value"}),
        SchemaKind::Null => Value::Null,
        SchemaKind::Boolean => Value::Bool(choice % 2 == 0),
        SchemaKind::Integer | SchemaKind::Number => {
            let integer = matches!(schema.kind, SchemaKind::Integer);
            let minimum = schema
                .constraints
                .get("minimum")
                .and_then(Value::as_f64)
                .unwrap_or_else(|| {
                    schema
                        .constraints
                        .get("maximum")
                        .and_then(Value::as_f64)
                        .or_else(|| {
                            schema
                                .constraints
                                .get("exclusiveMaximum")
                                .and_then(Value::as_f64)
                                .map(|maximum| maximum - 1.0)
                        })
                        .unwrap_or(0.0)
                        .min(0.0)
                });
            let exclusive_min = schema
                .constraints
                .get("exclusiveMinimum")
                .and_then(Value::as_f64);
            let mut number = exclusive_min.map_or(minimum, |minimum| minimum + 1.0);
            if integer {
                number = number.ceil();
            }
            if let Some(multiple) = schema.constraints.get("multipleOf").and_then(Value::as_f64) {
                if !multiple.is_finite() || multiple <= 0.0 {
                    return Err("invalid multipleOf constraint".into());
                }
                number = (number / multiple).ceil() * multiple;
                if integer && number.fract() != 0.0 {
                    return Err("cannot construct integer multipleOf fixture".into());
                }
            }
            if schema
                .constraints
                .get("maximum")
                .and_then(Value::as_f64)
                .is_some_and(|max| number > max)
                || schema
                    .constraints
                    .get("exclusiveMaximum")
                    .and_then(Value::as_f64)
                    .is_some_and(|max| number >= max)
            {
                return Err("incompatible numeric sample bounds".into());
            }
            if integer {
                if number < i64::MIN as f64 || number >= i64::MAX as f64 {
                    return Err("integer fixture exceeds signed 64-bit range".into());
                }
                if schema.format.as_deref() == Some("int64")
                    && schema.constraints.is_empty()
                    && choice > 0
                {
                    Value::from(if choice % 2 == 0 {
                        -9_007_199_254_740_993_i64
                    } else {
                        9_007_199_254_740_993_i64
                    })
                } else {
                    Value::from(number as i64)
                }
            } else {
                serde_json::Number::from_f64(number)
                    .map(Value::Number)
                    .ok_or("non-finite numeric fixture")?
            }
        }
        SchemaKind::String => {
            let example = match schema.format.as_deref() {
                Some("date") => "2024-01-02",
                Some("date-time") => "2024-01-02T03:04:05Z",
                Some("uuid") => "00000000-0000-4000-8000-000000000001",
                Some("email") => "sample@example.com",
                Some("uri" | "url") => "https://example.com/",
                Some("byte") => "aGVsbG8=",
                _ => "sample",
            };
            let min = usize_constraint(schema, "minLength", 0)?;
            let max = usize_constraint(schema, "maxLength", usize::MAX)?;
            if min > max || min > 4096 {
                return Err("string sample bounds exceed supported budget".into());
            }
            let mut value = example.to_owned();
            if schema.format.is_some() && (value.len() < min || value.len() > max) {
                return Err("format and length bounds need a custom fixture".into());
            }
            if value.len() < min {
                value.extend(std::iter::repeat_n('x', min - value.len()));
            }
            if value.len() > max {
                value.truncate(max);
            }
            Value::String(value)
        }
        SchemaKind::Array { items } => {
            let min = usize_constraint(schema, "minItems", 0)?;
            let max = usize_constraint(schema, "maxItems", options.max_array_items)?;
            if min > max || min > options.max_array_items {
                return Err("array sample bounds exceed configured budget".into());
            }
            let count = if full {
                min.max(1).min(max).min(options.max_array_items)
            } else {
                min
            };
            let mut values = Vec::new();
            for index in 0..count {
                let item = build(
                    api,
                    items,
                    options,
                    depth + 1,
                    full,
                    nulls,
                    choice + index,
                    seen,
                )?;
                if schema.constraints.get("uniqueItems") == Some(&Value::Bool(true))
                    && values.contains(&item)
                {
                    return Err("cannot construct sufficient unique array items".into());
                }
                values.push(item);
            }
            Value::Array(values)
        }
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let mut values = Map::new();
            for field in fields {
                if field.required || full {
                    match recurse(&field.value, seen) {
                        Ok(value) => {
                            values.insert(field.name.clone(), value);
                        }
                        Err(_) if !field.required => {}
                        Err(error) => {
                            return Err(format!("required field {}: {error}", field.name));
                        }
                    }
                }
            }
            if full {
                let mut name = "additional_example".to_owned();
                while values.contains_key(&name) {
                    name.push('_');
                }
                match additional_properties {
                    AdditionalProperties::Any | AdditionalProperties::Unspecified => {
                        values.insert(name, json!({"retained": true}));
                    }
                    AdditionalProperties::Schema { value } => {
                        if let Ok(value) = recurse(value, seen) {
                            values.insert(name, value);
                        }
                    }
                    AdditionalProperties::Forbidden => {}
                }
            }
            let min = usize_constraint(schema, "minProperties", 0)?;
            let max = usize_constraint(schema, "maxProperties", usize::MAX)?;
            if values.len() < min || values.len() > max {
                return Err("object sample does not fit property-count constraints".into());
            }
            Value::Object(values)
        }
        SchemaKind::Reference { reference } => {
            if !reference.starts_with("#/components/schemas/") && reference.contains('/') {
                return Err(format!("unresolved external sample reference {reference}"));
            }
            let name = reference
                .rsplit('/')
                .next()
                .unwrap()
                .replace("~1", "/")
                .replace("~0", "~");
            if !seen.insert(name.clone()) {
                return Err(format!("required recursive sample reference {name}"));
            }
            let target = api
                .schemas
                .iter()
                .find(|schema| schema.name == name)
                .ok_or_else(|| format!("sample reference {name} not found"))?;
            let result = recurse(&target.value, seen);
            seen.remove(&name);
            result?
        }
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => {
            if variants.is_empty() {
                return Err("empty sample union".into());
            }
            let index = choice % variants.len();
            let variant = &variants[index];
            let mut value = recurse(variant, seen)?;
            if let Some(discriminator) = &schema.discriminator {
                if let Some(object) = value.as_object_mut() {
                    let mapping = discriminator.mapping.iter().find(|(_, reference)| {
                        variant
                            .kind
                            .reference_name()
                            .is_some_and(|name| reference.rsplit('/').next() == Some(name))
                    });
                    if let Some((tag, _)) = mapping {
                        object.insert(
                            discriminator.property_name.clone(),
                            Value::String(tag.clone()),
                        );
                    }
                }
            }
            value
        }
        SchemaKind::AllOf { variants } => {
            let mut combined = Map::new();
            for variant in variants {
                let Value::Object(object) = recurse(variant, seen)? else {
                    return Err("non-object allOf requires custom fixture".into());
                };
                for (key, value) in object {
                    if combined
                        .get(&key)
                        .is_some_and(|existing| existing != &value)
                    {
                        return Err(format!("conflicting allOf sample property {key}"));
                    }
                    combined.insert(key, value);
                }
            }
            Value::Object(combined)
        }
        SchemaKind::Not { .. } => return Err("not schemas require a custom fixture".into()),
    };
    Ok(value)
}
fn usize_constraint(schema: &SchemaValue, name: &str, default: usize) -> Result<usize, String> {
    match schema.constraints.get(name) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("invalid {name} constraint")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Field, Schema};
    fn field(name: &str, value: SchemaValue, required: bool) -> Field {
        Field {
            name: name.into(),
            value,
            required,
            annotations: Default::default(),
        }
    }
    #[test]
    fn creates_omission_null_enum_and_additional_property_samples() {
        let mut enumeration = SchemaValue::new(SchemaKind::String);
        enumeration.enum_values = vec![json!("first"), json!("second")];
        let mut nullable = SchemaValue::new(SchemaKind::String);
        nullable.nullable = true;
        let schema = SchemaValue::new(SchemaKind::Object {
            fields: vec![
                field("state", enumeration, true),
                field("note", nullable, false),
            ],
            additional_properties: AdditionalProperties::Any,
        });
        let report = schema_samples(&Api::default(), &schema, SampleOptions::default());
        assert!(report.diagnostics.is_empty());
        assert!(
            report
                .samples
                .iter()
                .any(|sample| sample.value.get("note").is_none())
        );
        assert!(
            report
                .samples
                .iter()
                .any(|sample| sample.value.get("note") == Some(&Value::Null))
        );
        assert!(
            report
                .samples
                .iter()
                .any(|sample| sample.value["state"] == "second")
        );
        assert!(
            report
                .samples
                .iter()
                .any(|sample| sample.value.get("additional_example").is_some())
        );
    }
    #[test]
    fn recursive_optional_fields_terminate_required_cycles_report_failure() {
        let optional = SchemaValue::new(SchemaKind::Object {
            fields: vec![field(
                "next",
                SchemaValue::reference("#/components/schemas/Node"),
                false,
            )],
            additional_properties: AdditionalProperties::Forbidden,
        });
        let mut api = Api {
            schemas: vec![Schema::new("Node", optional)],
            ..Default::default()
        };
        let report = schema_samples(
            &api,
            &SchemaValue::reference("#/components/schemas/Node"),
            SampleOptions::default(),
        );
        assert!(!report.samples.is_empty());
        if let SchemaKind::Object { fields, .. } = &mut api.schemas[0].value.kind {
            fields[0].required = true;
        }
        let report = schema_samples(
            &api,
            &SchemaValue::reference("#/components/schemas/Node"),
            SampleOptions::default(),
        );
        assert!(report.samples.is_empty());
        assert!(!report.diagnostics.is_empty());
    }
    #[test]
    fn int64_fixtures_keep_digits_beyond_javascript_precision() {
        let mut schema = SchemaValue::new(SchemaKind::Integer);
        schema.format = Some("int64".into());
        let report = schema_samples(&Api::default(), &schema, SampleOptions::default());
        assert!(
            report
                .samples
                .iter()
                .any(|sample| sample.value.as_i64() == Some(9_007_199_254_740_993))
        );
        assert!(
            report
                .samples
                .iter()
                .any(|sample| sample.value.as_i64() == Some(-9_007_199_254_740_993))
        );
    }

    #[test]
    fn unions_bounds_and_unsupported_constraints_are_explicit() {
        let schema = SchemaValue::new(SchemaKind::OneOf {
            variants: vec![
                SchemaValue::new(SchemaKind::String),
                SchemaValue::new(SchemaKind::Integer),
            ],
        });
        let report = schema_samples(
            &Api::default(),
            &schema,
            SampleOptions {
                max_samples: 2,
                ..Default::default()
            },
        );
        assert_eq!(report.samples.len(), 2);
        assert!(report.samples.iter().any(|sample| sample.value.is_string()));
        assert!(report.samples.iter().any(|sample| sample.value.is_number()));
        let mut constrained = SchemaValue::new(SchemaKind::String);
        constrained
            .constraints
            .insert("pattern".into(), json!("^[A-Z]+$"));
        let report = schema_samples(&Api::default(), &constrained, SampleOptions::default());
        assert!(report.samples.is_empty());
        assert!(report.diagnostics[0].contains("pattern"));
    }
}
