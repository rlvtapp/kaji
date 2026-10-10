//! Ruby identifier, literal and wire-field naming shared by protocol backends.
use crate::*;

pub(crate) fn ruby_version(version: &str) -> String {
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() == 3 && parts.iter().all(|part| part.parse::<u32>().is_ok()) {
        version.into()
    } else {
        "0.1.0".into()
    }
}

pub(crate) fn ruby_string(value: &str) -> String {
    serde_json::to_string(value).expect("strings serialize")
}

pub(crate) fn ruby_file_name(value: &str) -> String {
    snake_case(value)
}

pub(crate) fn ruby_field_identifier(
    fields: &[poolster_core::Field],
    field: &poolster_core::Field,
) -> String {
    let mut used = std::collections::BTreeSet::from(
        ["to_h", "from_hash", "initialize", "with_present_fields"].map(str::to_owned),
    );
    for item in fields {
        let mut name = ruby_identifier(&item.name);
        while !used.insert(name.clone()) {
            name.push('_');
        }
        if std::ptr::eq(item, field) {
            return name;
        }
    }
    ruby_identifier(&field.name)
}

pub(crate) fn ruby_parameter_identifier(
    operation: &Operation,
    parameter: &poolster_core::OperationParameter,
) -> String {
    let mut used = std::collections::BTreeSet::from(["self".to_owned()]);
    if operation.request_body.is_some() {
        used.insert("body".to_owned());
    }
    for item in &operation.parameters {
        let mut name = ruby_identifier(&item.name);
        while !used.insert(name.clone()) {
            name.push('_');
        }
        if std::ptr::eq(item, parameter) {
            return name;
        }
    }
    ruby_identifier(&parameter.name)
}

pub(crate) fn ruby_pagination_argument(
    operation: &Operation,
    input: &poolster_core::pagination::PaginationInput,
) -> String {
    operation
        .parameters
        .iter()
        .find(|parameter| parameter.name == input.name && parameter.location == input.location)
        .map(|parameter| ruby_parameter_identifier(operation, parameter))
        .unwrap_or_else(|| ruby_identifier(&input.name))
}

pub(crate) fn ruby_identifier(value: &str) -> String {
    let value = snake_case(value);
    let value = if value.is_empty() {
        "value".into()
    } else {
        value
    };
    if value.starts_with(|c: char| c.is_ascii_digit()) {
        return format!("value_{value}");
    }
    match value.as_str() {
        "alias" | "and" | "begin" | "break" | "case" | "class" | "def" | "defined" | "do"
        | "else" | "elsif" | "end" | "ensure" | "false" | "for" | "if" | "in" | "module"
        | "next" | "nil" | "not" | "or" | "redo" | "rescue" | "retry" | "return" | "self"
        | "super" | "then" | "true" | "undef" | "unless" | "until" | "when" | "while" | "yield"
        | "private" | "public" => format!("{value}_"),
        _ => value,
    }
}

pub(crate) fn snake_case(value: &str) -> String {
    let mut output = String::new();
    let mut previous_is_lower = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase() && previous_is_lower {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
            previous_is_lower = character.is_ascii_lowercase() || character.is_ascii_digit();
        } else if !output.ends_with('_') && !output.is_empty() {
            output.push('_');
            previous_is_lower = false;
        }
    }
    output.trim_matches('_').into()
}

pub(crate) fn kebab_case(value: &str) -> String {
    snake_case(value).replace('_', "-")
}

pub(crate) fn pascal_case(value: &str) -> String {
    let mut output = String::new();
    let mut capitalize = true;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if capitalize {
                output.push(character.to_ascii_uppercase());
                capitalize = false;
            } else {
                output.push(character);
            }
        } else {
            capitalize = true;
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
