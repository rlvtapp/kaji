//! Names implementation for generated typescript-cli packages.

pub(super) fn kebab_case(value: &str) -> String {
    let mut output = String::new();
    let mut previous_lower = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase() && previous_lower {
                output.push('-');
            }
            output.push(character.to_ascii_lowercase());
            previous_lower = character.is_ascii_lowercase() || character.is_ascii_digit();
        } else if !output.is_empty() && !output.ends_with('-') {
            output.push('-');
            previous_lower = false;
        }
    }
    output.trim_matches('-').to_owned()
}

pub(super) fn camel_case(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if output.is_empty() {
                output.push(character.to_ascii_lowercase());
            } else if uppercase {
                output.push(character.to_ascii_uppercase());
                uppercase = false;
            } else {
                output.push(character);
            }
        } else {
            uppercase = true;
        }
    }
    output
}

pub(super) fn env_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}
