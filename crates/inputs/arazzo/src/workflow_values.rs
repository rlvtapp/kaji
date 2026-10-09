//! Conservative executable subset of Arazzo; unsupported semantics fail before emission.
use anyhow::{Context, Result, bail, ensure};
use poolster_core::native::{ModelField, ModelKind, ModelType, workflows::*};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn nonempty<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .filter(|v| !v.is_empty())
        .with_context(|| format!("missing string {key}"))
}
pub(super) fn reject(value: &Value, keys: &[&str]) -> Result<()> {
    for key in keys {
        ensure!(
            value.get(*key).is_none(),
            "unsupported Arazzo runner feature {key}"
        );
    }
    Ok(())
}
pub(super) fn string_list(value: &Value) -> Result<Vec<String>> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .context("expected string list")
        })
        .collect()
}
pub(super) fn schema_type(schema: &Value) -> Result<ModelType> {
    reject(
        schema,
        &[
            "$ref",
            "oneOf",
            "anyOf",
            "allOf",
            "not",
            "patternProperties",
            "additionalProperties",
            "format",
            "pattern",
            "minimum",
            "maximum",
            "minLength",
            "maxLength",
            "minItems",
            "maxItems",
            "const",
            "if",
            "then",
            "else",
            "dependentSchemas",
            "unevaluatedProperties",
            "uniqueItems",
        ],
    )?;
    ensure!(
        schema.get("enum").is_none() || schema["type"] == "string",
        "only string input enums are supported"
    );
    let kind = match schema["type"]
        .as_str()
        .context("workflow input schemas require explicit type")?
    {
        "string" => {
            if let Some(values) = schema["enum"].as_array() {
                ModelKind::Enum(
                    values
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .map(str::to_owned)
                                .context("string enum required")
                        })
                        .collect::<Result<_>>()?,
                )
            } else {
                ModelKind::Scalar("String".into())
            }
        }
        "integer" => ModelKind::Scalar("Int".into()),
        "number" => ModelKind::Scalar("Float".into()),
        "boolean" => ModelKind::Scalar("Boolean".into()),
        "array" => ModelKind::List(Box::new(schema_type(&schema["items"])?)),
        "object" => ModelKind::Object(schema_fields(schema)?),
        other => bail!("unsupported workflow input schema type {other}"),
    };
    Ok(ModelType {
        nullable: schema["nullable"].as_bool().unwrap_or(false),
        kind,
    })
}
fn schema_fields(schema: &Value) -> Result<Vec<ModelField>> {
    ensure!(
        schema["type"] == "object",
        "workflow inputs must have object type"
    );
    let required = string_list(&schema["required"])?;
    if let Some(properties) = schema.get("properties") {
        ensure!(properties.is_object(), "input properties must be an object");
    }
    let mut fields = Vec::new();
    for (name, schema) in schema["properties"].as_object().into_iter().flatten() {
        fields.push(ModelField {
            name: name.clone(),
            ty: schema_type(schema)?,
            optional: !required.contains(name),
            default_value: schema.get("default").map(Value::to_string),
        });
    }
    ensure!(
        required
            .iter()
            .all(|name| fields.iter().any(|field| &field.name == name)),
        "workflow input required name has no property"
    );
    Ok(fields)
}
pub(super) fn expression(value: &Value) -> Result<WorkflowValue> {
    Ok(match value {
        Value::String(text) if text.starts_with('$') => {
            if text == "$statusCode" {
                WorkflowValue::StatusCode
            } else if let Some(pointer) = text.strip_prefix("$response.body#") {
                ensure!(
                    pointer.is_empty() || pointer.starts_with('/'),
                    "response body requires JSON pointer"
                );
                let mut characters = pointer.chars();
                while let Some(character) = characters.next() {
                    if character == '~' {
                        ensure!(
                            matches!(characters.next(), Some('0' | '1')),
                            "invalid response JSON pointer escape"
                        );
                    }
                    ensure!(
                        character != '%',
                        "percent-encoded response pointers are unsupported"
                    );
                }
                WorkflowValue::ResponseBody(pointer.to_owned())
            } else if let Some(name) = text.strip_prefix("$inputs.") {
                ensure!(
                    !name.is_empty() && !name.contains('.'),
                    "only direct workflow input names are supported"
                );
                WorkflowValue::Input(name.to_owned())
            } else if let Some(reference) = text.strip_prefix("$steps.") {
                let (step, name) = reference
                    .split_once(".outputs.")
                    .context("step expression requires .outputs.<name>")?;
                ensure!(
                    !step.is_empty() && !name.is_empty() && !name.contains('.'),
                    "invalid step output expression"
                );
                WorkflowValue::StepOutput {
                    step: step.to_owned(),
                    name: name.to_owned(),
                }
            } else if let Some(reference) = text.strip_prefix("$workflows.") {
                let (workflow, name) = reference
                    .split_once(".outputs.")
                    .context("workflow expression requires .outputs.<name>")?;
                ensure!(
                    !workflow.is_empty() && !name.is_empty() && !name.contains('.'),
                    "invalid workflow output expression"
                );
                WorkflowValue::WorkflowOutput {
                    workflow: workflow.to_owned(),
                    name: name.to_owned(),
                }
            } else {
                bail!("unsupported Arazzo runtime expression {text:?}");
            }
        }
        Value::String(text) if text.contains("{$") => {
            bail!("interpolated Arazzo expressions are unsupported")
        }
        Value::Object(object) => WorkflowValue::Object(
            object
                .iter()
                .map(|(name, value)| Ok((name.clone(), expression(value)?)))
                .collect::<Result<_>>()?,
        ),
        Value::Array(values) => {
            WorkflowValue::Array(values.iter().map(expression).collect::<Result<_>>()?)
        }
        value => WorkflowValue::Literal(value.clone()),
    })
}
pub(super) fn output_map(value: &Value) -> Result<BTreeMap<String, WorkflowValue>> {
    value
        .as_object()
        .into_iter()
        .flatten()
        .map(|(name, value)| Ok((name.clone(), expression(value)?)))
        .collect()
}
pub(super) fn validate_value(
    value: &WorkflowValue,
    fields: &[ModelField],
    steps: &BTreeMap<String, Vec<String>>,
    dependencies: &[String],
    workflow_outputs: &BTreeMap<String, Vec<String>>,
    response: bool,
) -> Result<()> {
    match value {
        WorkflowValue::Input(name) => ensure!(
            fields.iter().any(|field| &field.name == name),
            "unknown workflow input {name:?}"
        ),
        WorkflowValue::StepOutput { step, name } => ensure!(
            steps.get(step).is_some_and(|names| names.contains(name)),
            "unknown or future step output {step}.{name}"
        ),
        WorkflowValue::WorkflowOutput { workflow, name } => ensure!(
            dependencies.contains(workflow)
                && workflow_outputs
                    .get(workflow)
                    .is_some_and(|names| names.contains(name)),
            "workflow output must reference a declared dependency: {workflow}.{name}"
        ),
        WorkflowValue::ResponseBody(_) | WorkflowValue::StatusCode => ensure!(
            response,
            "response expressions are supported only in step outputs"
        ),
        WorkflowValue::Object(object) => {
            for value in object.values() {
                validate_value(
                    value,
                    fields,
                    steps,
                    dependencies,
                    workflow_outputs,
                    response,
                )?;
            }
        }
        WorkflowValue::Array(array) => {
            for value in array {
                validate_value(
                    value,
                    fields,
                    steps,
                    dependencies,
                    workflow_outputs,
                    response,
                )?;
            }
        }
        WorkflowValue::Literal(_) => (),
    }
    Ok(())
}
