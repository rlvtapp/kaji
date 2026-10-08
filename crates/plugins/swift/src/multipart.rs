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
                "kajiEncoded" | "partHeaders" | "maximumBodyBytes"
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
fn field_type(api: &Api, field: &Field) -> Result<String> {
    let base = match shape(api, &field.value)? {
        Shape::File => "PoolsterMultipartFile".into(),
        Shape::FileArray => "[PoolsterMultipartFile]".into(),
        _ => swift_type(&field.value, false),
    };
    Ok(format!("{base}{}", if field.required { "" } else { "?" }))
}
fn render_body(api: &Api, operation: &Operation) -> Result<String> {
    if ordered(api, operation) {
        return render_ordered_body(operation);
    }
    let name = dto_name(operation);
    let fields = fields(api, operation)?;
    let mut declarations = String::new();
    let mut arguments = Vec::new();
    let mut assignments = String::new();
    let mut parts = String::new();
    for field in fields {
        let native = identifier(&field.name);
        let ty = field_type(api, field)?;
        writeln!(declarations, "    public var {native}: {ty}")?;
        arguments.push(format!(
            "{native}: {ty}{}",
            if field.required { "" } else { " = nil" }
        ));
        writeln!(assignments, "        self.{native} = {native}")?;
        let kind = shape(api, &field.value)?;
        let encoding = encoding(operation, &field.name);
        let form = encoding.is_some_and(|value| {
            value.get("style").is_some()
                || value.get("explode").is_some()
                || value.get("allowReserved").is_some()
        });
        let content_type = if form {
            None
        } else {
            encoding
                .and_then(|value| value.get("contentType"))
                .and_then(Value::as_str)
        };
        let header_expr = format!("partHeaders[{:?}] ?? [:]", field.name);
        let required = encoding
            .and_then(|value| value.get("headers"))
            .and_then(Value::as_object)
            .map(|headers| {
                headers
                    .iter()
                    .filter(|(name, value)| {
                        !name.eq_ignore_ascii_case("Content-Type")
                            && value.get("required").and_then(Value::as_bool) == Some(true)
                    })
                    .map(|(name, _)| format!("{name:?}"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .join(", ");
        let checked_required = if matches!(kind, Shape::File | Shape::FileArray) {
            ""
        } else {
            required.as_str()
        };
        let header_expr = format!(
            "try PoolsterMultipartEncoder.headers({header_expr}, required: [{checked_required}])"
        );
        let default_type = match kind {
            Shape::Json | Shape::JsonArray => "application/json",
            _ => "text/plain",
        };
        let mime = content_type.unwrap_or(default_type);
        let prefix = if field.required {
            "        "
        } else {
            "            "
        };
        if !field.required {
            writeln!(parts, "        if let {native} {{")?;
        }
        let scalar_method = if json_media(mime) { "json" } else { "scalar" };
        let add = match kind {
            Shape::Scalar => format!(
                "try builder.{scalar_method}(name: {:?}, value: {native}, contentType: {mime:?}, headers: {header_expr})",
                field.name
            ),
            Shape::Json => format!(
                "try builder.json(name: {:?}, value: {native}, contentType: {mime:?}, headers: {header_expr})",
                field.name
            ),
            Shape::File => format!(
                "try builder.file(name: {:?}, value: {native}, contentType: {}, headers: {header_expr}, requiredHeaders: [{required}])",
                field.name,
                content_type
                    .map(|value| format!("{value:?}"))
                    .unwrap_or_else(|| "nil".into())
            ),
            Shape::ScalarArray
                if form
                    && encoding
                        .and_then(|value| value.get("explode"))
                        .and_then(Value::as_bool)
                        == Some(false) =>
            {
                format!(
                    "try builder.scalar(name: {:?}, value: try {native}.map(PoolsterMultipartEncoder.scalarString).joined(separator: \",\"), contentType: {mime:?}, headers: {header_expr})",
                    field.name
                )
            }
            Shape::ScalarArray => format!(
                "for value in {native} {{ try builder.{scalar_method}(name: {:?}, value: value, contentType: {mime:?}, headers: {header_expr}) }}",
                field.name
            ),
            Shape::JsonArray => format!(
                "for value in {native} {{ try builder.json(name: {:?}, value: value, contentType: {mime:?}, headers: {header_expr}) }}",
                field.name
            ),
            Shape::FileArray => format!(
                "for value in {native} {{ try builder.file(name: {:?}, value: value, contentType: {}, headers: {header_expr}, requiredHeaders: [{required}]) }}",
                field.name,
                content_type
                    .map(|value| format!("{value:?}"))
                    .unwrap_or_else(|| "nil".into())
            ),
        };
        writeln!(parts, "{prefix}{add}")?;
        if !field.required {
            writeln!(parts, "        }}")?;
        }
    }
    arguments.push("partHeaders: [String: [String: String]] = [:]".into());
    arguments.push(format!("maximumBodyBytes: Int = {MAX_BODY_BYTES}"));
    let known = fields
        .iter()
        .map(|field| format!("{:?}", field.name))
        .collect::<Vec<_>>()
        .join(", ");
    let mut source = format!(
        "{NOTICE}\nimport Foundation\n\npublic struct {name}: Sendable {{\n{declarations}    public var partHeaders: [String: [String: String]]\n    public var maximumBodyBytes: Int\n\n    public init({}) {{\n{assignments}        self.partHeaders = partHeaders\n        self.maximumBodyBytes = maximumBodyBytes\n    }}\n\n    internal func kajiEncoded() throws -> PoolsterEncodedMultipart {{\n        try Task.checkCancellation()\n        guard partHeaders.keys.allSatisfy({{ [{known}].contains($0) }}) else {{ throw PoolsterMultipartError.unknownPart }}\n        var builder = try PoolsterMultipartEncoder(maximumBodyBytes: maximumBodyBytes)\n{parts}        return try builder.finish()\n    }}\n}}\n",
        arguments.join(", ")
    );
    if mixed(operation) {
        let media = operation
            .request_body
            .as_ref()
            .unwrap()
            .media_types
            .iter()
            .find(|media| json_media(&media.content_type))
            .unwrap();
        let ty = media
            .schema
            .as_ref()
            .map(|schema| swift_type(schema, false))
            .unwrap_or_else(|| "JSONValue".into());
        let wrapper = body_type(operation).unwrap();
        source.push_str(&format!("\n/// Select the declared wire representation explicitly.\npublic enum {wrapper}: Sendable {{\n    case multipart({name})\n    case json({ty})\n\n    internal func kajiEncoded() throws -> PoolsterEncodedMultipart {{\n        try Task.checkCancellation()\n        switch self {{\n        case .multipart(let value): return try value.poolsterEncoded()\n        case .json(let value):\n            let body = try JSONEncoder().encode(value)\n            guard body.count <= {MAX_BODY_BYTES} else {{ throw PoolsterMultipartError.bodyTooLarge }}\n            return PoolsterEncodedMultipart(body: body, contentType: {:?})\n        }}\n    }}\n}}\n",media.content_type));
    }
    Ok(source)
}
fn render_ordered_body(operation: &Operation) -> Result<String> {
    let name = dto_name(operation);
    let media = operation
        .request_body
        .as_ref()
        .unwrap()
        .media_types
        .iter()
        .find(|media| {
            media
                .content_type
                .to_ascii_lowercase()
                .starts_with("multipart/")
        })
        .unwrap();
    let definition = poolster_core::openapi32::request_content(operation)?
        .into_iter()
        .find(|content| content.content_type == media.content_type)
        .unwrap_or_else(|| poolster_core::openapi32::ContentDefinition {
            content_type: media.content_type.clone(),
            ..Default::default()
        });
    let definition = serde_json::to_string(&definition)?;
    let mut source = format!(
        "{NOTICE}\nimport Foundation\n\npublic struct {name}: Sendable {{\n    public var parts: [PoolsterOrderedPart]\n    public var maximumBodyBytes: Int\n    public init(parts: [PoolsterOrderedPart], maximumBodyBytes: Int = {MAX_BODY_BYTES}) {{ self.parts = parts; self.maximumBodyBytes = maximumBodyBytes }}\n    internal func kajiEncoded() throws -> PoolsterEncodedMultipart {{\n        let definition = try JSONDecoder().decode(PoolsterOrderedDefinition.self, from: Data({definition:?}.utf8))\n        return try kajiOrderedEncode(parts: parts, contentType: definition.content_type, named: definition.encoding ?? [:], prefix: definition.prefix_encoding ?? [], item: definition.item_encoding, maximumBodyBytes: maximumBodyBytes)\n    }}\n}}\n"
    );
    if mixed(operation) {
        let json = operation
            .request_body
            .as_ref()
            .unwrap()
            .media_types
            .iter()
            .find(|media| json_media(&media.content_type))
            .unwrap();
        let ty = json
            .schema
            .as_ref()
            .map(|schema| swift_type(schema, false))
            .unwrap_or_else(|| "JSONValue".into());
        let wrapper = body_type(operation).unwrap();
        source.push_str(&format!("\npublic enum {wrapper}: Sendable {{\n    case multipart({name})\n    case json({ty})\n    internal func kajiEncoded() throws -> PoolsterEncodedMultipart {{\n        switch self {{\n        case .multipart(let value): return try value.poolsterEncoded()\n        case .json(let value):\n            try Task.checkCancellation()\n            let bytes = try JSONEncoder().encode(value)\n            guard bytes.count <= {MAX_BODY_BYTES} else {{ throw PoolsterMultipartError.bodyTooLarge }}\n            return PoolsterEncodedMultipart(body: bytes, contentType: {:?})\n        }}\n    }}\n}}\n",json.content_type));
    }
    Ok(source)
}
pub(crate) fn emit(api: &Api, root: &str, module: &str, tree: &mut GeneratedTree) -> Result<()> {
    if !api.operations.iter().any(selected) {
        return Ok(());
    }
    insert(
        tree,
        root,
        &format!("Sources/{module}/PoolsterMultipart.swift"),
        include_str!("multipart.swift.txt").into(),
    )?;
    for operation in api
        .operations
        .iter()
        .filter(|operation| selected(operation))
    {
        insert(
            tree,
            root,
            &format!(
                "Sources/{module}/{}MultipartBody.swift",
                type_name(&operation.id)
            ),
            render_body(api, operation)?,
        )?;
    }
    insert(
        tree,
        root,
        "MULTIPART.md",
        include_str!("multipart_readme.md").into(),
    )?;
    Ok(())
}
#[cfg(test)]
mod tests;
