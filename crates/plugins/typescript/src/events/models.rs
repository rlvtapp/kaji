//! JSON Schema message models for the Kafka contract.
use super::*;

pub(super) fn render(value: &SchemaValue) -> Result<String> {
    let mut ty = if let Some(value) = &value.const_value {
        serde_json::to_string(value)?
    } else if !value.enum_values.is_empty() {
        value
            .enum_values
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join(" | ")
    } else {
        match &value.kind {
            SchemaKind::String => "string".into(),
            SchemaKind::Integer | SchemaKind::Number => "number".into(),
            SchemaKind::Boolean => "boolean".into(),
            SchemaKind::Null => "null".into(),
            SchemaKind::Array { items } => format!("Array<{}>", render(items)?),
            SchemaKind::Object {
                fields,
                additional_properties,
            } => {
                let mut object = String::from("{ ");
                for f in fields {
                    write!(
                        object,
                        "{}{}: {}; ",
                        serde_json::to_string(&f.name)?,
                        if f.required { "" } else { "?" },
                        render(&f.value)?
                    )?;
                }
                object.push('}');
                match additional_properties {
                    AdditionalProperties::Forbidden => object,
                    AdditionalProperties::Any | AdditionalProperties::Unspecified => {
                        format!("{object} & Record<string, unknown>")
                    }
                    AdditionalProperties::Schema { .. } => {
                        bail!("typed additionalProperties unsupported for event TypeScript")
                    }
                }
            }
            _ => bail!("unsupported event model kind"),
        }
    };
    if value.nullable {
        ty = format!("({ty}) | null");
    }
    Ok(ty)
}
