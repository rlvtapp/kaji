use super::*;

pub(crate) fn resource_operations(api: &Api) -> BTreeMap<String, Vec<&Operation>> {
    let mut groups = BTreeMap::new();
    for operation in &api.operations {
        let resource = operation
            .path
            .trim_matches('/')
            .split('/')
            .next()
            .filter(|part| !part.is_empty())
            .unwrap_or("default");
        groups
            .entry(snake_case(resource))
            .or_insert_with(Vec::new)
            .push(operation);
    }
    groups
}

pub(crate) fn response_model(api: &Api, operation: &Operation) -> Option<String> {
    let reference = operation.success_schema()?.kind.reference_name()?;
    api.schemas
        .iter()
        .find(|schema| schema.name == reference)
        .and_then(|schema| {
            matches!(schema.value.kind, SchemaKind::Object { .. }).then(|| pascal_case(reference))
        })
}

pub(crate) fn ruby_type(value: &SchemaValue) -> String {
    match &value.kind {
        SchemaKind::Any | SchemaKind::Not { .. } => "Object".into(),
        SchemaKind::Null => "NilClass".into(),
        SchemaKind::Boolean => "TrueClass | FalseClass".into(),
        SchemaKind::Integer => "Integer".into(),
        SchemaKind::Number => "Numeric".into(),
        SchemaKind::String => "String".into(),
        SchemaKind::Array { items } => format!("Array # {}", ruby_type(items)),
        SchemaKind::Object { .. } => "Hash".into(),
        SchemaKind::Reference { reference } => {
            pascal_case(reference.rsplit('/').next().unwrap_or(reference))
        }
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => variants
            .iter()
            .map(ruby_type)
            .collect::<Vec<_>>()
            .join(" | "),
        SchemaKind::AllOf { .. } => "Hash".into(),
    }
}
