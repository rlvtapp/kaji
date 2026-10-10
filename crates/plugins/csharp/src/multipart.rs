use super::*;
pub(crate) fn selected(operation: &Operation) -> bool {
    operation.request_body.as_ref().is_some_and(|body| {
        body.media_types.iter().any(|media| {
            media
                .content_type
                .to_ascii_lowercase()
                .starts_with("multipart/")
        })
    })
}
fn ordered_plan(api: &Api, operation: &Operation) -> Option<serde_json::Value> {
    let content = poolster_core::openapi32::request_content(operation)
        .ok()
        .and_then(|definitions| {
            definitions.into_iter().find(|item| {
                item.content_type
                    .to_ascii_lowercase()
                    .starts_with("multipart/")
            })
        });
    let media = operation
        .request_body
        .as_ref()?
        .media_types
        .iter()
        .find(|media| {
            media
                .content_type
                .to_ascii_lowercase()
                .starts_with("multipart/")
        })?;
    fn advanced(encoding: &poolster_core::openapi32::Encoding) -> bool {
        encoding
            .style
            .as_deref()
            .is_some_and(|style| style != "form")
            || !encoding.headers.is_empty()
            || !encoding.encoding.is_empty()
            || !encoding.prefix_encoding.is_empty()
            || encoding.item_encoding.is_some()
    }
    let needed = fields(api, operation).is_err()
        || !media
            .content_type
            .eq_ignore_ascii_case("multipart/form-data")
        || media.schema.as_ref().is_none_or(|schema| {
            schema.nullable
                || schema.nullish
                || matches!(
                    schema.kind,
                    SchemaKind::Any
                        | SchemaKind::Null
                        | SchemaKind::String
                        | SchemaKind::Number
                        | SchemaKind::Integer
                        | SchemaKind::Boolean
                )
        })
        || media
            .schema
            .as_ref()
            .is_some_and(|schema| matches!(schema.kind, SchemaKind::Array { .. }))
        || content.as_ref().is_some_and(|item| {
            !item.prefix_encoding.is_empty()
                || item.item_encoding.is_some()
                || item.encoding.values().any(advanced)
        });
    if !needed {
        return None;
    }
    Some(
        content
            .and_then(|item| serde_json::to_value(item).ok())
            .unwrap_or_else(|| serde_json::json!({"content_type":media.content_type})),
    )
}
pub(crate) fn body_name(operation: &Operation) -> String {
    let name = format!("{}MultipartBody", pascal_case(&operation.id));
    let collision=operation.request_body.as_ref().is_some_and(|body|body.media_types.iter().filter_map(|media|media.schema.as_ref()).any(|value|matches!(&value.kind,SchemaKind::Reference{reference} if pascal_case(reference.rsplit('/').next().unwrap_or(reference))==name)));
    if collision {
        format!("{name}Wire")
    } else {
        name
    }
}
pub(crate) fn mixed(operation: &Operation) -> bool {
    selected(operation)
        && operation
            .request_body
            .as_ref()
            .is_some_and(|body| body.media_types.len() > 1)
}

