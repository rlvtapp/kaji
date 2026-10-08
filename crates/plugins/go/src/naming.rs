use super::*;

pub(super) fn go_package_name(value: &str) -> String {
    let mut package = identifier_words(value)
        .into_iter()
        .map(|word| word.to_ascii_lowercase())
        .collect::<String>();
    if package.is_empty() {
        return package;
    }
    if package.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        package.insert(0, 'v');
    }
    match package.as_str() {
        "type" | "var" | "func" | "package" | "map" | "chan" | "interface" | "struct" | "go"
        | "defer" | "select" | "range" => format!("{package}sdk"),
        _ => package,
    }
}

pub(super) fn go_module_name(value: &str) -> String {
    let slug = identifier_words(value)
        .into_iter()
        .map(|word| word.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "poolster/sdk".into()
    } else {
        slug
    }
}

pub(super) fn go_type_name(value: &str) -> String {
    let mut name = identifier_words(value)
        .into_iter()
        .map(|word| go_exported_word(&word))
        .collect::<String>();
    if name.is_empty() {
        name = "Value".into();
    }
    if name.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        name.insert(0, 'V');
    }
    name
}

pub(super) fn go_exported_word(word: &str) -> String {
    match word.to_ascii_lowercase().as_str() {
        "api" => "API".into(),
        "id" => "ID".into(),
        "http" => "HTTP".into(),
        "https" => "HTTPS".into(),
        "json" => "JSON".into(),
        "oauth" => "OAuth".into(),
        "url" => "URL".into(),
        "uri" => "URI".into(),
        "uuid" => "UUID".into(),
        _ => {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        }
    }
}

pub(super) fn identifier_words(value: &str) -> Vec<String> {
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
