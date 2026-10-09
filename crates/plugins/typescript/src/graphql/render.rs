//! Preserve input/output direction and schema wrappers while rendering scalars.
use super::{GraphqlScalarMapping, ModelField, ModelKind, ModelType};
use std::{collections::BTreeMap, fmt::Write};

pub(super) fn render_fields(
    fields: &[ModelField],
    scalars: &BTreeMap<String, GraphqlScalarMapping>,
    input: bool,
) -> String {
    let mut out = String::from("{ ");
    for f in fields {
        write!(
            out,
            "{}{}: {}; ",
            serde_json::to_string(&f.name).unwrap(),
            if f.optional { "?" } else { "" },
            render_type(&f.ty, scalars, input)
        )
        .unwrap();
    }
    out.push('}');
    out
}
pub(super) fn render_type(
    ty: &ModelType,
    scalars: &BTreeMap<String, GraphqlScalarMapping>,
    input: bool,
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
        ModelKind::List(item) => format!("Array<{}>", render_type(item, scalars, input)),
        ModelKind::Object(fields) => render_fields(fields, scalars, input),
        ModelKind::Union(types) if types.is_empty() => "never".into(),
        ModelKind::Union(types) => types
            .iter()
            .map(|ty| render_type(ty, scalars, input))
            .collect::<Vec<_>>()
            .join(" | "),
    };
    if ty.nullable {
        format!("({value}) | null")
    } else {
        value
    }
}
