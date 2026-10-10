use super::*;

pub(crate) fn render_models(api: &Api) -> String {
    let mut output = format!("{NOTICE}\n");
    for schema in &api.schemas {
        render_description(&mut output, schema.value.description.as_deref());
        let _ = writeln!(
            output,
            "export type {} = {};\n",
            type_identifier(&schema.name),
            render_value(&schema.value)
        );
    }
    output
}

pub(crate) fn render_value(value: &SchemaValue) -> String {
    let primitive = match &value.kind {
        SchemaKind::Any => "unknown".to_owned(),
        SchemaKind::Null => "null".to_owned(),
        SchemaKind::Boolean => "boolean".to_owned(),
        SchemaKind::Integer => {
            match poolster_core::poolster_extension(&value.extensions, "integer")
                .and_then(Value::as_str)
            {
                Some("bigint") => "bigint".into(),
                Some("string") => "string".into(),
                _ => "number".into(),
            }
        }
        SchemaKind::Number => "number".to_owned(),
        SchemaKind::String => "string".to_owned(),
        SchemaKind::Array { items } => format!("Array<{}>", render_value(items)),
        SchemaKind::Object {
            fields,
            additional_properties,
        } => render_object(fields, additional_properties),
        SchemaKind::Reference { reference } => {
            type_identifier(reference.rsplit('/').next().unwrap_or(reference))
        }
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => {
            join_types(variants, " | ")
        }
        SchemaKind::AllOf { variants } => join_types(variants, " & "),
        // TypeScript has no sound type-level complement operator. `unknown` is
        // safer than emitting an invented constraint that rejects valid values.
        SchemaKind::Not { .. } => "unknown".to_owned(),
    };

    let constrained = if let Some(constant) = value
        .const_value
        .as_ref()
        .and_then(|literal| zod::artifact_literal(literal, value))
    {
        constant
    } else if !value.enum_values.is_empty() {
        let members = value
            .enum_values
            .iter()
            .filter_map(|literal| zod::artifact_literal(literal, value))
            .collect::<Vec<_>>();
        if members.is_empty() {
            primitive
        } else {
            members.join(" | ")
        }
    } else {
        primitive
    };

    let mut members = vec![constrained];
    if (value.nullable || value.nullish) && members[0] != "null" {
        members.push("null".into());
    }
    if value.optional || value.nullish {
        members.push("undefined".into());
    }
    members.join(" | ")
}

pub(crate) fn render_object(
    fields: &[poolster_core::ast::Field],
    additional_properties: &AdditionalProperties,
) -> String {
    let mut members = Vec::new();
    for field in fields {
        let optional = if field.required { "" } else { "?" };
        let mut member = String::new();
        if let Some(description) = field.value.description.as_deref() {
            member.push_str("/** ");
            member.push_str(&description.replace("*/", "* /"));
            member.push_str(" */ ");
        }
        let _ = write!(
            member,
            "{}{}: {}",
            property_name(&field.name),
            optional,
            render_value(&field.value)
        );
        members.push(member);
    }
    match additional_properties {
        AdditionalProperties::Any | AdditionalProperties::Unspecified => {
            members.push("[key: string]: unknown".to_owned());
        }
        AdditionalProperties::Schema { value } => {
            members.push(format!("[key: string]: {}", render_value(value)));
        }
        AdditionalProperties::Forbidden => {}
    }
    if members.is_empty() {
        "Record<string, never>".to_owned()
    } else {
        format!("{{ {} }}", members.join("; "))
    }
}

pub(crate) fn join_types(values: &[SchemaValue], separator: &str) -> String {
    if values.is_empty() {
        "unknown".to_owned()
    } else {
        values
            .iter()
            .map(|value| {
                let rendered = render_value(value);
                // `&` binds more tightly than `|` in TypeScript. Parenthesize
                // union members of an intersection so `allOf` keeps its JSON
                // Schema grouping instead of changing its meaning on render.
                if separator == " & " && rendered.contains(" | ") {
                    format!("({rendered})")
                } else {
                    rendered
                }
            })
            .collect::<Vec<_>>()
            .join(separator)
    }
}

pub(crate) fn ts_literal(value: &Value) -> Option<String> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            serde_json::to_string(value).ok()
        }
        Value::Array(values) => values
            .iter()
            .map(ts_literal)
            .collect::<Option<Vec<_>>>()
            .map(|values| format!("readonly [{}]", values.join(", "))),
        Value::Object(values) => values
            .iter()
            .map(|(name, value)| {
                ts_literal(value).map(|value| format!("{}: {value}", js_string(name)))
            })
            .collect::<Option<Vec<_>>>()
            .map(|values| format!("{{ {} }}", values.join("; "))),
    }
}

pub(crate) fn render_description(output: &mut String, description: Option<&str>) {
    if let Some(description) = description {
        output.push_str("/**\n");
        for line in description.lines() {
            let _ = writeln!(output, " * {}", line.replace("*/", "* /"));
        }
        output.push_str(" */\n");
    }
}
