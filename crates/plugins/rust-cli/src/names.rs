//! Names implementation for generated rust-cli packages.

pub(super) fn literal(value: &str) -> String {
    serde_json::to_string(value).expect("string")
}
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
