use super::*;

pub(crate) fn render_optional_value(
    value: &SchemaValue,
    required: bool,
    options: &ModelOptions,
) -> String {
    let rendered = render_value(value, options);
    if !required
        && matches!(
            options.optional_type,
            OptionalType::QuestionTokenAndUndefined | OptionalType::Undefined
        )
    {
        format!("{rendered} | undefined")
    } else {
        rendered
    }
}

pub(crate) fn optional_marker(required: bool, optional_type: OptionalType) -> &'static str {
    if required || matches!(optional_type, OptionalType::Undefined) {
        ""
    } else {
        "?"
    }
}

pub(crate) fn render_value(value: &SchemaValue, options: &ModelOptions) -> String {
    let body = if let Some(constant) = value.const_value.as_ref() {
        integer_literal(constant, value, options)
    } else {
        match &value.kind {
            SchemaKind::Any | SchemaKind::Not { .. } => "unknown".into(),
            SchemaKind::Null => "null".into(),
            SchemaKind::Boolean => "boolean".into(),
            SchemaKind::Integer => match crate::json::representation(value, options) {
                Int64Type::Number => "number".into(),
                Int64Type::String => "string".into(),
                Int64Type::BigInt => "bigint".into(),
            },
            SchemaKind::Number => "number".into(),
            SchemaKind::String => "string".into(),
            SchemaKind::Array { items } => match options.array_type {
                ArrayType::Array => {
                    let item = render_value(items, options);
                    if item.contains(" | ") || item.contains(" & ") {
                        format!("({item})[]")
                    } else {
                        format!("{item}[]")
                    }
                }
                ArrayType::Generic => format!("Array<{}>", render_value(items, options)),
            },
            SchemaKind::Reference { reference } => {
                poolster_core::poolster_extension(&value.extensions, "type-name")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        type_identifier(reference.rsplit('/').next().unwrap_or(reference))
                    })
            }
            SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => variants
                .iter()
                .map(|variant| render_value(variant, options))
                .collect::<Vec<_>>()
                .join(" | "),
            SchemaKind::AllOf { variants } => variants
                .iter()
                .map(|variant| render_value(variant, options))
                .collect::<Vec<_>>()
                .join(" & "),
            SchemaKind::Object {
                fields,
                additional_properties,
            } => render_object(fields, additional_properties, options),
        }
    };
    let body = if !value.enum_values.is_empty() {
        if let Some(name) = inline_enum_name(value) {
            match options.enum_type {
                EnumType::AsConst => format!("{name}{}", options.enum_type_suffix),
                EnumType::Enum | EnumType::ConstEnum => name.into(),
                EnumType::Literal => value
                    .enum_values
                    .iter()
                    .map(|literal| integer_literal(literal, value, options))
                    .collect::<Vec<_>>()
                    .join(" | "),
            }
        } else {
            value
                .enum_values
                .iter()
                .map(|literal| integer_literal(literal, value, options))
                .collect::<Vec<_>>()
                .join(" | ")
        }
    } else {
        body
    };
    let body = if options.open_enums && !value.enum_values.is_empty() {
        let wire = open_enum_wire(value, options);
        if wire.is_empty() {
            body
        } else {
            format!("{body} | {wire}")
        }
    } else {
        body
    };
    if value.nullable && body != "null" {
        format!("{body} | null")
    } else {
        body
    }
}

pub(crate) fn model_type_name(schema: &poolster_core::Schema, options: &ModelOptions) -> String {
    let name = type_identifier(&schema.name);
    if !schema.value.enum_values.is_empty() && options.enum_type == EnumType::AsConst {
        format!("{name}{}", options.enum_type_suffix)
    } else {
        name
    }
}

pub(crate) fn integer_literal(
    literal: &Value,
    schema: &SchemaValue,
    options: &ModelOptions,
) -> String {
    if literal.is_number() {
        match crate::json::representation(schema, options) {
            Int64Type::String => return poolster_literal(&Value::String(literal.to_string())),
            Int64Type::BigInt => return format!("{literal}n"),
            Int64Type::Number => {}
        }
    }
    poolster_literal(literal)
}

pub(crate) fn poolster_literal(value: &Value) -> String {
    match value {
        Value::String(value) => format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'")),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "unknown".into()),
    }
}

pub(crate) fn property_name(value: &str) -> String {
    if value
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '$'
        })
    {
        value.into()
    } else {
        poolster_literal(&Value::String(value.into()))
    }
}

/// Component names can share a terminal dotted segment (for example
/// `communications.participant` and `meeting.participant`). Keep every segment
/// in the filename so independently declared schemas never overwrite each
/// other in a large contract.
pub(crate) fn schema_file_identifier(value: &str) -> String {
    let identifier = type_identifier(value);
    const MAX_PREFIX_CHARS: usize = 96;
    if identifier.chars().count() <= MAX_PREFIX_CHARS {
        return identifier;
    }
    let prefix = identifier
        .chars()
        .take(MAX_PREFIX_CHARS)
        .collect::<String>();
    format!("{prefix}_{:016x}", stable_hash(value))
}

/// Stable module name for an operation's request/response types. Its exported
/// type keeps the complete operation name; only the filesystem component is
/// shortened for specifications with deeply nested operation IDs.
pub(crate) fn operation_model_file_identifier(value: &str) -> String {
    let identifier = type_identifier(value);
    const MAX_PREFIX_CHARS: usize = 96;
    if identifier.chars().count() <= MAX_PREFIX_CHARS {
        return identifier;
    }
    let prefix = identifier
        .chars()
        .take(MAX_PREFIX_CHARS)
        .collect::<String>();
    format!("{prefix}_{:016x}", stable_hash(value))
}

pub(crate) fn stable_hash(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

pub(crate) fn type_identifier(value: &str) -> String {
    crate::symbols::identifier(value)
}

pub(crate) fn lower_camel_identifier(value: &str) -> String {
    crate::symbols::camel(value)
}
