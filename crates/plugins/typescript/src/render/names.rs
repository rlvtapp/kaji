use super::*;

pub(crate) fn package_slug(name: &str) -> String {
    let slug = name
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    match slug.trim_matches('-') {
        "" => "api".to_owned(),
        value => value.to_owned(),
    }
}

pub(crate) fn property_name(name: &str) -> String {
    if is_identifier(name) && !reserved(name) {
        name.to_owned()
    } else {
        js_string(name)
    }
}

pub(crate) fn type_identifier(name: &str) -> String {
    let result = identifier(name, true);
    if reserved(&result) {
        format!("{result}Type")
    } else {
        result
    }
}

pub(crate) fn operation_identifier(name: &str, config: &ArtifactOptions) -> String {
    let mut result = match config.naming {
        Naming::PascalCase => identifier(name, true),
        Naming::SnakeCase => snake_case(name),
        Naming::CamelCase => crate::symbols::camel(name),
    };
    if reserved(&result) {
        result.push_str("Operation");
    }
    result
}

pub(crate) fn identifier(value: &str, upper_first: bool) -> String {
    let mut result = String::new();
    let mut uppercase_next = upper_first;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            if result.is_empty() && character.is_ascii_digit() {
                result.push('_');
            }
            if uppercase_next {
                result.extend(character.to_uppercase());
                uppercase_next = false;
            } else {
                result.push(character);
            }
        } else {
            uppercase_next = true;
        }
    }
    if result.is_empty() {
        "unnamed".to_owned()
    } else {
        result
    }
}

pub(crate) fn snake_case(value: &str) -> String {
    let mut result = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            result.push('_');
        }
        if character.is_ascii_alphanumeric() || character == '_' {
            result.push(character.to_ascii_lowercase());
        } else if !result.ends_with('_') {
            result.push('_');
        }
    }
    match result.trim_matches('_') {
        "" => "unnamed".to_owned(),
        value => value.to_owned(),
    }
}

pub(crate) fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(character) if character.is_ascii_alphabetic() || character == '_' || character == '$')
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '$'
        })
}

pub(crate) fn reserved(value: &str) -> bool {
    matches!(
        value,
        "as" | "async"
            | "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "from"
            | "function"
            | "get"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "let"
            | "new"
            | "null"
            | "of"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "set"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "type"
            | "typeof"
            | "undefined"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

pub(crate) fn js_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization is infallible")
}
