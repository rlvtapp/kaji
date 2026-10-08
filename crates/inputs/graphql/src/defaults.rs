//! SDL default coercion supplements Apollo's reference and structure validation.
use anyhow::{Result, anyhow, bail, ensure};
use apollo_compiler::{
    Schema,
    ast::{InputValueDefinition, Type, Value},
    schema::ExtendedType,
};

pub(crate) fn validate(schema: &Schema) -> Result<()> {
    let check = |input: &InputValueDefinition| -> Result<()> {
        if let Some(value) = &input.default_value {
            check_value(schema, &input.ty, value).map_err(|error| {
                anyhow!(
                    "invalid GraphQL schema: schema.graphql: default for {}: {error}",
                    input.name
                )
            })?;
        }
        Ok(())
    };
    for definition in schema.types.values() {
        match definition {
            ExtendedType::Object(object) => {
                for field in object.fields.values() {
                    for argument in &field.arguments {
                        check(argument)?;
                    }
                }
            }
            ExtendedType::Interface(interface) => {
                for field in interface.fields.values() {
                    for argument in &field.arguments {
                        check(argument)?;
                    }
                }
            }
            ExtendedType::InputObject(object) => {
                for field in object.fields.values() {
                    check(field)?;
                }
            }
            _ => {}
        }
    }
    for directive in schema.directive_definitions.values() {
        for argument in &directive.arguments {
            check(argument)?;
        }
    }
    Ok(())
}

fn check_value(schema: &Schema, ty: &Type, value: &Value) -> Result<()> {
    if matches!(value, Value::Null) {
        ensure!(!ty.is_non_null(), "null is incompatible with {ty}");
        return Ok(());
    }
    match ty {
        Type::List(inner) | Type::NonNullList(inner) => {
            if let Value::List(values) = value {
                for item in values {
                    check_value(schema, inner, item)?;
                }
            } else {
                // Single input values may be coerced into lists, including nested lists.
                check_value(schema, inner, value)?;
            }
        }
        Type::Named(name) | Type::NonNullNamed(name) => match &schema.types[name] {
            ExtendedType::Scalar(_) => {
                let valid = match name.as_str() {
                    "Int" => matches!(value, Value::Int(number) if number.try_to_i32().is_ok()),
                    "Float" => match value {
                        Value::Int(number) => {
                            number.as_str().parse::<f64>().is_ok_and(f64::is_finite)
                        }
                        Value::Float(number) => {
                            number.to_string().parse::<f64>().is_ok_and(f64::is_finite)
                        }
                        _ => false,
                    },
                    "String" => matches!(value, Value::String(_)),
                    "Boolean" => matches!(value, Value::Boolean(_)),
                    "ID" => matches!(value, Value::String(_) | Value::Int(_)),
                    // Custom scalar coercion is supplied by its runtime implementation.
                    _ => !matches!(value, Value::Variable(_)),
                };
                ensure!(valid, "{value} is incompatible with {ty}");
            }
            ExtendedType::Enum(enumeration) => ensure!(
                matches!(value, Value::Enum(name) if enumeration.values.contains_key(name)),
                "{value} is incompatible with {ty}"
            ),
            ExtendedType::InputObject(object) => {
                let Value::Object(fields) = value else {
                    bail!("{value} is incompatible with {ty}")
                };
                let mut seen = std::collections::HashSet::new();
                for (name, value) in fields {
                    ensure!(seen.insert(name), "duplicate input field {name}");
                    let field = object
                        .fields
                        .get(name)
                        .ok_or_else(|| anyhow!("unknown input field {name} on {ty}"))?;
                    check_value(schema, &field.ty, value)?;
                }
                for (name, field) in &object.fields {
                    ensure!(
                        !field.ty.is_non_null()
                            || field.default_value.is_some()
                            || seen.contains(name),
                        "missing required input field {name} on {ty}"
                    );
                }
            }
            _ => bail!("{ty} is not an input type"),
        },
    }
    Ok(())
}
