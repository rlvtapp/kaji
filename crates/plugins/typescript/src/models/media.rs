use super::*;

pub(crate) fn media_aliases(
    media: &[poolster_core::OperationMediaType],
) -> BTreeMap<String, String> {
    let content_types = media
        .iter()
        .map(|m| m.content_type.clone())
        .collect::<BTreeSet<_>>();
    let mut used = BTreeSet::new();
    content_types
        .into_iter()
        .map(|content_type| {
            let base = media_type_identifier(&content_type);
            let mut name = base.clone();
            let mut index = 2;
            while !used.insert(name.to_ascii_lowercase()) {
                name = format!("{base}{index}");
                index += 1;
            }
            (content_type, name)
        })
        .collect()
}

pub(crate) fn media_type_identifier(content_type: &str) -> String {
    match content_type {
        "application/json" => return "Json".into(),
        "application/xml" => return "Xml".into(),
        "text/xml" => return "TextXml".into(),
        "*/*" => return "AnyMedia".into(),
        "" => return "UnspecifiedMedia".into(),
        "multipart/form-data" => return "FormData".into(),
        _ => {}
    }
    content_type
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(type_identifier)
        .collect::<String>()
}
