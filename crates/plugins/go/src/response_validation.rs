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
    let mut output = include_str!("../templates/response_validation.go.tmpl").to_string();
    output = output.replace(
        "POOLSTER_SCHEMA_LITERAL",
        &serde_json::to_string(&literal).unwrap(),
    );
    output
}

/// Keep descriptors separate from HTTP runtime code. Chunk by declaration count
/// and serialized bytes; each schema remains atomic so wire validation is unchanged.
pub(super) fn split_files(api: &Api) -> Vec<(String, String)> {
    let composed = composition::models(api);
    let schemas = composed.as_deref().unwrap_or(&api.schemas);
    let mut chunks = Vec::new();
    let mut current = serde_json::Map::new();
    let mut bytes = 0usize;
    for schema in schemas {
        let value = shape(&schema.value, schemas);
        let size = serde_json::to_vec(&value).unwrap().len() + schema.name.len();
        if !current.is_empty() && (current.len() >= 100 || bytes + size > 128 * 1024) {
            chunks.push(std::mem::take(&mut current));
            bytes = 0;
        }
        current.insert(go_type_name(&schema.name), value);
        bytes += size;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    let mut registry = String::from(
        "var poolsterResponseShapes = func() map[string]poolsterResponseShape {\nshapes := make(map[string]poolsterResponseShape)\n",
    );
    let mut files = Vec::new();
    for (index, chunk) in chunks.into_iter().enumerate() {
        let name = format!("poolsterResponseShapesChunk{index:04}");
        registry.push_str(&format!(
            "for name, shape := range {name}() {{ shapes[name] = shape }}\n"
        ));
        let literal = serde_json::to_string(&Value::Object(chunk)).unwrap();
        let escaped = serde_json::to_string(&literal).unwrap();
        files.push((format!("response_shapes_{index:04}.go"), format!("func {name}() map[string]poolsterResponseShape {{\nvar shapes map[string]poolsterResponseShape\nif err := json.Unmarshal([]byte({escaped}), &shapes); err != nil {{ panic(\"poolster: invalid generated response descriptors\") }}\nreturn shapes\n}}\n")));
    }
    registry.push_str("return shapes\n}()\n");
    let runtime = include_str!("../templates/response_validation.go.tmpl");
    let start = runtime.find("var poolsterResponseShapes =").unwrap();
    let end = runtime.find("func poolsterValidateResponse").unwrap();
    files.push((
        "response_validation.go".into(),
        format!("{}{}{}", &runtime[..start], registry, &runtime[end..]),
    ));
    files
}
