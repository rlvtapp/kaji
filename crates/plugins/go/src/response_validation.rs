//! Opt-in structural response validation. Enum values and extra fields stay open.
use super::*;
use serde_json::{Value, json};
fn write_only(value: &SchemaValue, schemas: &[Schema], seen: &mut BTreeSet<String>) -> bool {
    if value.write_only {
        return true;
    }
    let Some(name) = value.kind.reference_name() else {
        return false;
    };
    if !seen.insert(name.to_owned()) {
        return false;
    }
    schemas
        .iter()
        .find(|schema| schema.name == name)
        .is_some_and(|schema| write_only(&schema.value, schemas, seen))
}
fn shape(value: &SchemaValue, schemas: &[Schema]) -> Value {
    let mut result = json!({"nullable":value.nullable || value.nullish});
    let kind = match &value.kind {
        SchemaKind::Any => "any",
        SchemaKind::Null => "null",
        SchemaKind::Boolean => "boolean",
        SchemaKind::Integer => "integer",
        SchemaKind::Number => "number",
        SchemaKind::String => "string",
        SchemaKind::Reference { .. } => {
            result["reference"] = json!(go_type_name(value.kind.reference_name().unwrap()));
            "reference"
        }
        SchemaKind::Array { items } => {
            result["items"] = shape(items, schemas);
            "array"
        }
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            if let AdditionalProperties::Schema { value } = additional_properties {
                result["additional"] = shape(value, schemas);
            }
            result["fields"] = Value::Object(
                fields
                    .iter()
                    .map(|field| {
                        (
                            field.name.clone(),
                            json!({"required":field.required && !write_only(&field.value, schemas, &mut BTreeSet::new()), "shape":shape(&field.value, schemas)}),
                        )
                    })
                    .collect(),
            );
            "object"
        }
        // Composition constraints are intentionally not enforced by this narrow validator.
        _ => "any",
    };
    result["kind"] = json!(kind);
    result
}
pub(super) fn render(api: &Api) -> String {
    let composed = composition::models(api);
    let schemas = composed.as_deref().unwrap_or(&api.schemas);
    let registry = schemas
        .iter()
        .map(|schema| (go_type_name(&schema.name), shape(&schema.value, schemas)))
        .collect::<serde_json::Map<_, _>>();
    let literal = serde_json::to_string(&Value::Object(registry)).unwrap();
    let mut output = include_str!("go_response_validation.txt").to_string();
    output = output.replace(
        "KAJI_SCHEMA_LITERAL",
        &serde_json::to_string(&literal).unwrap(),
    );
    output
}
