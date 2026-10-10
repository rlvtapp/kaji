//! HTTP models rendering.
use super::*;

pub(crate) fn render_model_for_api(
    api: &Api,
    schema: &Schema,
    open_unions: bool,
    open_enums: bool,
) -> String {
    fn reaches(
        api: &Api,
        value: &SchemaValue,
        goal: &str,
        seen: &mut std::collections::BTreeSet<String>,
    ) -> bool {
        match &value.kind {
            SchemaKind::Reference { reference } => {
                let target = reference.rsplit('/').next().unwrap_or(reference);
                if target == goal {
                    return true;
                }
                if !seen.insert(target.to_owned()) {
                    return false;
                }
                api.schemas
                    .iter()
                    .find(|item| item.name == target)
                    .is_some_and(|item| reaches(api, &item.value, goal, seen))
            }
            SchemaKind::Object { fields, .. } => fields
                .iter()
                .any(|field| reaches(api, &field.value, goal, seen)),
            SchemaKind::OneOf { variants }
            | SchemaKind::AnyOf { variants }
            | SchemaKind::AllOf { variants } => variants
                .iter()
                .any(|variant| reaches(api, variant, goal, seen)),
            _ => false,
        }
    }
    let mut schema = schema.clone();
    if let SchemaKind::Object { fields, .. } = &mut schema.value.kind {
        for field in fields {
            if matches!(field.value.kind, SchemaKind::Reference { .. })
                && reaches(api, &field.value, &schema.name, &mut Default::default())
            {
                field.value.format = Some("poolster-boxed-reference".into());
            }
        }
    }
    if let SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } = &mut schema.value.kind
    {
        for variant in variants {
            if matches!(variant.kind, SchemaKind::Reference { .. })
                && reaches(api, variant, &schema.name, &mut Default::default())
            {
                variant.format = Some("poolster-boxed-reference".into());
            }
        }
    }
    render_model(&schema, open_unions, open_enums)
}

pub(crate) fn render_model(schema: &Schema, open_unions: bool, open_enums: bool) -> String {
    let mut output = format!(
        "{NOTICE}\n\n#[allow(unused_imports)]\nuse crate::models::*;\n#[allow(unused_imports)]\nuse serde::{{Deserialize, Serialize}};\n\n"
    );
    render_schema(&mut output, schema, open_unions, open_enums);
    output
}

/// A file-system-safe module name. Including the source index makes otherwise
/// equivalent normalized OpenAPI component names distinct without relying on
/// a platform-specific hash implementation.
pub(crate) fn schema_module_name(name: &str, index: usize) -> String {
    format!("{}_{}", bounded_module_stem(name), index + 1)
}

pub(crate) fn bounded_module_stem(name: &str) -> String {
    let mut value = rust_field_name(name).replace("r#", "");
    if value.len() > 72 {
        value.truncate(72);
        value = value.trim_matches('_').to_owned();
    }
    if value.is_empty() {
        "value".into()
    } else {
        value
    }
}

pub(crate) fn render_schema(
    output: &mut String,
    schema: &Schema,
    open_unions: bool,
    open_enums: bool,
) {
    let name = type_name(&schema.name);
    match &schema.value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let names = crate::native_names::field_names(fields, rust_field_name, &[]);
            output.push_str("#[derive(Clone, Debug, Deserialize, Serialize)]\n");
            let _ = writeln!(output, "pub struct {name} {{");
            for field in fields {
                if field.name != names[&field.name].clone() {
                    let _ = writeln!(output, "    #[serde(rename = {:?})]", field.name);
                }
                if !field.required {
                    output.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
                }
                if !field.required && (field.value.nullable || field.value.nullish) {
                    output.push_str("    #[serde(default, deserialize_with = \"crate::models::poolster_deserialize_optional_nullable\")]\n");
                }
                let field_type = rust_type(&field.value);
                let field_type = if field.required {
                    field_type
                } else {
                    format!("Option<{field_type}>")
                };
                let _ = writeln!(
                    output,
                    "    pub {}: {},",
                    names[&field.name].clone(),
                    field_type
                );
            }
            let extra_type = match additional_properties {
                AdditionalProperties::Schema { value } => Some(rust_type(value)),
                AdditionalProperties::Any | AdditionalProperties::Unspecified => {
                    Some("serde_json::Value".into())
                }
                AdditionalProperties::Forbidden => None,
            };
            if let Some(extra_type) = extra_type {
                let mut extra_name = "additional_properties".to_owned();
                while fields
                    .iter()
                    .any(|field| names[&field.name].clone() == extra_name)
                {
                    extra_name.push('_');
                }
                output.push_str("    #[serde(flatten)]\n");
                let _ = writeln!(
                    output,
                    "    pub {extra_name}: std::collections::BTreeMap<String, {extra_type}>,"
                );
            }
            output.push_str("}\n");
        }
        SchemaKind::String
            if open_enums
                && !schema.value.enum_values.is_empty()
                && schema.value.enum_values.iter().all(Value::is_string) =>
        {
            crate::model_compatibility::render_open_enum(output, &name, &schema.value.enum_values);
        }
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => {
            output
                .push_str("#[derive(Clone, Debug, Deserialize, Serialize)]\n#[serde(untagged)]\n");
            let _ = writeln!(output, "pub enum {name} {{");
            for (index, variant) in variants.iter().enumerate() {
                let _ = writeln!(output, "    Variant{index}({}),", rust_type(variant));
            }
            if open_unions {
                output.push_str("    /// Retains a future variant without discarding its wire value.\n    Unknown(serde_json::Value),\n");
            }
            output.push_str("}\n");
        }
        _ => {
            let _ = writeln!(output, "pub type {name} = {};", rust_type(&schema.value));
        }
    }
}

pub(crate) fn rust_type(value: &SchemaValue) -> String {
    let base = match &value.kind {
        SchemaKind::Any | SchemaKind::Not { .. } => "serde_json::Value".into(),
        SchemaKind::Null => "()".into(),
        SchemaKind::Boolean => "bool".into(),
        SchemaKind::Integer => "i64".into(),
        SchemaKind::Number => "f64".into(),
        SchemaKind::String => match value.format.as_deref() {
            Some("binary") | Some("byte") => "Vec<u8>".into(),
            _ => "String".into(),
        },
        SchemaKind::Array { items } => format!("Vec<{}>", rust_type(items)),
        SchemaKind::Object { .. } => "serde_json::Value".into(),
        SchemaKind::Reference { reference } => {
            format!(
                "crate::models::{}",
                type_name(reference.rsplit('/').next().unwrap_or(reference))
            )
        }
        SchemaKind::OneOf { .. } | SchemaKind::AnyOf { .. } | SchemaKind::AllOf { .. } => {
            "serde_json::Value".into()
        }
    };
    let base = if value.format.as_deref() == Some("poolster-boxed-reference") {
        format!("Box<{base}>")
    } else {
        base
    };
    if (value.nullable || value.nullish) && base != "()" {
        format!("Option<{base}>")
    } else {
        base
    }
}
