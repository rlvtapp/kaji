use super::*;

pub(crate) fn render_schema(
    name: &str,
    value: &SchemaValue,
    options: &ModelOptions,
    notice: &str,
) -> String {
    let identifier = type_identifier(name);
    if options.open_enums && !value.enum_values.is_empty() {
        let mut closed = options.clone();
        closed.open_enums = false;
        if matches!(closed.enum_type, EnumType::Enum | EnumType::ConstEnum) {
            closed.enum_type = EnumType::AsConst;
            closed.enum_type_suffix.clear();
            closed.enum_const_casing = EnumConstCasing::PascalCase;
        }
        let source = render_schema(name, value, &closed, notice);
        let future = open_enum_wire(value, options);
        if future.is_empty() {
            return source;
        }
        return source
            .lines()
            .map(|line| {
                if line.starts_with("export type ") {
                    format!("{line} | {future}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
    }

    if !value.enum_values.is_empty()
        && crate::json::representation(value, options) != Int64Type::Number
    {
        let literals = value
            .enum_values
            .iter()
            .map(|v| integer_literal(v, value, options))
            .collect::<Vec<_>>();
        if options.enum_type == EnumType::Literal {
            return format!(
                "{notice}export type {identifier} = {}\n",
                literals.join(" | ")
            );
        }
        let value_name = if options.enum_type == EnumType::AsConst
            && options.enum_const_casing == EnumConstCasing::CamelCase
        {
            lower_camel_identifier(&identifier)
        } else {
            identifier.clone()
        };
        let type_name = if options.enum_type == EnumType::AsConst {
            format!("{identifier}{}", options.enum_type_suffix)
        } else {
            identifier.clone()
        };
        let members = value
            .enum_values
            .iter()
            .zip(literals)
            .enumerate()
            .map(|(i, (value, literal))| {
                format!(
                    "  {}: {literal},",
                    enum_member_name(&identifier, value, i, options.enum_key_casing)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        return format!(
            "{notice}export const {value_name} = {{\n{members}\n}} as const\nexport type {type_name} = (typeof {value_name})[keyof typeof {value_name}]\n"
        );
    }
    if !value.enum_values.is_empty() {
        return match options.enum_type {
            EnumType::Literal => format!(
                "{notice}export type {identifier} = {}\n",
                value
                    .enum_values
                    .iter()
                    .map(poolster_literal)
                    .collect::<Vec<_>>()
                    .join(" | ")
            ),
            EnumType::AsConst => {
                render_as_const_enum(&identifier, &value.enum_values, options, notice)
            }
            EnumType::Enum => render_named_enum(&identifier, &value.enum_values, options, notice),
            EnumType::ConstEnum => {
                render_const_enum(&identifier, &value.enum_values, options, notice)
            }
        };
    }
    if let (
        Syntax::Interface,
        SchemaKind::Object {
            fields,
            additional_properties,
        },
    ) = (options.syntax, &value.kind)
    {
        return format!(
            "{notice}export interface {identifier} {}\n",
            render_object(fields, additional_properties, options)
        );
    }
    let body = match &value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => render_object(fields, additional_properties, options),
        _ => render_value(value, options),
    };
    format!("{notice}export type {identifier} = {body}\n")
}

pub(crate) fn open_enum_wire(value: &SchemaValue, options: &ModelOptions) -> String {
    let mut kinds = BTreeSet::new();
    for literal in &value.enum_values {
        let kind = if literal.is_string() {
            "string"
        } else if literal.is_boolean() {
            "boolean"
        } else if literal.is_number() {
            if crate::json::representation(value, options) == Int64Type::String {
                "string"
            } else if crate::json::representation(value, options) == Int64Type::BigInt {
                "bigint"
            } else {
                "number"
            }
        } else if literal.is_null() {
            "null"
        } else {
            continue;
        };
        kinds.insert(if kind == "null" {
            "null".to_owned()
        } else {
            format!("({kind} & {{}})")
        });
    }
    kinds.into_iter().collect::<Vec<_>>().join(" | ")
}

pub(crate) fn render_as_const_enum(
    identifier: &str,
    values: &[Value],
    options: &ModelOptions,
    notice: &str,
) -> String {
    let value_name = match options.enum_const_casing {
        EnumConstCasing::CamelCase => lower_camel_identifier(identifier),
        EnumConstCasing::PascalCase => identifier.into(),
    };
    let mut source = format!("{notice}export const {value_name} = {{\n");
    for (index, value) in values.iter().enumerate() {
        let key = enum_member_name(identifier, value, index, options.enum_key_casing);
        let _ = writeln!(source, "  {key}: {},", poolster_literal(value));
    }
    source.push_str("} as const\n\n");
    source.push_str(&format!(
        "export type {identifier}{} = (typeof {value_name})[keyof typeof {value_name}]\n",
        options.enum_type_suffix
    ));
    source
}

pub(crate) fn render_named_enum(
    identifier: &str,
    values: &[Value],
    options: &ModelOptions,
    notice: &str,
) -> String {
    render_enum_declaration(identifier, values, "export enum", options, notice)
}

pub(crate) fn render_const_enum(
    identifier: &str,
    values: &[Value],
    options: &ModelOptions,
    notice: &str,
) -> String {
    render_enum_declaration(identifier, values, "export const enum", options, notice)
}

pub(crate) fn render_enum_declaration(
    identifier: &str,
    values: &[Value],
    declaration: &str,
    options: &ModelOptions,
    notice: &str,
) -> String {
    let mut source = format!("{notice}{declaration} {identifier} {{\n");
    for (index, value) in values.iter().enumerate() {
        let key = enum_member_name(identifier, value, index, options.enum_key_casing);
        let _ = writeln!(source, "  {key} = {},", poolster_literal(value));
    }
    source.push_str("}\n");
    source
}

pub(crate) fn enum_member_name(
    identifier: &str,
    value: &Value,
    index: usize,
    casing: EnumKeyCasing,
) -> String {
    let Some(value) = value.as_str() else {
        return match casing {
            EnumKeyCasing::None => format!("{identifier}_{}", index + 1),
            EnumKeyCasing::CamelCase => {
                format!("{}{}", lower_camel_identifier(identifier), index + 1)
            }
            EnumKeyCasing::PascalCase => format!("{identifier}{}", index + 1),
            EnumKeyCasing::SnakeCase => {
                format!("{}_{}", identifier.to_ascii_lowercase(), index + 1)
            }
            EnumKeyCasing::ScreamingSnakeCase => {
                format!("{}_{}", identifier.to_ascii_uppercase(), index + 1)
            }
        };
    };
    if matches!(casing, EnumKeyCasing::None) {
        return property_name(value);
    }
    let words = value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if words.is_empty() {
        return property_name(value);
    }
    let name = match casing {
        EnumKeyCasing::None => unreachable!("handled above"),
        EnumKeyCasing::CamelCase => words
            .iter()
            .enumerate()
            .map(|(index, word)| {
                if index == 0 {
                    word.clone()
                } else {
                    upper_camel_word(word)
                }
            })
            .collect(),
        EnumKeyCasing::PascalCase => words.iter().map(|word| upper_camel_word(word)).collect(),
        EnumKeyCasing::SnakeCase => words.join("_"),
        EnumKeyCasing::ScreamingSnakeCase => words.join("_").to_ascii_uppercase(),
    };
    property_name(&name)
}

pub(crate) fn upper_camel_word(word: &str) -> String {
    let mut characters = word.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

pub(crate) fn render_object_fields(
    fields: &[poolster_core::ast::Field],
    options: &ModelOptions,
) -> String {
    render_object(fields, &AdditionalProperties::Forbidden, options)
}

/// Poolster's TypeScript printer turns OpenAPI `readOnly` into a property modifier
/// and collapses `additionalProperties` into one string index signature. An
/// index signature alongside named properties must be `unknown` because each
/// declared property has to be assignable to it in TypeScript.
pub(crate) fn render_object(
    fields: &[poolster_core::ast::Field],
    additional_properties: &AdditionalProperties,
    options: &ModelOptions,
) -> String {
    if fields.is_empty() && matches!(additional_properties, AdditionalProperties::Forbidden) {
        return "object".into();
    }
    let mut object = String::from("{\n");
    for field in fields {
        if options.remove_optional_properties && !field.required {
            continue;
        }
        let optional = optional_marker(field.required, options.optional_type);
        let read_only = field.value.read_only
            || field
                .annotations
                .get("readOnly")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        let _ = writeln!(
            object,
            "  {}{}{}: {}",
            if read_only { "readonly " } else { "" },
            property_name(&field.name),
            optional,
            render_optional_value(&field.value, field.required, options)
        );
    }
    let index_value = match additional_properties {
        AdditionalProperties::Any | AdditionalProperties::Unspecified => Some("unknown".into()),
        AdditionalProperties::Schema { value } if fields.is_empty() => {
            Some(render_value(value, options))
        }
        AdditionalProperties::Schema { .. } if !fields.is_empty() => Some("unknown".into()),
        AdditionalProperties::Forbidden => None,
        AdditionalProperties::Schema { .. } => unreachable!("fields emptiness is exhaustive"),
    };
    if let Some(index_value) = index_value {
        let _ = writeln!(object, "  [key: string]: {index_value}");
    }
    object.push('}');
    object
}
