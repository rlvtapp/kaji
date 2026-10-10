//! GraphQL variable samples, validation and credential redaction.
use super::*;

pub(super) fn sensitive(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "password",
        "secret",
        "token",
        "apikey",
        "api_key",
        "authorization",
    ]
    .iter()
    .any(|v| name.contains(v))
}
pub(super) fn fields(
    values: &[ModelField],
    c: &GraphqlOperations,
    seen: &mut BTreeSet<String>,
    depth: usize,
) -> Result<Value> {
    let mut result = serde_json::Map::new();
    let mut names = BTreeSet::new();
    for field in values {
        ensure!(
            names.insert(&field.name),
            "duplicate variable/input field {}",
            field.name
        );
        if !field.optional {
            result.insert(field.name.clone(), sample(&field.ty, c, seen, depth + 1)?);
        }
    }
    Ok(Value::Object(result))
}
pub(super) fn sample(
    t: &ModelType,
    c: &GraphqlOperations,
    seen: &mut BTreeSet<String>,
    depth: usize,
) -> Result<Value> {
    ensure!(depth < 32, "recursive/deep required inputs");
    if t.nullable {
        return Ok(Value::Null);
    }
    Ok(match &t.kind {
        ModelKind::Scalar(name) => match name.as_str() {
            "ID" | "String" => json!("example"),
            "Int" => json!(0),
            "Float" => json!(0.0),
            "Boolean" => json!(true),
            _ => bail!("custom scalar {name}"),
        },
        ModelKind::Enum(values) => json!(
            values
                .first()
                .ok_or_else(|| anyhow::anyhow!("empty enum"))?
        ),
        ModelKind::Literal(value) => json!(value),
        ModelKind::List(_) => json!([]),
        ModelKind::Object(values) => fields(values, c, seen, depth + 1)?,
        ModelKind::Named(name) => {
            ensure!(seen.insert(name.clone()), "recursive required input {name}");
            let result = fields(
                c.input_objects
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("unknown input object {name}"))?,
                c,
                seen,
                depth + 1,
            )?;
            seen.remove(name);
            result
        }
        ModelKind::Union(_) => bail!("input union unsupported"),
    })
}

pub(super) fn validate_fields(
    fields: &[ModelField],
    value: &Value,
    c: &GraphqlOperations,
    depth: usize,
) -> Result<()> {
    ensure!(
        depth < 32,
        "configured variables exceed supported input depth"
    );
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("GraphQL variables/input objects must be JSON objects"))?;
    for key in object.keys() {
        ensure!(
            fields.iter().any(|f| &f.name == key),
            "Unknown GraphQL variable/input field {key}"
        );
    }
    for field in fields {
        match object.get(&field.name) {
            Some(value) => validate_value(&field.ty, value, c, depth + 1)?,
            None => ensure!(
                field.optional,
                "Missing required GraphQL variable/input field {}",
                field.name
            ),
        }
    }
    Ok(())
}
pub(super) fn validate_value(
    t: &ModelType,
    value: &Value,
    c: &GraphqlOperations,
    depth: usize,
) -> Result<()> {
    if value.is_null() {
        ensure!(
            t.nullable,
            "Explicit null for non-null GraphQL variable/input field"
        );
        return Ok(());
    }
    match &t.kind {
        ModelKind::Scalar(name) => ensure!(
            match name.as_str() {
                "String" => value.is_string(),
                "ID" => value.is_string() || value.is_i64(),
                "Boolean" => value.is_boolean(),
                "Int" => value.as_i64().is_some_and(|n| i32::try_from(n).is_ok()),
                "Float" => value.is_number(),
                _ => true,
            },
            "Invalid GraphQL {name} variable value"
        ),
        ModelKind::Enum(values) => ensure!(
            value
                .as_str()
                .is_some_and(|v| values.iter().any(|item| item == v)),
            "Invalid GraphQL enum variable value"
        ),
        ModelKind::Literal(literal) => ensure!(
            value.as_str() == Some(literal),
            "Invalid GraphQL literal variable value"
        ),
        ModelKind::Object(fields) => validate_fields(fields, value, c, depth + 1)?,
        ModelKind::Named(name) => validate_fields(
            c.input_objects
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Unknown GraphQL input object {name}"))?,
            value,
            c,
            depth + 1,
        )?,
        ModelKind::List(inner) => {
            let values = value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("GraphQL list variables must be JSON arrays"))?;
            for value in values {
                validate_value(inner, value, c, depth + 1)?;
            }
        }
        ModelKind::Union(_) => bail!("GraphQL input union unsupported"),
    }
    Ok(())
}

pub(super) fn scrub(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (name, value) in fields {
                if sensitive(name) {
                    *value = match value {
                        Value::String(_) => json!("<redacted>"),
                        Value::Number(_) => json!(0),
                        Value::Bool(_) => json!(false),
                        Value::Array(_) => json!([]),
                        Value::Object(_) => json!({}),
                        Value::Null => Value::Null,
                    };
                } else {
                    scrub(value);
                }
            }
        }
        Value::Array(items) => {
            for value in items {
                scrub(value);
            }
        }
        _ => {}
    }
}
