use super::*;
use anyhow::ensure;

const TYPE_KEY: &str = "poolster.swift.multipart_type";
fn json_media(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value == "application/json" || value.ends_with("+json")
}

const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

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
fn resolve<'a>(api: &'a Api, value: &'a SchemaValue, depth: usize) -> Result<&'a SchemaValue> {
    ensure!(
        depth < 12,
        "Swift multipart schema references exceed supported depth"
    );
    if let SchemaKind::Reference { reference } = &value.kind {
        let name = reference.rsplit('/').next().unwrap_or(reference);
        let schema = api
            .schemas
            .iter()
            .find(|schema| schema.name == name)
            .ok_or_else(|| anyhow::anyhow!("Swift multipart reference {reference:?} is missing"))?;
        return resolve(api, &schema.value, depth + 1);
    }
    Ok(value)
}
#[derive(Clone, Copy)]
enum Shape {
    Scalar,
    Json,
    File,
    ScalarArray,
    JsonArray,
    FileArray,
}
fn shape(api: &Api, value: &SchemaValue) -> Result<Shape> {
    let value = resolve(api, value, 0)?;
    ensure!(
        !value.nullable && !value.nullish,
        "Swift multipart nullable parts require an adapter"
    );
    Ok(match &value.kind {
        SchemaKind::String if value.format.as_deref() == Some("binary") => Shape::File,
        SchemaKind::String | SchemaKind::Boolean | SchemaKind::Integer | SchemaKind::Number => {
            Shape::Scalar
        }
        SchemaKind::Object { .. } | SchemaKind::Any => Shape::Json,
        SchemaKind::Array { items } => match shape(api, items)? {
            Shape::File => Shape::FileArray,
            Shape::Scalar => Shape::ScalarArray,
            Shape::Json | Shape::ScalarArray | Shape::JsonArray => Shape::JsonArray,
            Shape::FileArray => bail!("Swift multipart nested file arrays require an adapter"),
        },
        _ => bail!("Swift multipart union/composition/null parts require an adapter"),
    })
}
fn fields<'a>(api: &'a Api, operation: &'a Operation) -> Result<&'a [Field]> {
    let body = operation.request_body.as_ref().unwrap();
    ensure!(
        body.media_types.iter().all(|media| media
            .content_type
            .eq_ignore_ascii_case("multipart/form-data")
            || json_media(&media.content_type)),
        "Swift multipart operation '{}' supports multipart/form-data with one optional JSON alternative",
        operation.id
    );
    ensure!(
        body.media_types
            .iter()
            .filter(|media| json_media(&media.content_type))
            .count()
            <= 1,
        "Swift multipart operation '{}' has ambiguous JSON alternatives",
        operation.id
    );
    ensure!(
        body.media_types
            .iter()
            .filter(|media| media
                .content_type
                .eq_ignore_ascii_case("multipart/form-data"))
            .count()
            == 1,
        "Swift multipart operation has ambiguous multipart alternatives"
    );
    let media = body
        .media_types
        .iter()
        .find(|media| {
            media
                .content_type
                .eq_ignore_ascii_case("multipart/form-data")
        })
        .unwrap();
    let schema = media
        .schema
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Swift multipart root schema is required"))?;
    ensure!(
        matches!(schema.kind, SchemaKind::Reference { .. }),
        "Swift multipart root must be a named closed object"
    );
    let schema = resolve(api, schema, 0)?;
    ensure!(
        !schema.nullable && !schema.nullish,
        "Swift multipart root cannot be nullable"
    );
    let SchemaKind::Object {
        fields,
        additional_properties: AdditionalProperties::Forbidden,
    } = &schema.kind
    else {
        bail!(
            "Swift multipart root must be a named closed object; open/union roots require an adapter"
        )
    };
    ensure!(
        fields.len() <= 128,
        "Swift multipart root exceeds 128 parts"
    );
    let mut identifiers = std::collections::BTreeSet::new();
    for field in fields {
        ensure!(
            !field.name.is_empty()
                && field
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-[]".contains(&byte)),
            "Swift multipart part name {:?} is unsupported",
            field.name
        );
        let native = identifier(&field.name);
        ensure!(
            identifiers.insert(native.clone()),
            "Swift multipart fields collide as {native}"
        );
        ensure!(
            !matches!(
                native.trim_matches('`'),
                "poolsterEncoded" | "partHeaders" | "maximumBodyBytes"
            ),
            "Swift multipart field {:?} collides with a generated encoding control",
            field.name
        );
        let kind = shape(api, &field.value)?;
        let encoding = encoding(operation, &field.name);
        if let Some(style) = encoding
            .and_then(|value| value.get("style"))
            .and_then(Value::as_str)
        {
            ensure!(
                style == "form",
                "Swift multipart encoding style {style:?} requires an adapter"
            )
        }
        if encoding.is_some_and(|value| {
            value.get("style").is_some()
                || value.get("explode").is_some()
                || value.get("allowReserved").is_some()
        }) {
            ensure!(
                matches!(kind, Shape::Scalar | Shape::ScalarArray),
                "Swift multipart explicit form serialization supports scalar parts and scalar arrays; JSON/files use contentType encoding"
            )
        }
        if let Some(content_type) = encoding
            .and_then(|value| value.get("contentType"))
            .and_then(Value::as_str)
        {
            ensure!(
                valid_content_type(content_type),
                "Swift multipart encoding contentType {content_type:?} must be a single concrete media type"
            )
        }
        if let Some(content_type) = encoding
            .and_then(|value| value.get("contentType"))
            .and_then(Value::as_str)
        {
            if matches!(kind, Shape::Json | Shape::JsonArray) {
                ensure!(
                    json_media(content_type),
                    "Swift multipart JSON parts require a JSON contentType; custom serializers require an adapter"
                );
            }
        }
        if let Some(headers) = encoding
            .and_then(|value| value.get("headers"))
            .and_then(Value::as_object)
        {
            for name in headers.keys() {
                ensure!(
                    valid_header_name(name)
                        && !name.eq_ignore_ascii_case("Content-Disposition")
                        && !name.eq_ignore_ascii_case("Content-Length"),
                    "Swift multipart declared part header {name:?} requires an adapter"
                )
            }
        }
    }
    Ok(fields)
}
fn encoding<'a>(operation: &'a Operation, name: &str) -> Option<&'a Value> {
    operation
        .annotations
        .get("poolster.request_body_encodings")?
        .as_object()?
        .iter()
        .find(|(media, _)| media.eq_ignore_ascii_case("multipart/form-data"))?
        .1
        .get(name)
}
fn valid_header_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}
fn valid_content_type(value: &str) -> bool {
    let Some((major, minor)) = value.split_once('/') else {
        return false;
    };
    valid_header_name(major)
        && valid_header_name(minor)
        && !value.contains('*')
        && !minor.contains('/')
}
fn dto_name(operation: &Operation) -> String {
    format!("{}MultipartBody", type_name(&operation.id))
}
fn mixed(operation: &Operation) -> bool {
    operation
        .request_body
        .as_ref()
        .unwrap()
        .media_types
        .iter()
        .any(|media| json_media(&media.content_type))
}
fn ordered(api: &Api, operation: &Operation) -> bool {
    let Some(body) = operation.request_body.as_ref() else {
        return false;
    };
    fields(api, operation).is_err()
        || body.media_types.iter().any(|media| {
            media
                .content_type
                .to_ascii_lowercase()
                .starts_with("multipart/")
                && (media.content_type != "multipart/form-data"
                    || media
                        .schema
                        .as_ref()
                        .and_then(|s| resolve(api, s, 0).ok())
                        .is_some_and(|s| matches!(s.kind, SchemaKind::Array { .. })))
        })
        || poolster_core::openapi32::request_content(operation)
            .ok()
            .is_some_and(|content| {
                content
                    .iter()
                    .any(|media| !media.prefix_encoding.is_empty() || media.item_encoding.is_some())
            })
}
pub(crate) fn prepare(api: &Api) -> Result<std::borrow::Cow<'_, Api>> {
    if !api.operations.iter().any(selected) {
        return Ok(std::borrow::Cow::Borrowed(api));
    }
    let mut prepared = api.clone();
    for operation in prepared
        .operations
        .iter_mut()
        .filter(|operation| selected(operation))
    {
        if !ordered(api, operation) {
            fields(api, operation)?;
        }
        let body_type = if mixed(operation) {
            format!("{}RequestBody", type_name(&operation.id))
        } else {
            dto_name(operation)
        };
        for reserved in [
            dto_name(operation),
            body_type.clone(),
            "PoolsterMultipartFile".into(),
            "PoolsterMultipartError".into(),
            "PoolsterMultipartEncoder".into(),
            "PoolsterEncodedMultipart".into(),
            "PoolsterMultipartPart".into(),
            "PoolsterOrderedPart".into(),
            "PoolsterOrderedDefinition".into(),
            "PoolsterOrderedEncoding".into(),
        ] {
            ensure!(
                !api.schemas
                    .iter()
                    .any(|schema| type_name(&schema.name) == reserved),
                "Swift multipart generated type {reserved} collides with a schema"
            )
        }
        operation
            .annotations
            .insert(TYPE_KEY.into(), Value::String(body_type));
    }
    Ok(std::borrow::Cow::Owned(prepared))
}
pub(crate) fn body_type(operation: &Operation) -> Option<&str> {
    operation.annotations.get(TYPE_KEY).and_then(Value::as_str)
}
mod emission;
pub(crate) use emission::*;

#[cfg(test)]
mod tests;
