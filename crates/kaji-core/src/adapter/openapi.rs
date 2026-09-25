//! Native OpenAPI 3.0/3.1 document adapter.
//!
//! Kaji deliberately keeps OpenAPI parsing at this boundary. Generators only
//! see the stable Rust AST, while applications can pass JSON or YAML documents
//! directly without a Go sidecar or a JavaScript runtime.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::{
    AdditionalProperties, Api, Discriminator, Field, HttpMethod, OAuthFlow, Operation,
    OperationMediaType, OperationParameter, OperationRequestBody, OperationResponse, Schema,
    SchemaKind, SchemaValue, SecurityRequirement, SecurityScheme, SecuritySchemeCatalog,
    SecuritySchemeKind,
};

/// A parsed OpenAPI document with the normalized API and reusable security
/// scheme catalog kept together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenApiDocument {
    pub api: Api,
    pub security_schemes: SecuritySchemeCatalog,
}

/// Parses an OpenAPI 3.0 or 3.1 JSON/YAML document from bytes.
pub fn parse_openapi(input: impl AsRef<[u8]>) -> Result<OpenApiDocument> {
    let bytes = input.as_ref();
    let value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => {
            serde_yaml::from_slice(bytes).context("OpenAPI input must be valid JSON or YAML")?
        }
    };
    parse_openapi_value(&value)
}

/// Reads and parses an OpenAPI JSON or YAML document.
pub fn parse_openapi_file(path: impl AsRef<Path>) -> Result<OpenApiDocument> {
    let path = path.as_ref();
    parse_openapi(
        fs::read(path).with_context(|| format!("read OpenAPI document {}", path.display()))?,
    )
}