fn root_fields(api: &Api, value: &SchemaValue, depth: usize) -> Result<Vec<poolster_core::Field>> {
    anyhow::ensure!(
        depth < 12,
        "Multipart root references exceed supported depth"
    );
    let value = resolved(api, value, 0);
    match &value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let _ = additional_properties;
            Ok(fields.clone())
        }
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } if !variants.is_empty() => {
            let variants = variants
                .iter()
                .map(|value| root_fields(api, value, depth + 1))
                .collect::<Result<Vec<_>>>()?;
            let mut fields = std::collections::BTreeMap::<String, poolster_core::Field>::new();
            for variant in &variants {
                for field in variant {
                    if let Some(existing) = fields.get_mut(&field.name) {
                        if existing.value != field.value {
                            existing.value = SchemaValue::new(SchemaKind::Any);
                        }
                    } else {
                        fields.insert(field.name.clone(), field.clone());
                    }
                }
            }
            for field in fields.values_mut() {
                field.required = variants.iter().all(|variant| {
                    variant
                        .iter()
                        .any(|member| member.name == field.name && member.required)
                });
            }
            Ok(fields.into_values().collect())
        }
        _ => anyhow::bail!("Multipart root must be an object or union of closed objects"),
    }
}
fn open_root(api: &Api, value: &SchemaValue, depth: usize) -> bool {
    if depth > 12 {
        return false;
    }
    match &resolved(api, value, 0).kind {
        SchemaKind::Object {
            additional_properties,
            ..
        } => !matches!(additional_properties, AdditionalProperties::Forbidden),
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => variants
            .iter()
            .any(|value| open_root(api, value, depth + 1)),
        _ => false,
    }
}
fn extra_parts(api: &Api, operation: &Operation) -> bool {
    operation
        .request_body
        .as_ref()
        .and_then(|body| {
            body.media_types.iter().find(|media| {
                media
                    .content_type
                    .eq_ignore_ascii_case("multipart/form-data")
            })
        })
        .and_then(|media| media.schema.as_ref())
        .is_some_and(|value| open_root(api, value, 0))
}
fn fields(api: &Api, operation: &Operation) -> Result<Vec<poolster_core::Field>> {
    let body = operation.request_body.as_ref().unwrap();
    anyhow::ensure!(
        body.media_types
            .iter()
            .all(|media| !media.content_type.starts_with("multipart/")
                || media
                    .content_type
                    .eq_ignore_ascii_case("multipart/form-data")),
        "Only multipart/form-data multipart alternatives are supported"
    );
    let media = body
        .media_types
        .iter()
        .find(|media| {
            media
                .content_type
                .eq_ignore_ascii_case("multipart/form-data")
        })
        .ok_or_else(|| anyhow::anyhow!("Missing multipart/form-data media"))?;
    let schema = media
        .schema
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Multipart root schema is required"))?;
    anyhow::ensure!(
        !schema.nullable && !schema.nullish,
        "Multipart root cannot be nullable"
    );
    let fields = root_fields(api, schema, 0)?;
    for field in &fields {
        anyhow::ensure!(
            !field.name.is_empty()
                && field
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-[]".contains(&b)),
            "multipart operation '{}' has an unsupported part name",
            operation.id
        );
        anyhow::ensure!(
            field.value.format.as_deref() != Some("byte"),
            "multipart operation '{}' field '{}' uses ambiguous base64 byte format; use binary for raw file bytes",
            operation.id,
            field.name
        );
    }
    Ok(fields)
}
pub(crate) fn validate(api: &Api) -> Result<()> {
    for op in api.operations.iter().filter(|op| selected(op)) {
        if ordered_plan(api, op).is_none() {
            fields(api, op)?;
        }
    }
    Ok(())
}
fn binary(field: &poolster_core::Field) -> bool {
    field.value.format.as_deref() == Some("binary")
        && matches!(field.value.kind, SchemaKind::String)
}
fn resolved<'a>(api: &'a Api, value: &'a SchemaValue, depth: usize) -> &'a SchemaValue {
    if depth < 12 {
        if let SchemaKind::Reference { reference } = &value.kind {
            if let Some(schema) = api
                .schemas
                .iter()
                .find(|s| s.name == reference.rsplit('/').next().unwrap_or(reference))
            {
                return resolved(api, &schema.value, depth + 1);
            }
        }
    }
    value
}
fn file_shape(api: &Api, value: &SchemaValue, depth: usize) -> Option<bool> {
    if depth > 12 {
        return None;
    }
    let value = resolved(api, value, 0);
    match &value.kind {
        SchemaKind::String if value.format.as_deref() == Some("binary") => Some(false),
        SchemaKind::Array { items } if file_shape(api, items, depth + 1) == Some(false) => {
            Some(true)
        }
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } if !variants.is_empty() => {
            let shapes = variants
                .iter()
                .map(|value| file_shape(api, value, depth + 1))
                .collect::<Option<Vec<_>>>()?;
            Some(shapes.into_iter().any(|array| array))
        }
        _ => None,
    }
}
fn encoding<'a>(operation: &'a Operation, name: &str) -> Option<&'a serde_json::Value> {
    operation
        .annotations
        .get("poolster.request_body_encodings")?
        .get("multipart/form-data")?
        .get(name)
}
mod emission;
pub(crate) use emission::*;

#[cfg(test)]
#[path = "multipart/tests.rs"]
mod tests;
