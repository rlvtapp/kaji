use super::*;

pub(super) fn resource_operations(api: &Api) -> BTreeMap<String, Vec<(&Operation, String)>> {
    let mut groups = BTreeMap::<String, Vec<(&Operation, String)>>::new();
    for operation in &api.operations {
        let mut resource = resource_name(operation);
        if let Some(existing) = groups
            .keys()
            .find(|name| name.eq_ignore_ascii_case(&resource))
        {
            resource = existing.clone();
        }
        let candidate = resource_method_name(operation, &resource);
        let operations = groups.entry(resource).or_default();
        let used = operations
            .iter()
            .map(|(_, method)| method.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        let method = if used.contains(&candidate.to_ascii_lowercase()) {
            method_name(&operation.id)
        } else {
            candidate
        };
        let mut method = method;
        while used.contains(&method.to_ascii_lowercase()) {
            method.push('_');
        }
        operations.push((operation, method));
    }
    groups
}

/// Import only models named in a file's public signatures or decode paths.
/// Importing every component into every generated trait turns a large API's
/// otherwise bounded files into megabyte-sized `use` lists.
pub(super) fn operation_model_imports(
    operations: &[Operation],
    named_types: &NamedTypes,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for operation in operations {
        collect_operation_model_imports(operation, named_types, &mut names);
    }
    names
}

pub(super) fn resource_model_imports(
    operations: &[(&Operation, String)],
    named_types: &NamedTypes,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for (operation, _) in operations {
        collect_operation_model_imports(operation, named_types, &mut names);
    }
    names
}

pub(super) fn collect_operation_model_imports(
    operation: &Operation,
    named_types: &NamedTypes,
    names: &mut BTreeSet<String>,
) {
    for parameter in &operation.parameters {
        if let Some(schema) = &parameter.schema {
            collect_schema_model_imports(schema, named_types, names);
        }
    }
    if let Some(body) = &operation.request_body {
        for media in &body.media_types {
            if let Some(schema) = &media.schema {
                collect_schema_model_imports(schema, named_types, names);
            }
        }
    }
    for response in &operation.responses {
        for media in &response.media_types {
            if let Some(schema) = &media.schema {
                collect_schema_model_imports(schema, named_types, names);
            }
        }
    }
}

pub(super) fn collect_schema_model_imports(
    schema: &SchemaValue,
    named_types: &NamedTypes,
    names: &mut BTreeSet<String>,
) {
    match &schema.kind {
        SchemaKind::Reference { reference } => {
            let name = type_name(reference.rsplit('/').next().unwrap_or(reference));
            if named_types.is_class(&name) {
                names.insert(name);
            }
        }
        SchemaKind::Array { items } | SchemaKind::Not { schema: items } => {
            collect_schema_model_imports(items, named_types, names);
        }
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                collect_schema_model_imports(&field.value, named_types, names);
            }
            if let AdditionalProperties::Schema { value } = additional_properties {
                collect_schema_model_imports(value, named_types, names);
            }
        }
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            for variant in variants {
                collect_schema_model_imports(variant, named_types, names);
            }
        }
        SchemaKind::Any
        | SchemaKind::Null
        | SchemaKind::Boolean
        | SchemaKind::Integer
        | SchemaKind::Number
        | SchemaKind::String => {}
    }
}

pub(super) fn resource_name(operation: &Operation) -> String {
    operation
        .annotations
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(serde_json::Value::as_str)
        .map(type_name)
        .filter(|tag| !tag.is_empty())
        .unwrap_or_else(|| {
            operation
                .path
                .split('/')
                .filter(|segment| !segment.is_empty())
                .find(|segment| {
                    !segment.starts_with('{')
                        && !is_version_segment(segment)
                        && *segment != "email"
                        && *segment != "api"
                })
                .map(type_name)
                .filter(|segment| !segment.is_empty())
                .unwrap_or_else(|| "Api".into())
        })
}

pub(super) fn is_version_segment(segment: &str) -> bool {
    segment.strip_prefix('v').is_some_and(|number| {
        !number.is_empty() && number.chars().all(|character| character.is_ascii_digit())
    })
}

pub(super) fn resource_method_name(operation: &Operation, resource: &str) -> String {
    let operation_name = method_name(&operation.id);
    let singular = resource.strip_suffix('s').unwrap_or(resource);
    for suffix in [resource, singular] {
        if let Some(method) = operation_name.strip_suffix(suffix) {
            if !method.is_empty() {
                return method_name(method);
            }
        }
    }
    operation_name
}

/// Resource accessors share the `Client` class with direct operations. Keep
/// the pleasant `contacts()` name unless it would duplicate an operation such
/// as an `authCheck` health probe, in which case `authCheckResource()` is
/// unambiguous and preserves both exports.
pub(super) fn resource_accessor_name(api: &Api, resource: &str) -> String {
    let candidate = property_name(resource);
    if symbols::OPERATION_RESERVED
        .iter()
        .any(|name| name.eq_ignore_ascii_case(&candidate))
        || api
            .operations
            .iter()
            .any(|operation| method_name(&operation.id).eq_ignore_ascii_case(&candidate))
    {
        format!("{candidate}Resource")
    } else {
        candidate
    }
}
