//! Structured TypeScript Fetch and Axios operation-client generators.
//!
//! Fetch and Axios operation files share a consistent public source shape;
//! transport-specific setup lives in `.kaji/client`.

use crate::models::operation_model_file_identifier;
use anyhow::{Result, bail};
use serde_json::Value;

use kaji_core::GeneratedFile;
use kaji_core::ast::{Api, Operation, SecuritySchemeCatalog, SecuritySchemeKind};

const ESLINT_HEADER: &str = "/* eslint-disable no-alert, no-console */\n\n";

/// Per-operation rendering options used by the typed SDK generator.
pub(crate) struct ClientRenderOptions {
    pub output_dir: String,
    pub model_options: Option<crate::ModelOptions>,
    pub throw_on_error: bool,
    pub group_by_tag: bool,
    pub group_default_directory: bool,
    pub type_import_prefix: Option<String>,
    pub runtime_import_prefix: Option<String>,
    pub runtime_dir: String,
}

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

    api.operations
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
            if let Some(rule) = operation.annotations.get("x-kaji-idempotency-resolved") {
                let header = serde_json::to_string(rule.get("header").and_then(Value::as_str).unwrap())?;
                let auto_generate = rule.get("auto_generate").and_then(Value::as_bool).unwrap_or(false);
                source = source.replace("  const { client: request = client, ...config } = options", &format!("  const {{ client: request = client, ...config }} = options\n  const idempotencyHeaders = kajiIdempotencyHeaders(config.headers, {header}, {auto_generate})"));
                source = source.replace("      ...config,", &format!("      ...config,\n      headers: idempotencyHeaders,\n      idempotencyHeader: {header},"));
                source.push_str(include_str!("idempotency_headers.ts.txt"));
            }

            if let Some(plan) = config
                .model_options
                .as_ref()
                .and_then(|options| crate::json::operation_plan(api, operation, options))
            {
                let plan = serde_json::to_string(&plan)?;
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
        .collect()
}

fn rewrite_import_paths(
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
        .replace("from './.kaji/client'", &format!("from '{runtime_path}'"))
}

