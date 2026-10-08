use super::*;

pub(super) fn response_schema(operation: &Operation) -> Option<&SchemaValue> {
    operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .or_else(|| {
            operation
                .responses
                .iter()
                .find(|response| response.status == "default")
        })
        .and_then(|response| response.media_types.first())
        .and_then(|media| media.schema.as_ref())
}

pub(super) fn php_type(value: &SchemaValue, named_types: &NamedTypes) -> String {
    let base = match &value.kind {
        SchemaKind::Any | SchemaKind::Not { .. } => "mixed".into(),
        SchemaKind::Null => "null".into(),
        SchemaKind::Boolean => "bool".into(),
        SchemaKind::Integer => "int".into(),
        SchemaKind::Number => "float".into(),
        SchemaKind::String => "string".into(),
        SchemaKind::Array { .. } => "array".into(),
        SchemaKind::Object { .. } => "array".into(),
        SchemaKind::Reference { reference } => {
            let name = type_name(reference.rsplit('/').next().unwrap_or(reference));
            if let Some(wire) = named_types.enum_wire_types.get(&name) {
                format!("{name}|{wire}")
            } else if named_types.is_class(&name) {
                name
            } else {
                "mixed".into()
            }
        }
        SchemaKind::OneOf { .. } | SchemaKind::AnyOf { .. } | SchemaKind::AllOf { .. } => {
            "mixed".into()
        }
    };
    nullable_type(&base, value.nullable || value.optional || value.nullish)
}

pub(super) fn nullable_type(type_name: &str, nullable: bool) -> String {
    if nullable
        && type_name != "mixed"
        && type_name != "null"
        && !type_name.starts_with('?')
        && !type_name.split('|').any(|part| part == "null")
    {
        if type_name.contains('|') {
            format!("{type_name}|null")
        } else {
            format!("?{type_name}")
        }
    } else {
        type_name.into()
    }
}

pub(super) fn from_value(value: &str, schema: &SchemaValue, named_types: &NamedTypes) -> String {
    let decoded = match &schema.kind {
        SchemaKind::Reference { reference } => {
            let name = type_name(reference.rsplit('/').next().unwrap_or(reference));
            if named_types.models.contains(&name) {
                format!("{name}::fromArray({value})")
            } else if named_types.enums.contains(&name) {
                format!("({name}::tryFrom({value}) ?? {value})")
            } else if named_types.wrappers.contains(&name) {
                format!("{name}::from({value})")
            } else {
                value.into()
            }
        }
        SchemaKind::Array { items } => format!(
            "array_map(static fn (mixed $item): mixed => {}, {value})",
            from_value("$item", items, named_types)
        ),
        SchemaKind::Integer => format!("(int) {value}"),
        SchemaKind::Number => format!("(float) {value}"),
        SchemaKind::String => format!("(string) {value}"),
        SchemaKind::Boolean => format!("(bool) {value}"),
        _ => value.into(),
    };
    if schema.nullable && !matches!(schema.kind, SchemaKind::Null | SchemaKind::Any) {
        format!("({value} === null ? null : {decoded})")
    } else {
        decoded
    }
}

pub(super) fn is_backed_enum(value: &SchemaValue) -> bool {
    !value.enum_values.is_empty()
        && value
            .enum_values
            .iter()
            .all(|value| value.is_string() || value.as_i64().is_some())
        && (value.enum_values.iter().all(|value| value.is_string())
            || value
                .enum_values
                .iter()
                .all(|value| value.as_i64().is_some()))
}

pub(super) fn enum_case(value: &serde_json::Value, index: usize) -> String {
    let candidate = value
        .as_str()
        .map(type_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("Value{index}"));
    if candidate.eq_ignore_ascii_case("class") {
        return format!("{candidate}Value");
    }
    if candidate
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        format!("Value{candidate}")
    } else {
        candidate
    }
}

pub(super) fn method_name(name: &str) -> String {
    let name = property_name(name);
    match name.as_str() {
        "case" | "switch" | "return" | "if" | "else" | "elseif" | "for" | "foreach" | "while"
        | "do" | "break" | "continue" | "goto" | "try" | "catch" | "finally" | "throw"
        | "declare" | "instanceof" | "insteadof" | "abstract" | "final" | "public"
        | "protected" | "private" | "const" | "extends" | "implements" | "interface" | "enum"
        | "readonly" | "fn" | "as" | "and" | "or" | "xor" | "list" | "clone" | "match" | "new"
        | "self" | "parent" | "function" | "namespace" | "trait" | "class" | "use" | "static"
        | "default" | "global" | "empty" | "isset" | "unset" | "echo" | "print" | "include"
        | "require" | "eval" | "exit" | "die" | "yield" => format!("{name}Operation"),
        _ => name,
    }
}

pub(super) fn unique_name(candidate: String, used: &mut BTreeSet<String>) -> String {
    let mut name = candidate;
    while !used.insert(name.clone()) {
        name.push('_');
    }
    name
}

pub(super) fn type_name(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase = true;
    for character in value.chars() {
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
    match output.chars().next() {
        None => "Value".into(),
        Some(character) if character.is_ascii_digit() => format!("Value{output}"),
        _ => output,
    }
}

pub(super) fn php_field_identifier(
    fields: &[poolster_core::Field],
    field: &poolster_core::Field,
) -> String {
    let mut used = BTreeSet::new();
    for item in fields {
        let name = unique_name(property_name(&item.name), &mut used);
        if std::ptr::eq(item, field) {
            return name;
        }
    }
    property_name(&field.name)
}

pub(super) fn property_name(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if output.is_empty() {
                output.extend(character.to_lowercase());
            } else if uppercase {
                output.extend(character.to_uppercase());
            } else {
                output.push(character);
            }
            uppercase = false;
        } else {
            uppercase = true;
        }
    }
    let output = match output.as_str() {
        "" => "value".into(),
        "class" | "function" | "match" | "new" | "self" | "parent" | "static" | "use" => {
            format!("{output}Value")
        }
        _ => output,
    };
    if output
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        format!("value{output}")
    } else {
        output
    }
}

pub(super) fn package_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut previous_separator = true;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.extend(character.to_lowercase());
            previous_separator = false;
        } else if !previous_separator && !slug.is_empty() {
            slug.push('-');
            previous_separator = true;
        }
    }
    slug.trim_matches('-').to_owned().if_empty("api")
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.into()
        } else {
            self
        }
    }
}

pub(super) fn namespace_for_package(package_name: &str) -> String {
    package_name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .map(type_name)
        .collect::<Vec<_>>()
        .join("\\")
}

pub(super) fn php_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

pub(super) fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
