use super::*;

pub(crate) fn render_operation(
    operation: &Operation,
    options: &ModelOptions,
    notice: &str,
) -> String {
    struct MediaResponse {
        content_type: String,
        suffix: String,
        value: String,
        description: Option<String>,
        doc_type: Option<String>,
    }

    struct ResponseType {
        status: String,
        name: String,
        media: Vec<MediaResponse>,
    }

    let identifier = type_identifier(&operation.id);
    let mut inline_enums = Vec::new();
    let mut inline_enum_names = BTreeSet::new();
    for response in &operation.responses {
        for media in &response.media_types {
            if let Some(schema) = &media.schema {
                collect_inline_enums(schema, options, &mut inline_enum_names, &mut inline_enums);
            }
        }
    }
    if let Some(body) = &operation.request_body {
        for media in &body.media_types {
            if let Some(schema) = &media.schema {
                collect_inline_enums(schema, options, &mut inline_enum_names, &mut inline_enums);
            }
        }
    }
    let parameter_groups = [
        ("path", "Path"),
        ("query", "Query"),
        ("querystring", "Querystring"),
        ("header", "Headers"),
        ("cookie", "Cookies"),
    ]
    .into_iter()
    .filter_map(|(location, suffix)| {
        // Poolster keeps the first occurrence of an exactly duplicated
        // `(in, name)` parameter, while distinct casing variants remain
        // separate OpenAPI properties.
        let mut names = BTreeSet::new();
        let parameters = operation
            .parameters
            .iter()
            .filter(|parameter| {
                parameter.location == location && names.insert(parameter.name.as_str())
            })
            .collect::<Vec<_>>();
        (!parameters.is_empty()).then_some((suffix, parameters))
    })
    .collect::<Vec<_>>();
    let response_types = operation
        .responses
        .iter()
        .map(|response| {
            let status = if response.status.chars().all(|c| c.is_ascii_digit()) {
                response.status.clone()
            } else {
                type_identifier(&response.status)
            };
            let name = format!("{identifier}Status{status}");
            let aliases = media_aliases(&response.media_types);
            let media = response
                .media_types
                .iter()
                .map(|media| MediaResponse {
                    content_type: media.content_type.clone(),
                    suffix: aliases[&media.content_type].clone(),
                    value: if media
                        .content_type
                        .to_ascii_lowercase()
                        .starts_with("multipart/")
                    {
                        "ArrayBuffer | Uint8Array".into()
                    } else {
                        media
                            .schema
                            .as_ref()
                            .map(|schema| render_operation_value(schema, options))
                            .unwrap_or_else(|| "void".into())
                    },
                    description: None,
                    doc_type: None,
                })
                .collect();
            ResponseType {
                status: response.status.clone(),
                name,
                media,
            }
        })
        .collect::<Vec<_>>();
    let body_media = operation
        .request_body
        .as_ref()
        .map(|body| {
            let aliases = media_aliases(&body.media_types);
            body.media_types
                .iter()
                .map(|media| MediaResponse {
                    content_type: media.content_type.clone(),
                    suffix: aliases[&media.content_type].clone(),
                    value: media
                        .schema
                        .as_ref()
                        .map(|schema| render_operation_value(schema, options))
                        .unwrap_or_else(|| "unknown".into()),
                    description: media
                        .schema
                        .as_ref()
                        .and_then(|schema| schema.description.clone()),
                    doc_type: media.schema.as_ref().map(schema_doc_type),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut source = String::from(notice);
    for inline_enum in inline_enums {
        source.push_str(&inline_enum);
        source.push('\n');
    }
    for (suffix, parameters) in &parameter_groups {
        let _ = writeln!(source, "export type {identifier}{suffix} = {{");
        for parameter in parameters {
            let optional = if parameter.required { "" } else { "?" };
            let value = parameter
                .schema
                .as_ref()
                .map(|schema| render_operation_value(schema, options))
                .unwrap_or_else(|| "unknown".into());
            let _ = writeln!(
                source,
                "  {}{optional}: {value}",
                property_name(&parameter.name)
            );
        }
        source.push_str("}\n\n");
    }
    for response in &response_types {
        if response.media.len() > 1 {
            let media_names = response
                .media
                .iter()
                .map(|media| {
                    let name = format!("{}{}", response.name, media.suffix);
                    let _ = writeln!(source, "export type {name} = {}\n", media.value);
                    name
                })
                .collect::<Vec<_>>();
            let _ = writeln!(
                source,
                "export type {} = {}\n",
                response.name,
                media_names.join(" | ")
            );
        } else {
            let value = response
                .media
                .first()
                .map(|media| media.value.as_str())
                .unwrap_or("void");
            let _ = writeln!(source, "export type {} = {value}\n", response.name);
        }
    }
    if body_media.len() > 1 {
        let media_names = body_media
            .iter()
            .map(|media| {
                let name = format!("{identifier}Body{}", media.suffix);
                let _ = writeln!(source, "export type {name} = {}\n", media.value);
                name
            })
            .collect::<Vec<_>>();
        let _ = writeln!(
            source,
            "export type {identifier}Body = {}\n",
            media_names.join(" | ")
        );
    } else if let Some(body) = body_media.first() {
        if let (Some(description), Some(doc_type)) = (&body.description, &body.doc_type) {
            let _ = writeln!(
                source,
                "/**\n * @description {description}\n * @type {doc_type}\n */"
            );
        }
        let _ = writeln!(source, "export type {identifier}Body = {}\n", body.value);
    }
    source.push_str(&format!("export type {identifier}Options = {{\n"));
    if !body_media.is_empty() {
        source.push_str(&format!("  body: {identifier}Body\n"));
    } else {
        source.push_str("  body?: never\n");
    }
    for (location, suffix, property) in [
        ("path", "Path", "path"),
        ("query", "Query", "query"),
        ("querystring", "Querystring", "querystring"),
        ("header", "Headers", "headers"),
        ("cookie", "Cookies", "cookies"),
    ] {
        if parameter_groups
            .iter()
            .any(|(group_suffix, _)| *group_suffix == suffix)
        {
            let required = operation
                .parameters
                .iter()
                .any(|parameter| parameter.location == location && parameter.required);
            let optional = if required { "" } else { "?" };
            source.push_str(&format!("  {property}{optional}: {identifier}{suffix}\n"));
        } else if location == "header" {
            source.push_str("  headers?: never\n");
        } else {
            source.push_str(&format!("  {location}?: never\n"));
        }
    }
    source.push_str("}\n\n");
    source.push_str(&format!("export type {identifier}Responses = {{\n"));
    for response in &response_types {
        if response.media.len() > 1 {
            let _ = writeln!(source, "  {}:", response_status_key(&response.status));
            for media in &response.media {
                let media_name = format!("{}{}", response.name, media.suffix);
                let _ = writeln!(
                    source,
                    "    | {{\n        contentType: {}\n        data: {media_name}\n      }}",
                    poolster_literal(&Value::String(media.content_type.clone()))
                );
            }
        } else {
            let _ = writeln!(
                source,
                "  {}: {}",
                response_status_key(&response.status),
                response.name
            );
        }
    }
    source.push_str("}\n\n/**\n * @description Union of all possible responses\n */\n");
    let response = response_types
        .iter()
        .map(|response| response.name.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    let response = if response.is_empty() {
        "never"
    } else {
        &response
    };
    source.push_str(&format!("export type {identifier}Response = {response}\n"));
    source
}

/// Inline OpenAPI schemas can retain Poolster's resolved AST name in a safe
/// extension. When present, Poolster emits their enum declaration before the
/// operation model and fields refer to that generated alias.
pub(crate) fn inline_enum_name(value: &SchemaValue) -> Option<&str> {
    poolster_core::poolster_extension(&value.extensions, "name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
}

pub(crate) fn collect_inline_enums(
    value: &SchemaValue,
    options: &ModelOptions,
    names: &mut BTreeSet<String>,
    declarations: &mut Vec<String>,
) {
    if !value.enum_values.is_empty()
        && let Some(name) = inline_enum_name(value)
        && names.insert(name.into())
    {
        let declaration = if crate::json::representation(value, options) != Int64Type::Number {
            render_schema(name, value, options, "")
        } else {
            match options.enum_type {
                EnumType::Literal => String::new(),
                EnumType::AsConst => render_as_const_enum(name, &value.enum_values, options, ""),
                EnumType::Enum => render_named_enum(name, &value.enum_values, options, ""),
                EnumType::ConstEnum => render_const_enum(name, &value.enum_values, options, ""),
            }
        };
        if !declaration.is_empty() {
            declarations.push(declaration);
        }
    }

    match &value.kind {
        SchemaKind::Array { items } => collect_inline_enums(items, options, names, declarations),
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                collect_inline_enums(&field.value, options, names, declarations);
            }
            if let AdditionalProperties::Schema { value } = additional_properties {
                collect_inline_enums(value, options, names, declarations);
            }
        }
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            for variant in variants {
                collect_inline_enums(variant, options, names, declarations);
            }
        }
        SchemaKind::Any
        | SchemaKind::Null
        | SchemaKind::Boolean
        | SchemaKind::Integer
        | SchemaKind::Number
        | SchemaKind::String
        | SchemaKind::Reference { .. }
        | SchemaKind::Not { .. } => {}
    }
}

pub(crate) fn schema_doc_type(schema: &SchemaValue) -> String {
    match &schema.kind {
        SchemaKind::Object { .. } => "object",
        SchemaKind::Array { .. } => "array",
        SchemaKind::String => "string",
        SchemaKind::Boolean => "boolean",
        SchemaKind::Integer | SchemaKind::Number => "number",
        SchemaKind::Null => "null",
        SchemaKind::Any | SchemaKind::Not { .. } => "unknown",
        SchemaKind::Reference { .. }
        | SchemaKind::OneOf { .. }
        | SchemaKind::AnyOf { .. }
        | SchemaKind::AllOf { .. } => "object",
    }
    .into()
}

pub(crate) fn response_status_key(status: &str) -> String {
    if status == "default" {
        "default".into()
    } else {
        poolster_literal(&Value::String(status.into()))
    }
}

pub(crate) fn render_operation_value(value: &SchemaValue, options: &ModelOptions) -> String {
    match &value.kind {
        SchemaKind::Object { fields, .. } if fields.is_empty() => "object".into(),
        SchemaKind::Object { fields, .. } => render_object_fields(fields, options),
        _ => render_value(value, options),
    }
}
