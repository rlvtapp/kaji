use super::*;

pub(crate) fn representation(value: &SchemaValue, options: &ModelOptions) -> Int64Type {
    if !matches!(value.kind, SchemaKind::Integer) {
        return Int64Type::Number;
    }
    if options.integer_as_string {
        Int64Type::String
    } else if value.format.as_deref() == Some("int64") {
        options.int64_type
    } else {
        Int64Type::Number
    }
}
pub(crate) fn plan(value: &SchemaValue, options: &ModelOptions) -> Value {
    let mut descriptor = match &value.kind {
        SchemaKind::Integer => match representation(value, options) {
            Int64Type::Number => json!({"kind":"integer"}),
            Int64Type::String => json!({"kind":"integer","integer":"string"}),
            Int64Type::BigInt => json!({"kind":"integer","integer":"bigint"}),
        },
        SchemaKind::Reference { reference } => {
            json!({"ref":reference.rsplit('/').next().unwrap_or(reference)})
        }
        SchemaKind::Array { items } => json!({"kind":"array","items":plan(items, options)}),
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let fields = fields
                .iter()
                .map(|f| (f.name.clone(), plan(&f.value, options)))
                .collect::<serde_json::Map<_, _>>();
            let additional = match additional_properties {
                AdditionalProperties::Schema { value } => plan(value, options),
                _ => Value::Null,
            };
            json!({"kind":"object","fields":fields,"required":value_object_required(value),"additional":additional})
        }
        SchemaKind::AllOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::OneOf { variants } => {
            json!({"kind":if matches!(value.kind,SchemaKind::AllOf {..}) {"allOf"} else {"union"},"variants":variants.iter().map(|v| plan(v,options)).collect::<Vec<_>>()})
        }
        SchemaKind::String => json!({"kind":"string"}),
        SchemaKind::Number => json!({"kind":"number"}),
        SchemaKind::Boolean => json!({"kind":"boolean"}),
        SchemaKind::Null => json!({"kind":"null"}),
        _ => Value::Null,
    };
    if !descriptor.is_null() {
        descriptor["nullable"] = Value::Bool(value.nullable || value.nullish);
        if value.write_only {
            descriptor["writeOnly"] = Value::Bool(true);
        }
        if let Some(literal) = &value.const_value {
            descriptor["literals"] = json!([literal]);
        } else if !value.enum_values.is_empty() {
            descriptor["literals"] = json!(value.enum_values);
        }
    }
    descriptor
}
pub(crate) fn value_object_required(value: &SchemaValue) -> Vec<&str> {
    if let SchemaKind::Object { fields, .. } = &value.kind {
        fields
            .iter()
            .filter(|field| field.required)
            .map(|field| field.name.as_str())
            .collect()
    } else {
        vec![]
    }
}
#[cfg(test)]
pub(crate) fn operation_plan(
    api: &Api,
    operation: &poolster_core::Operation,
    options: &ModelOptions,
) -> Option<Value> {
    operation_plan_impl(api, operation, options, true)
}

pub(crate) fn operation_inline_plan(
    api: &Api,
    operation: &poolster_core::Operation,
    options: &ModelOptions,
) -> Option<Value> {
    operation_plan_impl(api, operation, options, false)
}

pub(crate) fn operation_plan_impl(
    api: &Api,
    operation: &poolster_core::Operation,
    options: &ModelOptions,
    include_refs: bool,
) -> Option<Value> {
    let schemas = api
        .schemas
        .iter()
        .map(|schema| (schema.name.as_str(), &schema.value))
        .collect::<std::collections::BTreeMap<_, _>>();
    let requests = operation
        .request_body
        .iter()
        .flat_map(|b| &b.media_types)
        .filter_map(|m| {
            m.schema
                .as_ref()
                .map(|s| (m.content_type.clone(), plan(s, options)))
        })
        .collect::<serde_json::Map<_, _>>();
    let responses = operation
        .responses
        .iter()
        .map(|r| {
            (
                r.status.clone(),
                Value::Object(
                    r.media_types
                        .iter()
                        .filter_map(|m| {
                            m.schema
                                .as_ref()
                                .map(|s| (m.content_type.clone(), plan(s, options)))
                        })
                        .collect(),
                ),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    if !include_refs {
        return Some(
            json!({"lossless":options.integer_as_string || options.int64_type != Int64Type::Number,"refs":null,"requests":requests,"responses":responses}),
        );
    }
    fn collect(value: &Value, names: &mut std::collections::BTreeSet<String>) {
        match value {
            Value::Object(values) => {
                if let Some(Value::String(name)) = values.get("ref") {
                    names.insert(name.clone());
                }
                for value in values.values() {
                    collect(value, names);
                }
            }
            Value::Array(values) => {
                for value in values {
                    collect(value, names);
                }
            }
            _ => {}
        }
    }
    let mut pending = std::collections::BTreeSet::new();
    collect(&Value::Object(requests.clone()), &mut pending);
    collect(&Value::Object(responses.clone()), &mut pending);
    let mut refs = serde_json::Map::new();
    while let Some(name) = pending.pop_first() {
        if refs.contains_key(&name) {
            continue;
        }
        if let Some(schema) = schemas.get(name.as_str()) {
            let descriptor = plan(schema, options);
            collect(&descriptor, &mut pending);
            refs.insert(name, descriptor);
        }
    }
    Some(
        json!({"lossless":options.integer_as_string || options.int64_type != Int64Type::Number,"refs":refs,"requests":requests,"responses":responses}),
    )
}

/// Schema definitions are emitted once per operation provider rather than
/// repeating their complete transitive closure in every operation module.
pub(crate) fn shared_refs(
    api: &Api,
    options: &ModelOptions,
) -> anyhow::Result<Vec<(String, String)>> {
    const CHUNK_BYTES: usize = 128 * 1024;
    let shape = RUNTIME
        .lines()
        .find(|line| line.starts_with("export type JsonShape"))
        .unwrap();
    let mut files = Vec::new();
    let mut chunk = serde_json::Map::new();
    let mut bytes = 0;
    for schema in &api.schemas {
        let descriptor = plan(&schema.value, options);
        let size = serde_json::to_string(&descriptor)?.len() + schema.name.len() + 4;
        if bytes + size > CHUNK_BYTES && !chunk.is_empty() {
            let name = format!("_poolster_json_refs_{:04}", files.len() + 1);
            files.push((
                name,
                format!(
                    "{shape}\nexport const refs: Record<string, JsonShape | null> = {}\n",
                    serde_json::to_string(&chunk)?
                ),
            ));
            chunk.clear();
            bytes = 0;
        }
        chunk.insert(schema.name.clone(), descriptor);
        bytes += size;
    }
    if !chunk.is_empty() {
        let name = format!("_poolster_json_refs_{:04}", files.len() + 1);
        files.push((
            name,
            format!(
                "{shape}\nexport const refs: Record<string, JsonShape | null> = {}\n",
                serde_json::to_string(&chunk)?
            ),
        ));
    }
    let mut root = format!("{shape}\n");
    for (index, (name, _)) in files.iter().enumerate() {
        root.push_str(&format!(
            "import {{ refs as chunk{index} }} from './{name}'\n"
        ));
    }
    root.push_str("export const poolsterJsonRefs: Record<string, JsonShape | null> = {\n");
    for index in 0..files.len() {
        root.push_str(&format!("  ...chunk{index},\n"));
    }
    root.push_str("}\n");
    files.push(("_poolster_json_refs".into(), root));
    Ok(files)
}
