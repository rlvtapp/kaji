//! Generated opt-in structural response checks for buffered Ruby JSON responses.
use super::*;
use serde_json::Value;
use serde_json::json;
pub(super) fn write_only(
    api: &Api,
    value: &SchemaValue,
    seen: &mut std::collections::BTreeSet<String>,
) -> bool {
    if value.write_only {
        return true;
    }
    if let SchemaKind::Reference { reference } = &value.kind {
        let name = reference.rsplit('/').next().unwrap_or(reference);
        if seen.insert(name.into()) {
            return api
                .schemas
                .iter()
                .find(|s| s.name == name)
                .is_some_and(|s| write_only(api, &s.value, seen));
        }
    }
    false
}
fn shape(api: &Api, value: &SchemaValue) -> Value {
    let mut result = match &value.kind {
        SchemaKind::Reference { reference } => {
            json!({"ref":reference.rsplit('/').next().unwrap_or(reference)})
        }
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let fields_map = fields
                .iter()
                .map(|f| (f.name.clone(), shape(api, &f.value)))
                .collect::<serde_json::Map<_, _>>();
            let required = fields
                .iter()
                .filter(|f| f.required && !write_only(api, &f.value, &mut Default::default()))
                .map(|f| &f.name)
                .collect::<Vec<_>>();
            let additional = if let AdditionalProperties::Schema { value } = additional_properties {
                shape(api, value)
            } else {
                Value::Null
            };
            json!({"kind":"object","fields":fields_map,"required":required,"additional":additional})
        }
        SchemaKind::Array { items } => json!({"kind":"array","items":shape(api,items)}),
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            json!({"kind":if matches!(value.kind,SchemaKind::AllOf {..}) {"allOf"} else {"union"},"variants":variants.iter().map(|v|shape(api,v)).collect::<Vec<_>>()})
        }
        SchemaKind::String => json!({"kind":"string"}),
        SchemaKind::Boolean => json!({"kind":"boolean"}),
        SchemaKind::Integer => json!({"kind":"integer"}),
        SchemaKind::Number => json!({"kind":"number"}),
        SchemaKind::Null => json!({"kind":"null"}),
        _ => json!({"kind":"any"}),
    };
    result["nullable"] = json!(value.nullable || value.nullish);
    result
}
pub(super) fn render(api: &Api, module: &str) -> String {
    let refs = api
        .schemas
        .iter()
        .map(|s| (s.name.clone(), shape(api, &s.value)))
        .collect::<serde_json::Map<_, _>>();
    let operations = api
        .operations
        .iter()
        .map(|op| {
            let responses = op
                .responses
                .iter()
                .map(|response| {
                    let media = response
                        .media_types
                        .iter()
                        .filter_map(|media| {
                            media.schema.as_ref().map(|schema| {
                                (media.content_type.to_ascii_lowercase(), shape(api, schema))
                            })
                        })
                        .collect::<serde_json::Map<_, _>>();
                    (response.status.to_ascii_uppercase(), Value::Object(media))
                })
                .collect::<serde_json::Map<_, _>>();
            (op.id.clone(), Value::Object(responses))
        })
        .collect::<serde_json::Map<_, _>>();
    let catalog = serde_json::to_string(&json!({"refs":refs,"operations":operations})).unwrap();
    format!(
        "{NOTICE}require \"json\"\nmodule {module}\n{}\n  ResponseShapes = JSON.parse({})\nend\n",
        include_str!("response_validation.rb"),
        ruby_string(&catalog)
    )
}