/// Converts a deserialized OpenAPI 3.0/3.1 JSON value into Kaji's AST.
pub fn parse_openapi_value(root: &Value) -> Result<OpenApiDocument> {
    let document = object(root, "OpenAPI document")?;
    let version = required_string(document, "openapi", "OpenAPI document")?;
    if !version.starts_with('3') {
        bail!("Kaji supports OpenAPI 3.0 and 3.1 documents, found {version:?}")
    }
    let info = object(required(document, "info", "OpenAPI document")?, "info")?;
    let name = required_string(info, "title", "info")?.to_owned();
    let api_version = required_string(info, "version", "info")?.to_owned();
    let components = document.get("components").and_then(Value::as_object);
    let schemas = components
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .map(|schemas| {
            schemas
                .iter()
                .map(|(name, schema)| Ok(Schema::new(name, parse_schema(schema)?)))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();
    let security_schemes = parse_security_schemes(components)?;
    let inherited_security = parse_security(document.get("security"), "root security")?;
    let paths = object(required(document, "paths", "OpenAPI document")?, "paths")?;
    let mut operations = Vec::new();
    for (path, item) in paths {
        let item = resolve_component(item, components, "pathItems")?;
        let item = object(item, &format!("path item {path:?}"))?;
        let path_parameters = parse_parameters(item.get("parameters"), components, path)?;
        for (method_name, operation_value) in item {
            let Some(method) = parse_method(method_name) else {
                continue;
            };
            let operation = object(operation_value, &format!("operation {method_name} {path}"))?;
            let operation_parameters =
                parse_parameters(operation.get("parameters"), components, path)?;
            let parameters = merge_parameters(path_parameters.clone(), operation_parameters);
            let request_body = operation
                .get("requestBody")
                .map(|value| parse_request_body(value, components, path))
                .transpose()?;
            let responses = parse_responses(
                required(operation, "responses", "operation")?,
                components,
                path,
            )?;
            let security = if operation.contains_key("security") {
                parse_security(operation.get("security"), "operation security")?
            } else {
                inherited_security.clone()
            };
            let id = operation
                .get("operationId")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| generated_operation_id(method, path));
            operations.push(Operation {
                id,
                method,
                path: path.clone(),
                response_type: response_type(&responses),
                request_type: request_body.as_ref().and_then(request_type),
                parameters,
                request_body,
                responses,
                security,
                annotations: operation_annotations(operation),
            });
        }
    }
    Ok(OpenApiDocument {
        api: Api {
            name,
            version: api_version,
            schemas,
            operations,
            annotations: BTreeMap::new(),
        },
        security_schemes,
    })
}

fn parse_method(value: &str) -> Option<HttpMethod> {
    match value {
        "get" => Some(HttpMethod::Get),
        "post" => Some(HttpMethod::Post),
        "put" => Some(HttpMethod::Put),
        "patch" => Some(HttpMethod::Patch),
        "delete" => Some(HttpMethod::Delete),
        _ => None,
    }
}

fn generated_operation_id(method: HttpMethod, path: &str) -> String {
    let suffix = path
        .split('/')
        .filter(|part| !part.is_empty())
        .map(|part| part.trim_matches(|character| character == '{' || character == '}'))
        .collect::<Vec<_>>()
        .join("_");
    let prefix = match method {
        HttpMethod::Get => "get",
        HttpMethod::Post => "post",
        HttpMethod::Put => "put",
        HttpMethod::Patch => "patch",
        HttpMethod::Delete => "delete",
    };
    format!("{prefix}_{suffix}")
}

fn parse_parameters(
    value: Option<&Value>,
    components: Option<&serde_json::Map<String, Value>>,
    path: &str,
) -> Result<Vec<OperationParameter>> {
    let Some(values) = value else {
        return Ok(Vec::new());
    };
    let values = values
        .as_array()
        .with_context(|| format!("parameters for {path} must be an array"))?;
    values
        .iter()
        .map(|parameter| {
            let parameter = resolve_component(parameter, components, "parameters")?;
            let parameter = object(parameter, "parameter")?;
            let location = required_string(parameter, "in", "parameter")?.to_owned();
            if !matches!(location.as_str(), "path" | "query" | "header" | "cookie") {
                bail!("parameter location {location:?} is unsupported")
            }
            Ok(OperationParameter {
                name: required_string(parameter, "name", "parameter")?.to_owned(),
                location,
                required: parameter
                    .get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                schema: parameter.get("schema").map(parse_schema).transpose()?,
                description: string(parameter.get("description")),
                annotations: parameter_annotations(parameter),
            })
        })
        .collect()
}

fn merge_parameters(
    path: Vec<OperationParameter>,
    operation: Vec<OperationParameter>,
) -> Vec<OperationParameter> {
    let operation_keys = operation
        .iter()
        .map(|parameter| (parameter.name.clone(), parameter.location.clone()))
        .collect::<BTreeSet<_>>();
    path.into_iter()
        .filter(|parameter| {
            !operation_keys.contains(&(parameter.name.clone(), parameter.location.clone()))
        })
        .chain(operation)
        .collect()
}

fn parse_request_body(
    value: &Value,
    components: Option<&serde_json::Map<String, Value>>,
    path: &str,
) -> Result<OperationRequestBody> {
    let body = resolve_component(value, components, "requestBodies")?;
    let body = object(body, &format!("request body for {path}"))?;
    Ok(OperationRequestBody {
        required: body
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        description: string(body.get("description")),
        media_types: parse_content(body.get("content"), "request body content")?,
    })
}

fn parse_responses(
    value: &Value,
    components: Option<&serde_json::Map<String, Value>>,
    path: &str,
) -> Result<Vec<OperationResponse>> {
    let values = object(value, &format!("responses for {path}"))?;
    values
        .iter()
        .map(|(status, response)| {
            let response = resolve_component(response, components, "responses")?;
            let response = object(response, "response")?;
            Ok(OperationResponse {
                status: status.clone(),
                description: string(response.get("description")),
                media_types: parse_content(response.get("content"), "response content")?,
            })
        })
        .collect()
}

fn parse_content(value: Option<&Value>, context: &str) -> Result<Vec<OperationMediaType>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    object(value, context)?
        .iter()
        .map(|(content_type, media)| {
            let media = object(media, "media type")?;
            Ok(OperationMediaType {
                content_type: content_type.clone(),
                schema: media.get("schema").map(parse_schema).transpose()?,
            })
        })
        .collect()
}

