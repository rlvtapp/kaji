//! Names emission for the csharp HTTP SDK.
use crate::*;

pub(crate) fn operation_resource_name(operation: &Operation) -> String {
    let tag = operation
        .annotations
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            operation
                .annotations
                .get("tag")
                .and_then(serde_json::Value::as_str)
        });
    tag.map(resource_name)
        .unwrap_or_else(|| resource_name(&path_resource_segment(&operation.path)))
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

pub(crate) fn resource_name(value: &str) -> String {
    let name = pascal_case(value);
    if name == "GeneratedValue" {
        "Default".into()
    } else if matches!(
        name.as_str(),
        "System" | "Task" | "Math" | "JsonSerializer" | "Guid" | "Uri"
    ) {
        format!("{name}Api")
    } else {
        name
    }
}

pub(crate) fn pascal_case(value: &str) -> String {
    let words = identifier_words(value);
    let mut output = words
        .into_iter()
        .map(|word| {
            let mut characters = word.chars();
            characters
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + characters.as_str())
                .unwrap_or_default()
        })
        .collect::<String>();
    if output.is_empty() {
        output = "GeneratedValue".into();
    }
    if output.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        output.insert(0, 'V');
    }
    if csharp_keyword(&output) {
        output.push_str("Value");
    }
    output
}

pub(crate) fn camel_case(value: &str) -> String {
    let mut output = pascal_case(value);
    if let Some(first) = output.get_mut(0..1) {
        first.make_ascii_lowercase();
    }
    if csharp_keyword(&output) {
        format!("@{output}")
    } else {
        output
    }
}

pub(crate) fn kebab_case(value: &str) -> String {
    identifier_words(value)
        .into_iter()
        .map(|word| word.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

pub(crate) fn dotnet_namespace(package: &str) -> String {
    let value = pascal_case(package);
    format!("Poolster.{value}")
}

pub(crate) fn identifier_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase()
                && !current.is_empty()
                && current
                    .chars()
                    .last()
                    .is_some_and(|last| last.is_ascii_lowercase())
            {
                words.push(std::mem::take(&mut current));
            }
            current.push(character);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

pub(crate) fn enum_member_name(value: &str, index: usize) -> String {
    let name = pascal_case(value);
    if name == "GeneratedValue" {
        format!("Value{index}")
    } else {
        name
    }
}

pub(crate) fn csharp_keyword(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "abstract"
            | "as"
            | "base"
            | "bool"
            | "break"
            | "byte"
            | "case"
            | "catch"
            | "char"
            | "class"
            | "const"
            | "continue"
            | "decimal"
            | "default"
            | "delegate"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "event"
            | "explicit"
            | "extern"
            | "false"
            | "finally"
            | "fixed"
            | "float"
            | "for"
            | "foreach"
            | "goto"
            | "if"
            | "implicit"
            | "in"
            | "int"
            | "interface"
            | "internal"
            | "is"
            | "lock"
            | "long"
            | "namespace"
            | "new"
            | "null"
            | "object"
            | "operator"
            | "out"
            | "override"
            | "params"
            | "private"
            | "protected"
            | "public"
            | "readonly"
            | "ref"
            | "return"
            | "sbyte"
            | "sealed"
            | "short"
            | "sizeof"
            | "stackalloc"
            | "static"
            | "string"
            | "struct"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "uint"
            | "ulong"
            | "unchecked"
            | "unsafe"
            | "ushort"
            | "using"
            | "virtual"
            | "void"
            | "volatile"
            | "while"
    )
}

pub(crate) fn dotnet_version(version: &str) -> String {
    let segments = version.split('-').collect::<Vec<_>>();
    if segments.len() == 3 && segments.iter().all(|part| part.parse::<u64>().is_ok()) {
        return segments.join(".");
    }
    if version.split('.').count() == 3 && version.split('.').all(|part| part.parse::<u64>().is_ok())
    {
        return version.into();
    }
    "0.1.0".into()
}

pub(crate) fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(crate) fn parameter_name(parameter: &OperationParameter) -> String {
    camel_case(
        parameter
            .annotations
            .get("poolster.native_argument")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&parameter.name),
    )
}
