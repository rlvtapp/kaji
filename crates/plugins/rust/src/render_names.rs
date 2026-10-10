//! HTTP names rendering.
use super::*;

pub(crate) fn request_body_type(operation: &Operation) -> Option<String> {
    operation.request_body.as_ref().map(|_| {
        if matches!(request_media_kind(operation), RequestMediaKind::Multipart) {
            return "MultipartBody".into();
        }
        operation
            .request_schema()
            .map(rust_type)
            .unwrap_or_else(|| "serde_json::Value".into())
    })
}

pub(crate) fn type_name(name: &str) -> String {
    let mut output = String::new();
    let mut uppercase = true;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if uppercase {
                output.extend(character.to_uppercase());
            } else {
                output.push(character);
            }
            uppercase = false;
        } else {
            uppercase = true;
        }
    }
    if output.is_empty() {
        "Value".into()
    } else if output.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        format!("Value{output}")
    } else {
        output
    }
}

pub(crate) fn rust_field_name(name: &str) -> String {
    let mut output = String::new();
    let mut previous_is_lower = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase() && previous_is_lower {
                output.push('_');
            }
            output.extend(character.to_lowercase());
            previous_is_lower = character.is_ascii_lowercase() || character.is_ascii_digit();
        } else if !output.ends_with('_') {
            output.push('_');
            previous_is_lower = false;
        }
    }
    let mut output = output.trim_matches('_').to_owned();
    if output.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        output = format!("value_{output}");
    }
    match output.as_str() {
        "self" | "super" | "crate" => format!("{output}_value"),
        "as" | "async" | "await" | "break" | "const" | "continue" | "dyn" | "else" | "enum"
        | "extern" | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match"
        | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "static" | "struct" | "trait"
        | "true" | "type" | "unsafe" | "use" | "where" | "while" | "abstract" | "become"
        | "box" | "do" | "final" | "gen" | "macro" | "override" | "priv" | "try" | "typeof"
        | "unsized" | "virtual" | "yield" => format!("r#{output}"),
        "" => "value".into(),
        _ => output,
    }
}

pub(crate) fn kebab_case(name: &str) -> String {
    rust_field_name(name)
        .trim_start_matches("r#")
        .replace('_', "-")
}

/// Collision-safe identifiers for extensible enum constructors.
pub(crate) fn enum_helper_name(raw: &str) -> String {
    let name = rust_field_name(raw);
    if matches!(
        name.as_str(),
        "as_str" | "is_known" | "from" | "fmt" | "clone" | "default"
    ) {
        format!("{name}_value")
    } else if name.is_empty() {
        "empty_value".into()
    } else {
        name
    }
}

pub(crate) fn parameter_name(parameter: &OperationParameter) -> String {
    rust_field_name(
        parameter
            .annotations
            .get("poolster.native_argument")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&parameter.name),
    )
}

pub(crate) fn prepare_api(api: &Api) -> Api {
    crate::native_names::prepare(
        api,
        type_name,
        rust_field_name,
        &[
            "String",
            "Vec",
            "Option",
            "Result",
            "Box",
            "Value",
            "Client",
            "ApiResponse",
            "RetryConfig",
            "TokenProviderError",
            "Transport",
            "TransportFuture",
            "MultipartBody",
            "MultipartBodyError",
            "CallOptions",
            "RequestInfo",
            "Arc",
        ],
    )
}
