//! Names emission for the Swift HTTP SDK.
use crate::*;

pub(crate) fn type_name(value: &str) -> String {
    let result = value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect::<String>();
    let result = if result.is_empty() {
        "PoolsterAPI".to_owned()
    } else {
        result
    };
    if result
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        format!("Poolster{result}")
    } else {
        result
    }
}

pub(crate) fn kebab_case(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if value.is_empty() {
        "poolster".to_owned()
    } else {
        value
    }
}

pub(crate) fn function_name(value: &str) -> String {
    identifier(&lower_camel(value))
}

pub(crate) fn model_file_name(name: &str) -> String {
    let name = type_name(name);
    if name == "Operations"
        || name.starts_with("Poolster")
        || name.ends_with("Resource")
        || matches!(name.as_str(), "Streaming" | "Pagination" | "JSONValue")
    {
        format!("Model_{name}")
    } else {
        name
    }
}

pub(crate) fn identifier(value: &str) -> String {
    let mut value = lower_camel(value);
    if value.is_empty() {
        value = "value".to_owned();
    }
    if value
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        value.insert(0, '_');
    }
    if matches!(
        value.as_str(),
        "as" | "associatedtype"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "continue"
            | "default"
            | "defer"
            | "deinit"
            | "do"
            | "else"
            | "enum"
            | "extension"
            | "fallthrough"
            | "false"
            | "fileprivate"
            | "for"
            | "func"
            | "guard"
            | "if"
            | "import"
            | "in"
            | "init"
            | "inout"
            | "internal"
            | "is"
            | "let"
            | "nil"
            | "open"
            | "operator"
            | "private"
            | "protocol"
            | "public"
            | "repeat"
            | "return"
            | "self"
            | "static"
            | "struct"
            | "subscript"
            | "super"
            | "switch"
            | "throw"
            | "throws"
            | "true"
            | "try"
            | "typealias"
            | "var"
            | "where"
            | "while"
    ) {
        format!("`{value}`")
    } else {
        value
    }
}

pub(crate) fn enum_case(value: &str, index: usize) -> String {
    let name = identifier(value).trim_matches('`').to_owned();
    if name.is_empty() || name == "value" {
        format!("value{index}")
    } else {
        name
    }
}

/// Escapes only characters which terminate a Swift string. In particular, it
/// preserves `\\(expression)` interpolation inserted for OpenAPI path values.
pub(crate) fn swift_path_literal(value: &str) -> String {
    value
        .replace('"', "\\\"")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

pub(crate) fn parameter_name(parameter: &OperationParameter) -> String {
    identifier(
        parameter
            .annotations
            .get("poolster.native_argument")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&parameter.name),
    )
}
