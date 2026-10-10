pub(crate) fn pascal_identifier(value: &str) -> String {
    crate::symbols::identifier(value)
}

pub(crate) fn lower_camel_identifier(value: &str) -> String {
    crate::symbols::camel(value)
}

pub(crate) fn package_slug(value: &str) -> String {
    let slug = value
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
        "" => "api".into(),
        value => value.into(),
    }
}
