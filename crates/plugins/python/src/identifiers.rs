pub(super) fn python_identifier(value: &str) -> String {
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
        "False"
            | "None"
            | "True"
            | "and"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "break"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "finally"
            | "for"
            | "from"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "try"
            | "while"
            | "with"
            | "yield"
    ) {
        value.push('_');
    }
    value
}

pub(super) fn snake_case(value: &str) -> String {
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

pub(super) fn pascal_case(value: &str) -> String {
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
        "GeneratedValue".into()
    } else if output
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        format!("Value{output}")
    } else {
        output
    }
}

pub(super) fn kebab_case(value: &str) -> String {
    snake_case(value).replace('_', "-")
}

pub(super) fn python_package_version(version: &str) -> String {
    let values = version.split('-').collect::<Vec<_>>();
    if values.len() == 3 && values.iter().all(|value| value.parse::<u64>().is_ok()) {
        return values.join(".");
    }
    if version.split('.').count() == 3
        && version.split('.').all(|value| value.parse::<u64>().is_ok())
    {
        return version.into();
    }
    "0.1.0".into()
}
