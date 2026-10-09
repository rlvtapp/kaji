//! Preserve input/output direction and schema wrappers while rendering scalars.
use super::{GraphqlScalarMapping, ModelField, ModelKind, ModelType};
use std::{collections::BTreeMap, fmt::Write};

pub(super) fn render_fields(
    fields: &[ModelField],
    scalars: &BTreeMap<String, GraphqlScalarMapping>,
    input: bool,
) -> String {
    render_fields_at(fields, scalars, input, 0)
}
fn render_fields_at(
    fields: &[ModelField],
    scalars: &BTreeMap<String, GraphqlScalarMapping>,
    input: bool,
    depth: usize,
) -> String {
    if fields.is_empty() {
        return "{}".into();
    }
    let indent = "  ".repeat(depth + 1);
    let mut out = String::from("{\n");
    for f in fields {
        writeln!(
            out,
            "{indent}{}{}: {};",
            serde_json::to_string(&f.name).unwrap(),
            if f.optional { "?" } else { "" },
            render_type_at(&f.ty, scalars, input, depth + 1)
        )
        .unwrap();
    }
    out.push_str(&"  ".repeat(depth));
    out.push('}');
    out
}
pub(super) fn render_type(
    ty: &ModelType,
    scalars: &BTreeMap<String, GraphqlScalarMapping>,
    input: bool,
) -> String {
    render_type_at(ty, scalars, input, 0)
}
fn render_type_at(
    ty: &ModelType,
    scalars: &BTreeMap<String, GraphqlScalarMapping>,
    input: bool,
    depth: usize,
) -> String {
    let value = match &ty.kind {
        ModelKind::Scalar(s) if scalars.contains_key(s) => {
            let mapping = &scalars[s];
            format!(
                "({})",
                if input {
                    &mapping.input
                } else {
                    &mapping.output
                }
            )
        }
        ModelKind::Scalar(s) => match s.as_str() {
            "Int" | "Float" => "number",
            "String" | "ID" => "string",
            "Boolean" => "boolean",
            _ => "unknown",
        }
        .into(),
        ModelKind::Enum(values) => values
            .iter()
            .map(|v| serde_json::to_string(v).unwrap())
            .collect::<Vec<_>>()
            .join(" | "),
        ModelKind::Literal(value) => serde_json::to_string(value).unwrap(),
        ModelKind::Named(name) => name.clone(),
        ModelKind::List(item) => format!("Array<{}>", render_type_at(item, scalars, input, depth)),
        ModelKind::Object(fields) => render_fields_at(fields, scalars, input, depth),
        ModelKind::Union(types) if types.is_empty() => "never".into(),
        ModelKind::Union(types) => types
            .iter()
            .map(|ty| render_type_at(ty, scalars, input, depth))
            .collect::<Vec<_>>()
            .join(" | "),
    };
    if ty.nullable {
        format!("({value}) | null")
    } else {
        value
    }
}