fn grouped_directory(
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

fn operation_tag_group(operation: &Operation) -> Option<String> {
    operation
        .annotations
        .get("tags")
        .and_then(Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(Value::as_str)
        .map(lower_camel_identifier)
        .filter(|tag| !tag.is_empty())
}

fn render_operation(
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
            .get("kaji.parameter_content")
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
    if let Ok(content) = kaji_core::openapi32::request_content(operation) {
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
        "{ESLINT_HEADER}import type {{ Options, RequestResult, ResponseResult }} from './.kaji/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, resolveResponse }} from './.kaji/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError>,\n): Promise<ResponseResult<RequestResult<{type_name}Responses, ThrowOnError>, ThrowOnError>> {{\n  const {{ client: request = client, ...config }} = options\n  const throwOnError = (config.throwOnError ?? {throw_on_error}) as ThrowOnError\n\n  return resolveResponse(\n    request({{\n      method: '{method}',\n      url: '{}',\n{metadata}      ...config,\n      throwOnError,\n    }}) as Promise<RequestResult<{type_name}Responses, ThrowOnError>>,\n    throwOnError,\n  )\n}}\n",
        operation.path,
    )
}

/// Preserves explicit OpenAPI parameter serialization metadata for Kaji's
/// runtime. Unspecified style/explode settings intentionally remain absent so
/// the runtime can apply each location's OpenAPI defaults.
fn style_property_name(name: &str) -> String {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if valid {
        name.to_owned()
    } else {
        serde_json::to_string(name).expect("string serialization")
    }
}

fn render_parameter_styles(operation: &Operation) -> Option<String> {
    let groups = ["path", "query", "header", "cookie"]
        .into_iter()
        .filter_map(|location| {
            let parameters = operation
                .parameters
                .iter()
                .filter(|parameter| parameter.location == location)
                .filter_map(|parameter| {
                    let style = parameter.annotations.get("style").and_then(Value::as_str);
                    let explode = parameter
                        .annotations
                        .get("explode")
                        .and_then(Value::as_bool);
                    let content_type = kaji_core::openapi32::parameter_content(parameter)
                        .ok()
                        .and_then(|content| {
                            content.first().map(|media| media.content_type.clone())
                        });
                    (style.is_some() || explode.is_some() || content_type.is_some()).then(|| {
                        let mut fields = Vec::new();
                        if let Some(content_type) = &content_type {
                            fields.push(format!(
                                "contentType: {}",
                                serde_json::to_string(content_type).expect("media type")
                            ));
                        }
                        if let Some(style) = style {
                            fields.push(format!("style: '{style}'"));
                        }
                        if let Some(explode) = explode {
                            fields.push(format!("explode: {explode}"));
                        }
                        format!(
                            "{}: {{ {} }}",
                            style_property_name(&parameter.name),
                            fields.join(", ")
                        )
                    })
                })
                .collect::<Vec<_>>();
            (!parameters.is_empty()).then(|| format!("{location}: {{ {} }}", parameters.join(", ")))
        })
        .collect::<Vec<_>>();
    (!groups.is_empty()).then(|| format!("{{ {} }}", groups.join(", ")))
}

/// Carries OpenAPI's multipart/urlencoded Encoding Object to the transport.
/// The compiler stores it as an annotation so non-TypeScript targets do not
/// need a JavaScript-shaped form-data type in the shared AST.
fn render_form_encodings(operation: &Operation) -> Option<String> {
    let mut encodings = operation
        .annotations
        .get("kaji.request_body_encodings")?
        .clone();
    if let Some(media_types) = encodings.as_object_mut() {
        for fields in media_types.values_mut().filter_map(Value::as_object_mut) {
            for encoding in fields.values_mut().filter_map(Value::as_object_mut) {
                for key in ["style", "explode", "contentType", "allowReserved"] {
                    if encoding.get(key).is_some_and(Value::is_null) {
                        encoding.remove(key);
                    }
                }
            }
        }
    }
    serde_json::to_string(&encodings).ok()
}

/// Converts declared OpenAPI OR-of-AND requirements without guessing scheme kinds.
fn render_security(
    operation: &Operation,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Option<String> {
    (!operation.security.is_empty()).then(|| {
        let alternatives = operation
            .security
            .iter()
            .map(|requirement| {
                let schemes = requirement
                    .schemes
                    .iter()
                    .map(|(name, scopes)| {
                        render_catalog_security_scheme(name, scopes, security_schemes)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{schemes}]")
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("[{alternatives}]")
    })
}

/// Emits runtime-addressable descriptors for SDK packages. `id` is the
/// OpenAPI component key, so one client can carry several independently named
/// API keys, bearer tokens, and OAuth credentials without guessing headers.
fn render_catalog_security_scheme(
    scheme_name: &str,
    scopes: &[String],
    catalog: Option<&SecuritySchemeCatalog>,
) -> String {
    let descriptor = render_security_scheme(scheme_name, catalog);
    let scopes = if scopes.is_empty() {
        String::new()
    } else {
        format!(
            ", scopes: {}",
            serde_json::to_string(scopes).unwrap_or_else(|_| "[]".into())
        )
    };
    descriptor
        .replacen("{ ", &format!("{{ id: '{scheme_name}', "), 1)
        .replacen(" }", &format!("{scopes} }}"), 1)
}

fn render_security_scheme(scheme_name: &str, catalog: Option<&SecuritySchemeCatalog>) -> String {
    let scheme = catalog
        .and_then(|catalog| {
            catalog
                .schemes
                .iter()
                .find(|scheme| scheme.name == scheme_name)
        })
        .expect("security catalog validated before rendering");

    match &scheme.kind {
        SecuritySchemeKind::ApiKey { name, location } => {
            let name = name.as_deref().expect("validated API key name");
            let location = location.as_deref().expect("validated API key location");
            format!("{{ type: 'apiKey', name: '{name}', in: '{location}' }}")
        }
        SecuritySchemeKind::Http { scheme, .. } => {
            let scheme = scheme.as_deref().expect("validated HTTP scheme");
            format!("{{ type: 'http', scheme: '{scheme}' }}")
        }
        SecuritySchemeKind::OAuth2 { .. } | SecuritySchemeKind::OpenIdConnect { .. } => {
            "{ type: 'oauth2' }".to_owned()
        }
        SecuritySchemeKind::Other { .. } => {
            unreachable!("unsupported security scheme rejected before rendering")
        }
    }
}

/// Kaji routes Server-Sent Events through the event-stream client helper,
/// selected from the response media type rather than an operation-name rule.
fn render_event_stream_operation(
    operation: &Operation,
    throw_on_error: bool,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> String {
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str().replace('\'', "\\'");
    let mut metadata = String::new();
    if let Some(parameter) = operation
        .parameters
        .iter()
        .find(|p| p.location == "querystring")
    {
        let content_type = parameter
            .annotations
            .get("kaji.parameter_content")
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
    if let Ok(content) = kaji_core::openapi32::request_content(operation) {
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
    let options_default = if operation
        .parameters
        .iter()
        .any(|parameter| parameter.required)
        || operation.request_body.is_some()
    {
        ""
    } else {
        " = {}"
    };
    format!(
        "{ESLINT_HEADER}import type {{ Options, EventStreamResult, SuccessOf }} from './.kaji/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, toEventStream }} from './.kaji/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError>{options_default},\n): Promise<EventStreamResult<SuccessOf<{type_name}Responses>>> {{\n  const {{ client: request = client, ...config }} = options\n\n  return toEventStream<SuccessOf<{type_name}Responses>>(\n    request({{\n      method: '{method}',\n      url: '{}',\n      responseType: 'stream',\n{metadata}      ...config,\n      throwOnError: config.throwOnError ?? {throw_on_error},\n    }}),\n  )\n}}\n",
        operation.path,
    )
}

fn request_content_type(operation: &Operation) -> Option<&str> {
    operation
        .request_body
        .as_ref()?
        .media_types
        .iter()
        .find_map(|media_type| {
            (media_type.content_type.starts_with("multipart/")
                || matches!(
                    media_type.content_type.as_str(),
                    "application/x-www-form-urlencoded"
                        | "application/json-seq"
                        | "application/x-ndjson"
                        | "application/ndjson"
                        | "application/jsonl"
                ))
            .then_some(media_type.content_type.as_str())
        })
}

fn is_event_stream(operation: &Operation) -> bool {
    operation.responses.iter().any(|response| {
        response
            .media_types
            .iter()
            .any(|media_type| media_type.content_type == "text/event-stream")
    })
}

fn pascal_identifier(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase = true;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if uppercase {
                output.extend(character.to_uppercase());
            } else {
                output.push(character);
            }
            uppercase = false;
        } else {
            uppercase = true;
        }
    }
    if output.is_empty() {
        "Operation".into()
    } else {
        output
    }
}

fn lower_camel_identifier(value: &str) -> String {
    let pascal = pascal_identifier(value);
    let mut characters = pascal.chars();
    match characters.next() {
        Some(first) => first.to_lowercase().collect::<String>() + characters.as_str(),
        None => "operation".into(),
    }
}

/// Stable, filesystem-safe module name for an operation. Public function names
/// stay fully descriptive; only an excessively long filename is compacted.
pub(crate) fn operation_file_identifier(value: &str) -> String {
    let identifier = lower_camel_identifier(value);
    const MAX_PREFIX_CHARS: usize = 96;
    if identifier.chars().count() <= MAX_PREFIX_CHARS {
        return identifier;
    }
    let prefix = identifier
        .chars()
        .take(MAX_PREFIX_CHARS)
        .collect::<String>();
    format!("{prefix}_{:016x}", stable_hash(value))
}

fn stable_hash(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::ast::{HttpMethod, SecurityRequirement, SecurityScheme, SecuritySchemeKind};

    fn operation(id: &str, method: HttpMethod, path: &str) -> Operation {
        Operation {
            id: id.into(),
            method,
            path: path.into(),
            responses: vec![kaji_core::OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![kaji_core::OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(kaji_core::SchemaValue::reference(
                        "#/components/schemas/Pet",
                    )),
                }],
            }],
            annotations: Default::default(),
            ..Default::default()
        }
    }

    #[test]
    fn renders_kaji_path_parameter_docs_and_source() {
        let source = render_operation(
            &operation("getPetById", HttpMethod::Get, "/pet/{petId}"),
            true,
            None,
        );
        assert!(source.contains("{@link /pet/:petId}"));
        assert!(source.contains("method: 'GET'"));
        assert!(source.contains("GetPetByIdOptions"));
    }

    #[test]
    fn security_catalog_preserves_http_api_key_locations_and_oauth() {
        let mut operation = operation("getSecure", HttpMethod::Get, "/secure");
        operation.security = ["bearer", "header_key", "query_key", "oauth"]
            .into_iter()
            .map(|name| SecurityRequirement {
                schemes: [(name.into(), Vec::new())].into_iter().collect(),
            })
            .collect();
        let catalog = SecuritySchemeCatalog {
            schemes: vec![
                SecurityScheme {
                    name: "bearer".into(),
                    description: None,
                    kind: SecuritySchemeKind::Http {
                        scheme: Some("bearer".into()),
                        bearer_format: Some("JWT".into()),
                    },
                },
                SecurityScheme {
                    name: "header_key".into(),
                    description: None,
                    kind: SecuritySchemeKind::ApiKey {
                        name: Some("X-API-Key".into()),
                        location: Some("header".into()),
                    },
                },
                SecurityScheme {
                    name: "query_key".into(),
                    description: None,
                    kind: SecuritySchemeKind::ApiKey {
                        name: Some("api_key".into()),
                        location: Some("query".into()),
                    },
                },
                SecurityScheme {
                    name: "oauth".into(),
                    description: None,
                    kind: SecuritySchemeKind::OAuth2 {
                        flows: Vec::new(),
                        metadata_url: None,
                    },
                },
            ],
        };

        let source = render_operation(&operation, true, Some(&catalog));
        // SDK generation keeps OpenAPI's OR-of-AND alternatives rather than
        // flattening every scheme into one ambiguous list. The component key
        // is retained as the runtime credential lookup key.
        assert!(source.contains("security: [[{ id: 'bearer', type: 'http', scheme: 'bearer'"));
        assert!(
            source.contains("[{ id: 'header_key', type: 'apiKey', name: 'X-API-Key', in: 'header'")
        );
        assert!(
            source.contains("[{ id: 'query_key', type: 'apiKey', name: 'api_key', in: 'query'")
        );
        assert!(source.contains("id: 'oauth', type: 'oauth2'"));
    }

    #[test]
    fn encoding_omission_and_required_sse_options_match_native_types() {
        let mut operation = operation("streamProbe", HttpMethod::Get, "/probe/{id}");
        operation.parameters.push(kaji_core::OperationParameter {
            name: "id".into(),
            location: "path".into(),
            required: true,
            schema: None,
            description: None,
            annotations: Default::default(),
        });
        operation.annotations.insert("kaji.request_body_encodings".into(), serde_json::json!({"multipart/form-data": {"payload": {"style":null,"explode":null,"contentType":null,"allowReserved":null}}}));
        assert_eq!(
            render_form_encodings(&operation).unwrap(),
            "{\"multipart/form-data\":{\"payload\":{}}}"
        );
        let source = render_event_stream_operation(&operation, false, None);
        assert!(source.contains("options: Options<StreamProbeOptions, ThrowOnError>,"));
        assert!(!source.contains("Options<StreamProbeOptions, ThrowOnError> = {}"));
        operation.parameters.clear();
        let source = render_event_stream_operation(&operation, false, None);
        assert!(source.contains("Options<StreamProbeOptions, ThrowOnError> = {}"));
    }

    #[test]
    fn style_metadata_preserves_unsafe_wire_keys_as_string_properties() {
        let mut operation = operation("headerProbe", HttpMethod::Get, "/probe");
        for name in ["openai-beta", "x'quoted", "1st", "雪"] {
            operation.parameters.push(kaji_core::OperationParameter {
                name: name.into(),
                location: "header".into(),
                required: false,
                schema: None,
                description: None,
                annotations: std::collections::BTreeMap::from([(
                    "style".into(),
                    serde_json::json!("simple"),
                )]),
            });
        }
        let styles = render_parameter_styles(&operation).unwrap();
        for name in ["openai-beta", "x'quoted", "1st", "雪"] {
            assert!(styles.contains(&format!(
                "{}: {{ style: 'simple' }}",
                serde_json::to_string(name).unwrap()
            )));
        }
        assert_eq!(style_property_name("petId"), "petId");
    }

    #[test]
    fn operation_composes_media_styles_and_security_metadata() {
        let mut operation = operation("updatePet", HttpMethod::Post, "/pets/{petId}");
        operation.parameters.push(kaji_core::OperationParameter {
            name: "petId".into(),
            location: "path".into(),
            required: true,
            schema: None,
            description: None,
            annotations: std::collections::BTreeMap::from([(
                "style".into(),
                serde_json::json!("matrix"),
            )]),
        });
        operation.request_body = Some(kaji_core::OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![kaji_core::OperationMediaType {
                content_type: "multipart/form-data".into(),
                schema: None,
            }],
        });
        operation.annotations.insert(
            "kaji.request_body_encodings".into(),
            serde_json::json!({
                "multipart/form-data": {
                    "metadata": {
                        "contentType": "application/json",
                        "explode": false,
                        "headers": {
                            "X-Part-Id": {
                                "required": true,
                                "style": "simple",
                                "schema_definition": { "type": "string" }
                            }
                        }
                    }
                }
            }),
        );
        operation.security = vec![SecurityRequirement {
            schemes: [("cookie".into(), Vec::new())].into_iter().collect(),
        }];
        let catalog = SecuritySchemeCatalog {
            schemes: vec![SecurityScheme {
                name: "cookie".into(),
                description: None,
                kind: SecuritySchemeKind::ApiKey {
                    name: Some("session".into()),
                    location: Some("cookie".into()),
                },
            }],
        };
        let source = render_operation(&operation, true, Some(&catalog));
        assert!(source.contains("contentType: { request: 'multipart/form-data' }"));
        assert!(source.contains("formEncodings: {\"multipart/form-data\":{\"metadata\":{\"contentType\":\"application/json\",\"explode\":false,\"headers\":{\"X-Part-Id\":{\"required\":true,\"schema_definition\":{\"type\":\"string\"},\"style\":\"simple\"}}}}}"));
        assert!(source.contains("styles: { path: { petId: { style: 'matrix' } } }"));
        assert!(source.contains(
            "security: [[{ id: 'cookie', type: 'apiKey', name: 'session', in: 'cookie' }]]"
        ));
    }
}
