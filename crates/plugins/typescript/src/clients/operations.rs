use super::*;

pub(crate) fn generate_operations(
    api: &Api,
    config: &ClientRenderOptions,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<Vec<GeneratedFile>> {
    for operation in &api.operations {
        for requirement in &operation.security {
            for name in requirement.schemes.keys() {
                let scheme = security_schemes.and_then(|catalog| catalog.schemes.iter().find(|scheme| &scheme.name == name))
                    .ok_or_else(|| anyhow::anyhow!("operation {} references security scheme {name:?}, but its definition is missing", operation.id))?;
                match &scheme.kind {
                    SecuritySchemeKind::Other { .. } => {
                        bail!("unsupported security scheme {name:?}")
                    }
                    SecuritySchemeKind::ApiKey {
                        name: key,
                        location,
                    } if key.is_none() || location.is_none() => {
                        bail!("API key security scheme {name:?} requires a name and location")
                    }
                    SecuritySchemeKind::Http { scheme, .. } if scheme.is_none() => {
                        bail!("HTTP security scheme {name:?} requires a scheme")
                    }
                    _ => {}
                }
            }
        }
    }
    let output_dir = config.output_dir.trim_matches('/');
    let throw_on_error = config.throw_on_error;
    let group_by_tag = config.group_by_tag;

    let mut files = api.operations
        .iter()
        .map(|operation| {
            let module = operation_file_identifier(&operation.id);
            let group = grouped_directory(operation, config, group_by_tag);
            let path = if output_dir.is_empty() {
                match group {
                    Some(group) => format!("{group}/{module}.ts"),
                    None => format!("{module}.ts"),
                }
            } else {
                match group {
                    Some(group) => format!("{output_dir}/{group}/{module}.ts"),
                    None => format!("{output_dir}/{module}.ts"),
                }
            };
            let mut source = render_operation(operation, throw_on_error, security_schemes);
            if let Some(rule) = poolster_core::poolster_extension(&operation.annotations, "idempotency-resolved") {
                let header = serde_json::to_string(rule.get("header").and_then(Value::as_str).unwrap())?;
                let auto_generate = rule.get("auto_generate").and_then(Value::as_bool).unwrap_or(false);
                source = source.replace("  const { client: request = client, ...config } = options", &format!("  const {{ client: request = client, ...config }} = options\n  const idempotencyHeaders = poolsterIdempotencyHeaders(config.headers, {header}, {auto_generate})"));
                source = source.replace("      ...config,", &format!("      ...config,\n      headers: idempotencyHeaders,\n      idempotencyHeader: {header},"));
                source.push_str(include_str!("../../templates/idempotency_headers.ts.tmpl"));
            }

            if let Some(plan) = config
                .model_options
                .as_ref()
                .and_then(|options| crate::json::operation_inline_plan(api, operation, options))
            {
                let plan = format!("{{ lossless: {}, refs: poolsterJsonRefs, requests: {}, responses: {} }}",
                    serde_json::to_string(&plan["lossless"])?,
                    serde_json::to_string(&plan["requests"])?,
                    serde_json::to_string(&plan["responses"])?);
                let prefix = if grouped_directory(operation, config, group_by_tag).is_some() { "../" } else { "./" };
                source = format!("import {{ poolsterJsonRefs }} from '{prefix}_poolster_json_refs'\n{source}");
                source = source.replace(
                    "      ...config,",
                    &format!("      jsonPlan: {plan},\n      ...config,"),
                );
                if is_event_stream(operation) {
                    source = source.replace("    }),\n  )", &format!("    }}),\n    {plan},\n  )"));
                }
            }
            GeneratedFile::new(path, rewrite_import_paths(source, operation, config))
        })
        .collect::<Result<Vec<_>>>()?;
    if let Some(options) = &config.model_options {
        for (name, source) in crate::json::shared_refs(api, options)? {
            files.push(GeneratedFile::new(
                format!("{output_dir}/{name}.ts"),
                source,
            )?);
        }
    }
    Ok(files)
}

pub(crate) fn rewrite_import_paths(
    source: String,
    operation: &Operation,
    config: &ClientRenderOptions,
) -> String {
    if config.type_import_prefix.is_none() && config.runtime_import_prefix.is_none() {
        return source;
    }
    let type_prefix = config.type_import_prefix.as_deref().unwrap_or(".");
    let runtime_prefix = config.runtime_import_prefix.as_deref().unwrap_or(".");
    let runtime_dir = &config.runtime_dir;
    let group_by_tag = config.group_by_tag;
    let group = grouped_directory(operation, config, group_by_tag);
    let module = operation_model_file_identifier(&operation.id);
    let type_path = group
        .as_deref()
        .map(|group| format!("{type_prefix}/{group}/{module}"))
        .unwrap_or_else(|| format!("{type_prefix}/{module}"));
    let runtime_path = if group.is_some() {
        format!("../../{runtime_dir}/client")
    } else {
        format!("{runtime_prefix}/{runtime_dir}/client")
    };
    source
        .replace(
            &format!("from './{}'", pascal_identifier(&operation.id)),
            &format!("from '{type_path}'"),
        )
        .replace(
            "from './.poolster/client'",
            &format!("from '{runtime_path}'"),
        )
}

pub(crate) fn grouped_directory(
    operation: &Operation,
    config: &ClientRenderOptions,
    group_by_tag: bool,
) -> Option<String> {
    if !group_by_tag {
        return None;
    }
    operation_tag_group(operation)
        .or_else(|| config.group_default_directory.then(|| "default".into()))
}

pub(crate) fn operation_tag_group(operation: &Operation) -> Option<String> {
    operation
        .annotations
        .get("tags")
        .and_then(Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(Value::as_str)
        .map(lower_camel_identifier)
        .filter(|tag| !tag.is_empty())
}

pub(crate) fn render_operation(
    operation: &Operation,
    throw_on_error: bool,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> String {
    if is_event_stream(operation) {
        return render_event_stream_operation(operation, throw_on_error, security_schemes);
    }
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str().replace('\'', "\\'");

    // These concerns deliberately compose. Older generation selected the
    // first matching branch (form body *or* security *or* styles), silently
    // losing metadata when an operation used more than one OpenAPI feature.
    let mut metadata = String::new();
    if operation
        .success_schema()
        .is_some_and(|schema| schema.format.as_deref() == Some("binary"))
        && operation
            .responses
            .iter()
            .flat_map(|response| &response.media_types)
            .any(|media| {
                media
                    .content_type
                    .to_ascii_lowercase()
                    .starts_with("multipart/")
            })
    {
        metadata.push_str("      responseType: 'arraybuffer',\n");
    }
    if let Some(parameter) = operation
        .parameters
        .iter()
        .find(|p| p.location == "querystring")
    {
        let content_type = parameter
            .annotations
            .get("poolster.parameter_content")
            .and_then(Value::as_array)
            .and_then(|m| m.first())
            .and_then(|m| m.get("content_type"))
            .and_then(Value::as_str)
            .unwrap_or("application/x-www-form-urlencoded");
        metadata.push_str(&format!(
            "      wholeQuery: {{ name: {}, contentType: {} }},\n",
            serde_json::to_string(&parameter.name).expect("parameter name"),
            serde_json::to_string(content_type).expect("media type")
        ));
    }
    if let Ok(content) = poolster_core::openapi32::request_content(operation) {
        if let Some(media) = content.iter().find(|m| {
            !m.prefix_encoding.is_empty()
                || m.item_encoding.is_some()
                || m.encoding.values().any(|e| {
                    !e.encoding.is_empty()
                        || !e.prefix_encoding.is_empty()
                        || e.item_encoding.is_some()
                })
        }) {
            metadata.push_str(&format!(
                "      multipartPlan: {},\n",
                serde_json::to_string(media).expect("multipart plan")
            ));
        }
    }
    if let Some(content_type) = request_content_type(operation) {
        metadata.push_str(&format!(
            "      contentType: {{ request: '{content_type}' }},\n"
        ));
    }
    if let Some(styles) = render_parameter_styles(operation) {
        metadata.push_str(&format!("      styles: {styles},\n"));
    }
    if let Some(form_encodings) = render_form_encodings(operation) {
        metadata.push_str(&format!("      formEncodings: {form_encodings},\n"));
    }
    if let Some(security) = render_security(operation, security_schemes) {
        metadata.push_str(&format!("      security: {security},\n"));
    }
    format!(
        "{ESLINT_HEADER}import type {{ Options, RequestResult, ResponseResult }} from './.poolster/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, resolveResponse }} from './.poolster/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError>,\n): Promise<ResponseResult<RequestResult<{type_name}Responses, ThrowOnError>, ThrowOnError>> {{\n  const {{ client: request = client, ...config }} = options\n  const throwOnError = (config.throwOnError ?? {throw_on_error}) as ThrowOnError\n\n  return resolveResponse(\n    request({{\n      method: '{method}',\n      url: '{}',\n{metadata}      ...config,\n      throwOnError,\n    }}) as Promise<RequestResult<{type_name}Responses, ThrowOnError>>,\n    throwOnError,\n  )\n}}\n",
        operation.path,
    )
}
