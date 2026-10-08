use super::*;

pub(super) fn response_type(operation: &Operation) -> String {
    if operation_is_sse_response(operation) {
        return "Iterator[Any]".into();
    }
    if matches!(
        analyze_operation(operation, None).streaming,
        Some(StreamingKind::Binary)
    ) {
        return "bytes".into();
    }
    operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .and_then(|response| response.media_types.first())
        .and_then(|media| media.schema.as_ref())
        .map(python_type)
        .unwrap_or_else(|| "None".into())
}

pub(super) fn operation_is_sse_response(operation: &Operation) -> bool {
    operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .and_then(|response| response.media_types.first())
        .is_some_and(|media| media.content_type.eq_ignore_ascii_case("text/event-stream"))
}

pub(super) fn operation_is_binary_response(operation: &Operation) -> bool {
    matches!(
        analyze_operation(operation, None).streaming,
        Some(StreamingKind::Binary)
    )
}

/// Returns an object model name when the selected successful response is a
/// component object. Those models can be reconstructed from response JSON at
/// runtime instead of merely being a static `cast`.
pub(super) fn response_object_model(api: &Api, operation: &Operation) -> Option<String> {
    let candidate = operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .and_then(|response| response.media_types.first())
        .and_then(|media| media.schema.as_ref())
        .and_then(|schema| match &schema.kind {
            SchemaKind::Reference { reference } => {
                Some(reference.rsplit('/').next().unwrap_or(reference).to_owned())
            }
            _ => None,
        })?;
    api.schemas
        .iter()
        .find(|schema| schema.name == candidate)
        .and_then(|schema| {
            matches!(schema.value.kind, SchemaKind::Object { .. }).then_some(candidate)
        })
        .map(|name| python_type_name(&name))
}

pub(super) fn python_literal(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "None".into(),
        serde_json::Value::Bool(value) => {
            if *value {
                "True".into()
            } else {
                "False".into()
            }
        }
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => format!("{value:?}"),
        serde_json::Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(python_literal)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        serde_json::Value::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(key, value)| format!("{key:?}: {}", python_literal(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

pub(super) fn python_type(value: &SchemaValue) -> String {
    let base = match &value.kind {
        SchemaKind::Any | SchemaKind::Not { .. } => "Any".into(),
        SchemaKind::Null => "None".into(),
        SchemaKind::Boolean => "bool".into(),
        SchemaKind::Integer => "int".into(),
        SchemaKind::Number => "float".into(),
        SchemaKind::String => "str".into(),
        SchemaKind::Array { items } => format!("list[{}]", python_type(items)),
        SchemaKind::Object {
            additional_properties: AdditionalProperties::Schema { value },
            ..
        } => format!("dict[str, {}]", python_type(value)),
        SchemaKind::Object { .. } => "dict[str, Any]".into(),
        SchemaKind::Reference { reference } => {
            python_type_name(reference.rsplit('/').next().unwrap_or(reference))
        }
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => variants
            .iter()
            .map(python_type)
            .collect::<Vec<_>>()
            .join(" | "),
        SchemaKind::AllOf { .. } => "dict[str, Any]".into(),
    };
    if value.nullable && base != "None" {
        format!("{base} | None")
    } else {
        base
    }
}

pub(super) fn python_type_name(name: &str) -> String {
    pascal_case(name)
}

pub(super) fn python_module_name(name: &str) -> String {
    let value = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let value = value.trim_matches('_');
    let value = if value.is_empty() {
        "generated_sdk"
    } else {
        value
    };
    if value
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        format!("sdk_{value}")
    } else {
        value.into()
    }
}

/// A stable, bounded filename for component/tag names.  OpenAPI component
/// names and Graph-style tags can be arbitrarily long; preserving the full
/// name in a path is both fragile on macOS and unnecessary for imports.
pub(super) fn schema_file_name(name: &str) -> String {
    let stem = python_module_name(name);
    let prefix = &stem[..stem.len().min(96)];
    let hash = name
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("{prefix}_{hash:016x}")
}

pub(super) fn python_field_identifier(
    fields: &[poolster_core::Field],
    field: &poolster_core::Field,
) -> String {
    let mut used =
        std::collections::BTreeSet::from(["from_dict", "classmethod"].map(str::to_owned));
    for item in fields {
        let mut name = python_identifier(&item.name);
        while !used.insert(name.clone()) {
            name.push('_');
        }
        if std::ptr::eq(item, field) {
            return name;
        }
    }
    python_identifier(&field.name)
}

pub(super) fn python_parameter_identifier(
    operation: &Operation,
    parameter: &poolster_core::OperationParameter,
) -> String {
    let mut used = std::collections::BTreeSet::from(["self".to_owned()]);
    if operation.request_body.is_some() {
        used.insert("body".to_owned());
    }
    for item in &operation.parameters {
        let mut name = python_identifier(&item.name);
        while !used.insert(name.clone()) {
            name.push('_');
        }
        if std::ptr::eq(item, parameter) {
            return name;
        }
    }
    python_identifier(&parameter.name)
}

pub(super) fn python_pagination_argument(
    operation: &Operation,
    input: &poolster_core::pagination::PaginationInput,
) -> String {
    operation
        .parameters
        .iter()
        .find(|parameter| parameter.name == input.name && parameter.location == input.location)
        .map(|parameter| python_parameter_identifier(operation, parameter))
        .unwrap_or_else(|| python_identifier(&input.name))
}
