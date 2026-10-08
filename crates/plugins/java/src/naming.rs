use super::*;

pub(super) fn java_package_name(value: &str) -> String {
    let parts = value
        .split('.')
        .map(package_segment)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    parts.join(".")
}

pub(super) fn package_segment(value: &str) -> String {
    let mut output = String::new();
    let mut previous_dash = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }
    let output = output.trim_matches('-').replace('-', "");
    let output = if output.is_empty() {
        "sdk".to_owned()
    } else {
        output
    };
    if output
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        format!("sdk{output}")
    } else if java_keywords().contains(&output.as_str()) {
        format!("{output}sdk")
    } else {
        output
    }
}

pub(super) fn type_name(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase = true;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if output.is_empty() && character.is_ascii_digit() {
                output.push_str("Value");
            }
            output.push(if uppercase {
                character.to_ascii_uppercase()
            } else {
                character
            });
            uppercase = false;
        } else {
            uppercase = true;
        }
    }
    let output = if output.is_empty() {
        "Value".to_owned()
    } else {
        output
    };
    if java_keywords().contains(&output.to_ascii_lowercase().as_str()) {
        format!("{output}Value")
    } else {
        output
    }
}

pub(super) fn method_name(value: &str) -> String {
    let pascal = type_name(value);
    let mut chars = pascal.chars();
    let Some(first) = chars.next() else {
        return "call".into();
    };
    let name = format!("{}{}", first.to_ascii_lowercase(), chars.as_str());
    if java_keywords().contains(&name.as_str()) {
        format!("{name}Operation")
    } else {
        name
    }
}

pub(super) fn field_name(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if output.is_empty() && character.is_ascii_digit() {
                output.push_str("value");
            }
            output.push(if uppercase {
                character.to_ascii_uppercase()
            } else {
                character
            });
            uppercase = false;
        } else {
            uppercase = true;
        }
    }
    let output = if output.is_empty() {
        "value".to_owned()
    } else {
        output
    };
    let mut characters = output.chars();
    let first = characters
        .next()
        .map(|character| character.to_ascii_lowercase())
        .unwrap_or('v');
    let output = format!("{first}{}", characters.as_str());
    if java_keywords().contains(&output.as_str()) {
        format!("{output}Value")
    } else {
        output
    }
}

pub(super) fn enum_name(value: &str, index: usize) -> String {
    let value = type_name(value).to_ascii_uppercase();
    if value == "VALUE" {
        format!("VALUE_{index}")
    } else {
        value
    }
}

pub(super) fn package_version(value: &str) -> String {
    let value = value.trim();
    if value.split('.').count() == 3 && value.split('.').all(|part| part.parse::<u64>().is_ok()) {
        value.to_owned()
    } else if value.len() == 10
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
    {
        value.replace('-', ".")
    } else {
        "0.1.0".into()
    }
}

pub(super) fn java_keywords() -> &'static [&'static str] {
    &[
        "null",
        "true",
        "false",
        "abstract",
        "assert",
        "boolean",
        "break",
        "byte",
        "case",
        "catch",
        "char",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "double",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "float",
        "for",
        "goto",
        "if",
        "implements",
        "import",
        "instanceof",
        "int",
        "interface",
        "long",
        "native",
        "new",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "short",
        "static",
        "strictfp",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "void",
        "volatile",
        "while",
        "record",
        "sealed",
        "permits",
        "var",
        "yield",
    ]
}

pub(super) fn prepare_api(api: &Api) -> Api {
    native_names::prepare(
        api,
        type_name,
        method_name,
        &[
            "Object",
            "Module",
            "Package",
            "Record",
            "ClassLoader",
            "ClassValue",
            "Stack",
            "Currency",
            "Date",
            "Locale",
            "UUID",
            "Calendar",
            "Random",
            "Timer",
            "TimeZone",
            "String",
            "Boolean",
            "Long",
            "Double",
            "Integer",
            "Short",
            "Byte",
            "Float",
            "Void",
            "Character",
            "Number",
            "Exception",
            "RuntimeException",
            "Error",
            "Thread",
            "System",
            "Math",
            "Class",
            "Override",
            "Iterable",
            "Enum",
            "Collections",
            "Collection",
            "List",
            "Map",
            "Set",
            "Queue",
            "Dictionary",
            "Optional",
            "Objects",
            "ArrayList",
            "JsonNode",
            "JsonProperty",
            "JsonCreator",
            "JsonValue",
            "JsonInclude",
            "Client",
            "ClientBase",
            "ClientConfig",
            "ClientHooks",
            "RetryConfig",
            "ApiException",
            "Presence",
            "MultipartBody",
            "OrderedMultipart",
        ],
    )
}

pub(super) fn parameter_name(parameter: &OperationParameter) -> String {
    field_name(
        parameter
            .annotations
            .get("poolster.native_argument")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&parameter.name),
    )
}
