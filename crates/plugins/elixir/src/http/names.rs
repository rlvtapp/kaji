//! Names emission for the elixir HTTP SDK.
use crate::*;

pub(crate) fn operation_resource_name(operation: &Operation) -> String {
    let tag = operation
        .annotations
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(serde_json::Value::as_str);
    tag.map(pascal_case)
        .unwrap_or_else(|| pascal_case(&path_resource_segment(&operation.path)))
}

pub(crate) fn path_resource_segment(path: &str) -> String {
    path.split('/')
        .filter(|segment| !segment.is_empty() && !segment.starts_with('{'))
        .find(|segment| {
            let lowered = segment.to_ascii_lowercase();
            lowered != "api"
                && lowered != "email"
                && !(lowered.starts_with('v')
                    && lowered.len() > 1
                    && lowered[1..]
                        .chars()
                        .all(|character| character.is_ascii_digit()))
        })
        .unwrap_or("default")
        .into()
}

pub(crate) fn package_slug(value: &str) -> String {
    snake_case(value).replace('_', "-")
}

/// Keeps generated paths portable on conservative file systems while retaining
/// readable component names. A suffix is only added for a normalized collision
/// or a value that had to be shortened, so ordinary output stays familiar.
pub(crate) fn unique_file_stems<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let values = values.into_iter().collect::<Vec<_>>();
    let mut counts = BTreeMap::<String, usize>::new();
    for value in &values {
        *counts.entry(bounded_file_stem(value)).or_default() += 1;
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let stem = bounded_file_stem(value);
            if counts[&stem] > 1 || snake_case(value).len() > 80 {
                format!("{stem}_{:04}", index + 1)
            } else {
                stem
            }
        })
        .collect()
}

pub(crate) fn bounded_file_stem(value: &str) -> String {
    let stem = snake_case(value);
    if stem.len() <= 80 {
        return stem;
    }
    let mut prefix = stem;
    prefix.truncate(64);
    prefix = prefix.trim_matches('_').to_owned();
    format!("{prefix}_{:08x}", stable_path_hash(value))
}

pub(crate) fn stable_path_hash(value: &str) -> u32 {
    value.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        hash.wrapping_mul(0x0100_0193) ^ u32::from(byte)
    })
}

pub(crate) fn elixir_field_identifier(
    fields: &[poolster_core::Field],
    field: &poolster_core::Field,
) -> String {
    let mut used = std::collections::BTreeSet::new();
    for item in fields {
        let mut name = elixir_identifier(&item.name);
        while !used.insert(name.clone()) {
            name.push('_');
        }
        if std::ptr::eq(item, field) {
            return name;
        }
    }
    elixir_identifier(&field.name)
}

pub(crate) fn elixir_parameter_identifier(
    operation: &Operation,
    parameter: &poolster_core::OperationParameter,
) -> String {
    let mut used = std::collections::BTreeSet::from([
        "client".to_owned(),
        "options".to_owned(),
        "path".to_owned(),
        "query".to_owned(),
        "headers".to_owned(),
        "body".to_owned(),
        "response".to_owned(),
    ]);
    for item in &operation.parameters {
        let mut name = elixir_identifier(&item.name);
        while !used.insert(name.clone()) {
            name.push('_');
        }
        if std::ptr::eq(item, parameter) {
            return name;
        }
    }
    elixir_identifier(&parameter.name)
}

pub(crate) fn elixir_identifier(value: &str) -> String {
    let mut value = snake_case(value);
    if value.is_empty() {
        value = "value".into();
    }
    if value
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        value.insert_str(0, "value_");
    }
    if matches!(
        value.as_str(),
        "after" | "catch" | "do" | "else" | "end" | "rescue" | "when"
    ) {
        value.push('_');
    }
    value
}

pub(crate) fn snake_case(value: &str) -> String {
    let mut output = String::new();
    let mut prior_lowercase = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase() && prior_lowercase && !output.ends_with('_') {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
            prior_lowercase = character.is_ascii_lowercase() || character.is_ascii_digit();
        } else if !output.is_empty() && !output.ends_with('_') {
            output.push('_');
            prior_lowercase = false;
        }
    }
    output.trim_matches('_').into()
}

pub(crate) fn pascal_case(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase = true;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if uppercase {
                output.push(character.to_ascii_uppercase());
                uppercase = false;
            } else {
                output.push(character);
            }
        } else {
            uppercase = true;
        }
    }
    if output.is_empty() {
        "GeneratedSdk".into()
    } else if output
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        format!("Sdk{output}")
    } else {
        output
    }
}

pub(crate) fn package_version(version: &str) -> String {
    let parts = version.split('-').collect::<Vec<_>>();
    if parts.len() == 3 && parts.iter().all(|part| part.parse::<u64>().is_ok()) {
        return parts
            .iter()
            .filter_map(|part| part.parse::<u64>().ok())
            .map(|part| part.to_string())
            .collect::<Vec<_>>()
            .join(".");
    }
    if version.split('.').count() == 3 && version.split('.').all(|part| part.parse::<u64>().is_ok())
    {
        return version.into();
    }
    "0.1.0".into()
}

pub(crate) fn escape_elixir_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
