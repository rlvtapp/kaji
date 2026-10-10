use super::*;
pub(super) fn field_type(api: &Api, field: &Field) -> Result<String> {
    let base = match shape(api, &field.value)? {
        Shape::File => "PoolsterMultipartFile".into(),
        Shape::FileArray => "[PoolsterMultipartFile]".into(),
        _ => swift_type(&field.value, false),
    };
    Ok(format!("{base}{}", if field.required { "" } else { "?" }))
}
pub(super) fn render_body(api: &Api, operation: &Operation) -> Result<String> {
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
        "{NOTICE}\nimport Foundation\n\npublic struct {name}: Sendable {{\n{declarations}    public var partHeaders: [String: [String: String]]\n    public var maximumBodyBytes: Int\n\n    public init({}) {{\n{assignments}        self.partHeaders = partHeaders\n        self.maximumBodyBytes = maximumBodyBytes\n    }}\n\n    internal func poolsterEncoded() throws -> PoolsterEncodedMultipart {{\n        try Task.checkCancellation()\n        guard partHeaders.keys.allSatisfy({{ [{known}].contains($0) }}) else {{ throw PoolsterMultipartError.unknownPart }}\n        var builder = try PoolsterMultipartEncoder(maximumBodyBytes: maximumBodyBytes)\n{parts}        return try builder.finish()\n    }}\n}}\n",
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
        source.push_str(&format!("\n/// Select the declared wire representation explicitly.\npublic enum {wrapper}: Sendable {{\n    case multipart({name})\n    case json({ty})\n\n    internal func poolsterEncoded() throws -> PoolsterEncodedMultipart {{\n        try Task.checkCancellation()\n        switch self {{\n        case .multipart(let value): return try value.poolsterEncoded()\n        case .json(let value):\n            let body = try JSONEncoder().encode(value)\n            guard body.count <= {MAX_BODY_BYTES} else {{ throw PoolsterMultipartError.bodyTooLarge }}\n            return PoolsterEncodedMultipart(body: body, contentType: {:?})\n        }}\n    }}\n}}\n",media.content_type));
    }
    Ok(source)
}
pub(super) fn render_ordered_body(operation: &Operation) -> Result<String> {
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
        "{NOTICE}\nimport Foundation\n\npublic struct {name}: Sendable {{\n    public var parts: [PoolsterOrderedPart]\n    public var maximumBodyBytes: Int\n    public init(parts: [PoolsterOrderedPart], maximumBodyBytes: Int = {MAX_BODY_BYTES}) {{ self.parts = parts; self.maximumBodyBytes = maximumBodyBytes }}\n    internal func poolsterEncoded() throws -> PoolsterEncodedMultipart {{\n        let definition = try JSONDecoder().decode(PoolsterOrderedDefinition.self, from: Data({definition:?}.utf8))\n        return try poolsterOrderedEncode(parts: parts, contentType: definition.content_type, named: definition.encoding ?? [:], prefix: definition.prefix_encoding ?? [], item: definition.item_encoding, maximumBodyBytes: maximumBodyBytes)\n    }}\n}}\n"
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
        source.push_str(&format!("\npublic enum {wrapper}: Sendable {{\n    case multipart({name})\n    case json({ty})\n    internal func poolsterEncoded() throws -> PoolsterEncodedMultipart {{\n        switch self {{\n        case .multipart(let value): return try value.poolsterEncoded()\n        case .json(let value):\n            try Task.checkCancellation()\n            let bytes = try JSONEncoder().encode(value)\n            guard bytes.count <= {MAX_BODY_BYTES} else {{ throw PoolsterMultipartError.bodyTooLarge }}\n            return PoolsterEncodedMultipart(body: bytes, contentType: {:?})\n        }}\n    }}\n}}\n",json.content_type));
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
        include_str!("../../templates/multipart.swift.tmpl").into(),
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
        include_str!("../../templates/multipart_readme.md.tmpl").into(),
    )?;
    Ok(())
}