fn parse_schema(value: &Value) -> Result<SchemaValue> {
    let schema = object(value, "schema")?;
    let kind = if let Some(reference) = string(schema.get("$ref")) {
        SchemaKind::Reference { reference }
    } else if let Some(values) = schema.get("oneOf").and_then(Value::as_array) {
        SchemaKind::OneOf {
            variants: values.iter().map(parse_schema).collect::<Result<_>>()?,
        }
    } else if let Some(values) = schema.get("anyOf").and_then(Value::as_array) {
        SchemaKind::AnyOf {
            variants: values.iter().map(parse_schema).collect::<Result<_>>()?,
        }
    } else if let Some(values) = schema.get("allOf").and_then(Value::as_array) {
        SchemaKind::AllOf {
            variants: values.iter().map(parse_schema).collect::<Result<_>>()?,
        }
    } else if let Some(value) = schema.get("not") {
        SchemaKind::Not {
            schema: Box::new(parse_schema(value)?),
        }
    } else if schema_type(schema) == Some("array") {
        SchemaKind::Array {
            items: Box::new(
                schema
                    .get("items")
                    .map(parse_schema)
                    .transpose()?
                    .unwrap_or_else(SchemaValue::unknown),
            ),
        }
    } else if schema_type(schema) == Some("object")
        || schema.contains_key("properties")
        || schema.contains_key("additionalProperties")
    {
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let fields = schema
            .get("properties")
            .and_then(Value::as_object)
            .map(|properties| {
                properties
                    .iter()
                    .map(|(name, value)| {
                        Ok::<Field, anyhow::Error>(Field {
                            name: name.clone(),
                            value: parse_schema(value)?,
                            required: required.contains(name.as_str()),
                            annotations: BTreeMap::new(),
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        let additional_properties = match schema.get("additionalProperties") {
            None => AdditionalProperties::Unspecified,
            Some(Value::Bool(true)) => AdditionalProperties::Any,
            Some(Value::Bool(false)) => AdditionalProperties::Forbidden,
            Some(value) => AdditionalProperties::Schema {
                value: Box::new(parse_schema(value)?),
            },
        };
        SchemaKind::Object {
            fields,
            additional_properties,
        }
    } else {
        match schema_type(schema) {
            Some("null") => SchemaKind::Null,
            Some("boolean") => SchemaKind::Boolean,
            Some("integer") => SchemaKind::Integer,
            Some("number") => SchemaKind::Number,
            Some("string") => SchemaKind::String,
            None => SchemaKind::Any,
            Some(other) => bail!("unsupported OpenAPI schema type {other:?}"),
        }
    };
    Ok(SchemaValue {
        kind,
        nullable: schema
            .get("nullable")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || schema
                .get("type")
                .and_then(Value::as_array)
                .is_some_and(|types| types.iter().any(|value| value.as_str() == Some("null"))),
        optional: false,
        nullish: false,
        format: string(schema.get("format")),
        enum_values: schema
            .get("enum")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        const_value: schema.get("const").cloned(),
        default: schema.get("default").cloned(),
        title: string(schema.get("title")),
        description: string(schema.get("description")),
        deprecated: schema
            .get("deprecated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        read_only: schema
            .get("readOnly")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        write_only: schema
            .get("writeOnly")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        discriminator: schema
            .get("discriminator")
            .map(parse_discriminator)
            .transpose()?,
        constraints: schema_constraints(schema),
        extensions: extensions(schema),
    })
}

fn parse_discriminator(value: &Value) -> Result<Discriminator> {
    let value = object(value, "discriminator")?;
    Ok(Discriminator {
        property_name: required_string(value, "propertyName", "discriminator")?.to_owned(),
        mapping: value
            .get("mapping")
            .and_then(Value::as_object)
            .map(|mapping| {
                mapping
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn parse_security_schemes(
    components: Option<&serde_json::Map<String, Value>>,
) -> Result<SecuritySchemeCatalog> {
    let Some(schemes) = components
        .and_then(|components| components.get("securitySchemes"))
        .and_then(Value::as_object)
    else {
        return Ok(SecuritySchemeCatalog::default());
    };
    Ok(SecuritySchemeCatalog {
        schemes: schemes
            .iter()
            .map(|(name, value)| {
                let value = object(value, "security scheme")?;
                let type_name = required_string(value, "type", "security scheme")?;
                let kind = match type_name {
                    "apiKey" => SecuritySchemeKind::ApiKey {
                        name: string(value.get("name")),
                        location: string(value.get("in")),
                    },
                    "http" => SecuritySchemeKind::Http {
                        scheme: string(value.get("scheme")),
                        bearer_format: string(value.get("bearerFormat")),
                    },
                    "oauth2" => SecuritySchemeKind::OAuth2 {
                        flows: parse_oauth_flows(value.get("flows"))?,
                        metadata_url: None,
                    },
                    "openIdConnect" => SecuritySchemeKind::OpenIdConnect {
                        discovery_url: string(value.get("openIdConnectUrl")),
                    },
                    other => SecuritySchemeKind::Other {
                        type_name: other.to_owned(),
                    },
                };
                Ok(SecurityScheme {
                    name: name.clone(),
                    description: string(value.get("description")),
                    kind,
                })
            })
            .collect::<Result<_>>()?,
    })
}

fn parse_oauth_flows(value: Option<&Value>) -> Result<Vec<OAuthFlow>> {
    let Some(flows) = value else {
        return Ok(Vec::new());
    };
    object(flows, "OAuth flows")?
        .iter()
        .map(|(flow_type, flow)| {
            let flow = object(flow, "OAuth flow")?;
            Ok(OAuthFlow {
                flow_type: flow_type.clone(),
                authorization_url: string(flow.get("authorizationUrl")),
                token_url: string(flow.get("tokenUrl")),
                refresh_url: string(flow.get("refreshUrl")),
                scopes: flow
                    .get("scopes")
                    .and_then(Value::as_object)
                    .map(|scopes| {
                        scopes
                            .iter()
                            .filter_map(|(scope, description)| {
                                description
                                    .as_str()
                                    .map(|description| (scope.clone(), description.to_owned()))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn parse_security(value: Option<&Value>, context: &str) -> Result<Vec<SecurityRequirement>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .with_context(|| format!("{context} must be an array"))?
        .iter()
        .map(|requirement| {
            Ok(SecurityRequirement {
                schemes: object(requirement, context)?
                    .iter()
                    .map(|(name, scopes)| {
                        Ok((
                            name.clone(),
                            scopes
                                .as_array()
                                .with_context(|| format!("scopes for {name} must be an array"))?
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect(),
                        ))
                    })
                    .collect::<Result<_>>()?,
            })
        })
        .collect()
}

fn response_type(responses: &[OperationResponse]) -> String {
    responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .or_else(|| responses.first())
        .and_then(|response| {
            response
                .media_types
                .iter()
                .find(|media| media.content_type.contains("json"))
                .or_else(|| response.media_types.first())
        })
        .and_then(|media| media.schema.as_ref())
        .map(schema_name)
        .unwrap_or_else(|| "void".into())
}
fn request_type(body: &OperationRequestBody) -> Option<String> {
    body.media_types
        .iter()
        .find(|media| media.content_type.contains("json"))
        .or_else(|| body.media_types.first())
        .and_then(|media| media.schema.as_ref())
        .map(schema_name)
}
fn schema_name(schema: &SchemaValue) -> String {
    match &schema.kind {
        SchemaKind::Reference { reference } => {
            reference.rsplit('/').next().unwrap_or(reference).to_owned()
        }
        SchemaKind::Array { items } => format!("{}[]", schema_name(items)),
        SchemaKind::String => "string".into(),
        SchemaKind::Integer | SchemaKind::Number => "number".into(),
        SchemaKind::Boolean => "boolean".into(),
        SchemaKind::Object { .. } => "object".into(),
        _ => "unknown".into(),
    }
}
fn operation_annotations(operation: &serde_json::Map<String, Value>) -> BTreeMap<String, Value> {
    let mut annotations = extensions(operation);
    for key in ["summary", "description"] {
        if let Some(value) = operation.get(key) {
            annotations.insert(key.into(), value.clone());
        }
    }
    if operation.get("deprecated").and_then(Value::as_bool) == Some(true) {
        annotations.insert("deprecated".into(), Value::Bool(true));
    }
    if let Some(tags) = operation.get("tags") {
        annotations.insert("tags".into(), tags.clone());
    }
    annotations
}
fn parameter_annotations(parameter: &serde_json::Map<String, Value>) -> BTreeMap<String, Value> {
    let mut annotations = extensions(parameter);
    for key in ["style", "explode"] {
        if let Some(value) = parameter.get(key) {
            annotations.insert(key.into(), value.clone());
        }
    }
    annotations
}
fn extensions(object: &serde_json::Map<String, Value>) -> BTreeMap<String, Value> {
    object
        .iter()
        .filter(|(key, _)| key.starts_with("x-"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}
fn schema_constraints(schema: &serde_json::Map<String, Value>) -> BTreeMap<String, Value> {
    const KEYS: &[&str] = &[
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "minLength",
        "maxLength",
        "pattern",
        "minItems",
        "maxItems",
        "uniqueItems",
        "minProperties",
        "maxProperties",
    ];
    schema
        .iter()
        .filter(|(key, _)| KEYS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}
fn schema_type(schema: &serde_json::Map<String, Value>) -> Option<&str> {
    schema.get("type").and_then(Value::as_str).or_else(|| {
        schema
            .get("type")
            .and_then(Value::as_array)
            .and_then(|types| {
                types
                    .iter()
                    .find_map(|value| value.as_str().filter(|value| *value != "null"))
            })
    })
}
fn resolve_component<'a>(
    value: &'a Value,
    components: Option<&'a serde_json::Map<String, Value>>,
    kind: &str,
) -> Result<&'a Value> {
    let Some(reference) = value.get("$ref").and_then(Value::as_str) else {
        return Ok(value);
    };
    let prefix = format!("#/components/{kind}/");
    let name = reference.strip_prefix(&prefix).with_context(|| {
        format!("only local {prefix} references are supported, found {reference:?}")
    })?;
    components
        .and_then(|components| components.get(kind))
        .and_then(Value::as_object)
        .and_then(|values| values.get(name))
        .with_context(|| format!("OpenAPI reference {reference:?} was not found"))
}
fn object<'a>(value: &'a Value, context: &str) -> Result<&'a serde_json::Map<String, Value>> {
    value
        .as_object()
        .with_context(|| format!("{context} must be an object"))
}
fn required<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a Value> {
    object
        .get(key)
        .with_context(|| format!("{context} requires {key}"))
}
fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a str> {
    required(object, key, context)?
        .as_str()
        .with_context(|| format!("{context}.{key} must be a string"))
}
fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_and_yaml_without_a_sidecar() {
        let json = serde_json::json!({
            "openapi": "3.1.0",
            "info": { "title": "Email API", "version": "1.2.3" },
            "components": {
                "schemas": { "Contact": { "type": "object", "required": ["id"], "properties": { "id": { "type": "string" } } } },
                "securitySchemes": { "Bearer": { "type": "http", "scheme": "bearer" } }
            },
            "security": [{ "Bearer": [] }],
            "paths": {
                "/contacts/{id}": {
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "get": {
                        "operationId": "getContact",
                        "responses": {
                            "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Contact" } } } },
                            "404": { "description": "missing" }
                        },
                        "x-kaji-mock": { "scenarios": [] }
                    }
                }
            }
        });
        let document = parse_openapi(serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(document.api.name, "Email API");
        assert_eq!(document.api.operations[0].id, "getContact");
        assert_eq!(
            document.api.operations[0].security[0].schemes["Bearer"],
            Vec::<String>::new()
        );
        assert!(
            document.api.operations[0]
                .annotations
                .contains_key("x-kaji-mock")
        );
        assert_eq!(document.security_schemes.schemes.len(), 1);
        let yaml = b"openapi: 3.0.3\ninfo:\n  title: YAML API\n  version: 1.0.0\npaths: {}\n";
        assert_eq!(parse_openapi(yaml).unwrap().api.name, "YAML API");
    }
}
