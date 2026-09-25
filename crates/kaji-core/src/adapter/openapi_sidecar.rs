//! Adapter for the JSON emitted by `docs-compiler/openapi`.
//!
//! The Go helper remains the temporary OpenAPI parser. This module is the
//! narrow boundary that turns its JSON-compatible schema definitions into the
//! Rust-native, target-neutral AST used by every generator.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::ast::{
    AdditionalProperties, Discriminator, OAuthFlow, OperationMediaType, OperationParameter,
    OperationRequestBody, OperationResponse, SchemaKind, SchemaValue, SecurityRequirement,
    SecurityScheme, SecuritySchemeCatalog, SecuritySchemeKind,
};
use crate::{Api, Field, HttpMethod, Operation, Schema};

#[derive(Debug, Deserialize)]
struct SidecarOperation {
    path: String,
    method: String,
    #[serde(default)]
    operation_id: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    deprecated: bool,
    #[serde(default)]
    extensions: BTreeMap<String, Value>,
    #[serde(default)]
    parameters: Vec<SidecarParameter>,
    #[serde(default)]
    request_body: Option<SidecarBody>,
    #[serde(default)]
    responses: Vec<SidecarResponse>,
    #[serde(default)]
    request_examples: Vec<SidecarExample>,
    #[serde(default)]
    security_requirements: Vec<SidecarSecurityRequirement>,
}

#[derive(Debug, Deserialize)]
struct SidecarBody {
    #[serde(default)]
    required: bool,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    media_types: Vec<SidecarMediaType>,
    // The fields below make this reader compatible with sidecar output from
    // before `media_types` existed.
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    schema_definition: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct SidecarResponse {
    code: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    schema_definition: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct SidecarParameter {
    name: String,
    #[serde(rename = "in")]
    location: String,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    explode: Option<bool>,
    #[serde(default)]
    schema: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct SidecarMediaType {
    content_type: String,
    #[serde(default)]
    schema_definition: Option<Value>,
}

/// Kept source-compatible with `openapi.ExampleDoc`, which is also the shape
/// consumed by Kaji Docs' operation playground payload.
#[derive(Debug, Deserialize, serde::Serialize)]
struct SidecarExample {
    label: String,
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    example_json: String,
}

#[derive(Debug, Deserialize)]
struct SidecarSecurityRequirement {
    #[serde(default)]
    // OpenAPI permits an empty OAuth scope array. The Go sidecar serializes
    // that empty slice as `null`, so retain it as the semantic empty vector
    // instead of rejecting otherwise valid API-key and bearer requirements.
    schemes: BTreeMap<String, Option<Vec<String>>>,
}

#[derive(Debug, Deserialize)]
struct SidecarSchemas {
    #[serde(default)]
    schemas: Vec<SidecarSchema>,
}

#[derive(Debug, Deserialize)]
struct SidecarSchema {
    name: String,
    schema: Value,
}

#[derive(Debug, Deserialize)]
struct SidecarSecuritySchemes {
    #[serde(default)]
    schemes: Vec<SidecarSecurityScheme>,
}

#[derive(Debug, Deserialize)]
struct SidecarSecurityScheme {
    name: String,
    #[serde(rename = "type")]
    type_name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    api_key_name: Option<String>,
    #[serde(default)]
    api_key_in: Option<String>,
    #[serde(default)]
    http_scheme: Option<String>,
    #[serde(default)]
    bearer_format: Option<String>,
    #[serde(default)]
    oauth_flows: Vec<SidecarOAuthFlow>,
    #[serde(default)]
    open_id_connect_url: Option<String>,
    #[serde(default)]
    oauth2_metadata_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SidecarOAuthFlow {
    #[serde(rename = "type")]
    flow_type: String,
    #[serde(default)]
    authorization_url: Option<String>,
    #[serde(default)]
    token_url: Option<String>,
    #[serde(default)]
    refresh_url: Option<String>,
    #[serde(default)]
    scopes: BTreeMap<String, String>,
}

/// Loads an API operation set from a completed Go-sidecar output directory.
/// Component, request, and response schemas go through the same conversion,
/// preventing target generators from knowing the temporary Go output format.
pub fn load_operations(output_dir: &Path, name: String, version: String) -> Result<Api> {
    let index: BTreeMap<String, String> = read_json(&output_dir.join("operations.json"))?;
    let order: Vec<String> = read_json(&output_dir.join("operations-order.json"))?;
    let mut operations = Vec::with_capacity(order.len());

    for key in order {
        let file = index
            .get(&key)
            .with_context(|| format!("sidecar operation index is missing {key:?}"))?;
        let document: SidecarOperation = read_json(&output_dir.join("operations").join(file))?;
        let response_schema = preferred_response_schema(&document.responses);
        let mut annotations = document.extensions;
        add_optional_annotation(&mut annotations, "summary", document.summary);
        add_optional_annotation(&mut annotations, "description", document.description);
        if document.deprecated {
            annotations.insert("deprecated".into(), Value::Bool(true));
        }
        if !document.request_examples.is_empty() {
            // Preserve the exact sidecar/Docs payload under a stable
            // adapter-owned annotation. This keeps the neutral AST backwards
            // compatible for third-party Rust plugins while giving any Kaji
            // profile access to the per-operation examples without reparsing
            // Go-sidecar files.
            annotations.insert(
                "kaji.docs.request_examples".into(),
                serde_json::to_value(&document.request_examples)
                    .expect("sidecar request examples are JSON-compatible"),
            );
        }
        operations.push(Operation {
            id: if document.operation_id.is_empty() {
                operation_id(&document.method, &document.path)
            } else {
                document.operation_id
            },
            method: parse_method(&document.method)?,
            path: document.path,
            response_type: response_schema.map(type_name).unwrap_or_else(|| {
                if document.responses.is_empty() {
                    "void".into()
                } else {
                    "unknown".into()
                }
            }),
            request_type: document
                .request_body
                .as_ref()
                .and_then(|body| body.schema_definition.as_ref())
                .map(type_name),
            parameters: document
                .parameters
                .into_iter()
                .map(|parameter| {
                    let mut annotations = BTreeMap::new();
                    add_optional_annotation(&mut annotations, "style", parameter.style);
                    if let Some(explode) = parameter.explode {
                        annotations.insert("explode".into(), Value::Bool(explode));
                    }
                    OperationParameter {
                        name: parameter.name,
                        location: parameter.location,
                        required: parameter.required,
                        schema: parameter.schema.as_ref().map(convert_value),
                        description: parameter.description,
                        annotations,
                    }
                })
                .collect(),
            request_body: document.request_body.as_ref().map(convert_request_body),
            responses: convert_responses(&document.responses),
            security: document
                .security_requirements
                .into_iter()
                .map(|requirement| SecurityRequirement {
                    schemes: requirement
                        .schemes
                        .into_iter()
                        .map(|(name, scopes)| (name, scopes.unwrap_or_default()))
                        .collect(),
                })
                .collect(),
            annotations,
        });
    }

    let schemas_path = output_dir.join("schemas.json");
    let schemas = if schemas_path.exists() {
        let document: SidecarSchemas = read_json(&schemas_path)?;
        document
            .schemas
            .iter()
            .map(|schema| Schema::new(schema.name.clone(), convert_value(&schema.schema)))
            .collect()
    } else {
        Vec::new()
    };

    Ok(Api {
        name,
        version,
        schemas,
        operations,
        annotations: BTreeMap::new(),
    })
}

/// Loads reusable OpenAPI component security schemes from the Go-sidecar
/// `security-schemes.json` artifact. This deliberately remains independent of
/// [`load_operations`]: callers can adopt typed auth metadata without changing
/// existing operation-only generation paths or requiring older artifacts to
/// contain this newly-added file.
pub fn load_security_schemes(output_dir: &Path) -> Result<SecuritySchemeCatalog> {
    let document: SidecarSecuritySchemes = read_json(&output_dir.join("security-schemes.json"))?;
    Ok(SecuritySchemeCatalog {
        schemes: document
            .schemes
            .into_iter()
            .map(convert_security_scheme)
            .collect(),
    })
}

fn convert_security_scheme(scheme: SidecarSecurityScheme) -> SecurityScheme {
    let kind = match scheme.type_name.as_str() {
        "apiKey" => SecuritySchemeKind::ApiKey {
            name: scheme.api_key_name,
            location: scheme.api_key_in,
        },
        "http" => SecuritySchemeKind::Http {
            scheme: scheme.http_scheme,
            bearer_format: scheme.bearer_format,
        },
        "oauth2" => SecuritySchemeKind::OAuth2 {
            flows: scheme
                .oauth_flows
                .into_iter()
                .map(|flow| OAuthFlow {
                    flow_type: flow.flow_type,
                    authorization_url: flow.authorization_url,
                    token_url: flow.token_url,
                    refresh_url: flow.refresh_url,
                    scopes: flow.scopes,
                })
                .collect(),
            metadata_url: scheme.oauth2_metadata_url,
        },
        "openIdConnect" => SecuritySchemeKind::OpenIdConnect {
            discovery_url: scheme.open_id_connect_url,
        },
        type_name => SecuritySchemeKind::Other {
            type_name: type_name.to_owned(),
        },
    };
    SecurityScheme {
        name: scheme.name,
        description: scheme.description,
        kind,
    }
}

fn convert_request_body(body: &SidecarBody) -> OperationRequestBody {
    let mut media_types = body
        .media_types
        .iter()
        .map(convert_media_type)
        .collect::<Vec<_>>();

    // Read historical one-media-type sidecar output as a single media type.
    if media_types.is_empty() {
        if let Some(content_type) = &body.content_type {
            media_types.push(OperationMediaType {
                content_type: content_type.clone(),
                schema: body.schema_definition.as_ref().map(convert_value),
            });
        }
    }

    OperationRequestBody {
        required: body.required,
        description: body.description.clone(),
        media_types,
    }
}

fn convert_media_type(media_type: &SidecarMediaType) -> OperationMediaType {
    OperationMediaType {
        content_type: media_type.content_type.clone(),
        schema: media_type.schema_definition.as_ref().map(convert_value),
    }
}

fn convert_responses(responses: &[SidecarResponse]) -> Vec<OperationResponse> {
    // The Go sidecar emits one entry per (status, media type). Group it here so
    // the neutral AST reflects OpenAPI's actual response shape while retaining
    // output order deterministically.
    let mut converted = Vec::<OperationResponse>::new();
    for response in responses {
        if let Some(existing) = converted
            .iter_mut()
            .find(|existing| existing.status == response.code)
        {
            if let Some(content_type) = &response.content_type {
                existing.media_types.push(OperationMediaType {
                    content_type: content_type.clone(),
                    schema: response.schema_definition.as_ref().map(convert_value),
                });
            }
            continue;
        }

        converted.push(OperationResponse {
            status: response.code.clone(),
            description: response.description.clone(),
            media_types: response
                .content_type
                .as_ref()
                .map(|content_type| {
                    vec![OperationMediaType {
                        content_type: content_type.clone(),
                        schema: response.schema_definition.as_ref().map(convert_value),
                    }]
                })
                .unwrap_or_default(),
        });
    }
    converted
}

fn preferred_response_schema(responses: &[SidecarResponse]) -> Option<&Value> {
    responses
        .iter()
        .find(|response| response.code.starts_with('2'))
        .or_else(|| responses.first())
        .and_then(|response| response.schema_definition.as_ref())
}

fn add_optional_annotation(
    annotations: &mut BTreeMap<String, Value>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value {
        annotations.insert(key.to_owned(), Value::String(value));
    }
}

/// Converts an OpenAPI Schema Object represented as JSON without rendering a
/// target-language type string or discarding a schema composition keyword.
fn convert_value(schema: &Value) -> SchemaValue {
    let Some(object) = schema.as_object() else {
        return SchemaValue::unknown();
    };
    let mut value = SchemaValue::new(convert_kind(object));
    value.nullable = object
        .get("nullable")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || object
            .get("type")
            .and_then(Value::as_array)
            .is_some_and(|types| types.iter().any(|kind| kind.as_str() == Some("null")));
    value.format = object
        .get("format")
        .and_then(Value::as_str)
        .map(str::to_owned);
    value.enum_values = object
        .get("enum")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    value.const_value = object.get("const").cloned();
    value.default = object.get("default").cloned();
    value.title = object
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_owned);
    value.description = object
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_owned);
    value.deprecated = object
        .get("deprecated")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    value.read_only = object
        .get("readOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    value.write_only = object
        .get("writeOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    value.discriminator = object.get("discriminator").and_then(convert_discriminator);
    for (key, raw) in object {
        if key.starts_with("x-") {
            value.extensions.insert(key.clone(), raw.clone());
        } else if is_constraint_key(key) {
            value.constraints.insert(key.clone(), raw.clone());
        }
    }
    value
}

fn convert_kind(schema: &Map<String, Value>) -> SchemaKind {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return SchemaKind::Reference {
            reference: reference.to_owned(),
        };
    }
    if let Some(variants) = schema.get("oneOf").and_then(Value::as_array) {
        return SchemaKind::OneOf {
            variants: variants.iter().map(convert_value).collect(),
        };
    }
    if let Some(variants) = schema.get("anyOf").and_then(Value::as_array) {
        return SchemaKind::AnyOf {
            variants: variants.iter().map(convert_value).collect(),
        };
    }
    if let Some(variants) = schema.get("allOf").and_then(Value::as_array) {
        return SchemaKind::AllOf {
            variants: variants.iter().map(convert_value).collect(),
        };
    }
    if let Some(negated) = schema.get("not") {
        return SchemaKind::Not {
            schema: Box::new(convert_value(negated)),
        };
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        let variants = types
            .iter()
            .filter_map(Value::as_str)
            .filter(|kind| *kind != "null")
            .map(|kind| SchemaValue::new(kind_from_type(kind, schema)))
            .collect::<Vec<_>>();
        if variants.len() > 1 {
            return SchemaKind::AnyOf { variants };
        }
        return variants
            .into_iter()
            .next()
            .map(|value| value.kind)
            .unwrap_or(SchemaKind::Null);
    }
    schema
        .get("type")
        .and_then(Value::as_str)
        .map(|kind| kind_from_type(kind, schema))
        .unwrap_or_else(|| {
            if schema.contains_key("properties") || schema.contains_key("additionalProperties") {
                kind_from_type("object", schema)
            } else if schema.contains_key("items") {
                kind_from_type("array", schema)
            } else {
                SchemaKind::Any
            }
        })
}

fn kind_from_type(kind: &str, schema: &Map<String, Value>) -> SchemaKind {
    match kind {
        "null" => SchemaKind::Null,
        "boolean" => SchemaKind::Boolean,
        "integer" => SchemaKind::Integer,
        "number" => SchemaKind::Number,
        "string" => SchemaKind::String,
        "array" => SchemaKind::Array {
            items: Box::new(
                schema
                    .get("items")
                    .map(convert_value)
                    .unwrap_or_else(SchemaValue::unknown),
            ),
        },
        "object" => SchemaKind::Object {
            fields: object_fields(schema),
            additional_properties: additional_properties(schema.get("additionalProperties")),
        },
        _ => SchemaKind::Any,
    }
}

fn object_fields(schema: &Map<String, Value>) -> Vec<Field> {
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    schema
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, value)| Field {
            name: name.clone(),
            value: convert_value(value),
            required: required.contains(name.as_str()),
            annotations: BTreeMap::new(),
        })
        .collect()
}

fn additional_properties(value: Option<&Value>) -> AdditionalProperties {
    match value {
        None => AdditionalProperties::Unspecified,
        Some(Value::Bool(true)) => AdditionalProperties::Any,
        Some(Value::Bool(false)) => AdditionalProperties::Forbidden,
        Some(value) => AdditionalProperties::Schema {
            value: Box::new(convert_value(value)),
        },
    }
}

fn convert_discriminator(value: &Value) -> Option<Discriminator> {
    let object = value.as_object()?;
    Some(Discriminator {
        property_name: object.get("propertyName")?.as_str()?.to_owned(),
        mapping: object
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

fn is_constraint_key(key: &str) -> bool {
    matches!(
        key,
        "multipleOf"
            | "maximum"
            | "exclusiveMaximum"
            | "minimum"
            | "exclusiveMinimum"
            | "maxLength"
            | "minLength"
            | "pattern"
            | "maxItems"
            | "minItems"
            | "uniqueItems"
            | "maxProperties"
            | "minProperties"
            | "contentEncoding"
            | "contentMediaType"
            | "example"
            | "examples"
            | "$schema"
            | "$id"
            | "$anchor"
            | "$comment"
            | "unevaluatedProperties"
    )
}

fn type_name(schema: &Value) -> String {
    type_name_from_value(&convert_value(schema))
}

fn type_name_from_value(value: &SchemaValue) -> String {
    let name = match &value.kind {
        SchemaKind::Reference { reference } => {
            reference.rsplit('/').next().unwrap_or("unknown").to_owned()
        }
        SchemaKind::Null => "null".into(),
        SchemaKind::Boolean => "boolean".into(),
        SchemaKind::Integer | SchemaKind::Number => "number".into(),
        SchemaKind::String => "string".into(),
        SchemaKind::Array { items } => format!("{}[]", type_name_from_value(items)),
        SchemaKind::Object { .. } => "Record<string, unknown>".into(),
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => variants
            .iter()
            .map(type_name_from_value)
            .collect::<Vec<_>>()
            .join(" | "),
        SchemaKind::AllOf { variants } => variants
            .iter()
            .map(type_name_from_value)
            .collect::<Vec<_>>()
            .join(" & "),
        SchemaKind::Any | SchemaKind::Not { .. } => "unknown".into(),
    };
    if value.nullable {
        format!("{name} | null")
    } else {
        name
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("read Go OpenAPI sidecar output {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("parse Go OpenAPI sidecar output {}", path.display()))
}

fn parse_method(method: &str) -> Result<HttpMethod> {
    match method {
        "GET" => Ok(HttpMethod::Get),
        "POST" => Ok(HttpMethod::Post),
        "PUT" => Ok(HttpMethod::Put),
        "PATCH" => Ok(HttpMethod::Patch),
        "DELETE" => Ok(HttpMethod::Delete),
        other => bail!("unsupported HTTP method from Go OpenAPI sidecar: {other}"),
    }
}

fn operation_id(method: &str, path: &str) -> String {
    let parts = path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let segment = segment.trim_matches(['{', '}']);
            let mut characters = segment.chars();
            characters
                .next()
                .map(|first| format!("{}{}", first.to_uppercase(), characters.as_str()))
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("");
    let prefix = method.to_ascii_lowercase();
    if parts.is_empty() {
        prefix
    } else {
        format!("{prefix}{parts}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn preserves_composed_components_and_operation_schemas() {
        let temp = tempfile::tempdir().unwrap();
        let operations = temp.path().join("operations");
        fs::create_dir_all(&operations).unwrap();
        fs::write(
            temp.path().join("operations.json"),
            r#"{"GET /pets/{petId}":"get.json"}"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("operations-order.json"),
            r#"["GET /pets/{petId}"]"#,
        )
        .unwrap();
        fs::write(operations.join("get.json"), r##"{"path":"/pets/{petId}","method":"GET","request_body":{"schema_definition":{"$ref":"#/components/schemas/Pet"}},"responses":[{"code":"200","schema_definition":{"type":"array","items":{"$ref":"#/components/schemas/Pet"}}}]}"##).unwrap();
        fs::write(temp.path().join("schemas.json"), r##"{"schemas":[{"name":"Pet","schema":{"type":"object","required":["name"],"additionalProperties":{"type":"string"},"properties":{"name":{"type":"string","format":"uuid"},"kind":{"type":["string","null"],"enum":["cat","dog"]}},"x-target-name":"animal"}},{"name":"Animal","schema":{"oneOf":[{"$ref":"#/components/schemas/Pet"},{"type":"integer"}],"nullable":true,"discriminator":{"propertyName":"kind","mapping":{"cat":"#/components/schemas/Pet"}}}}]}"##).unwrap();
        let api = load_operations(temp.path(), "Pets".into(), "1.0.0".into()).unwrap();
        assert_eq!(api.operations[0].request_type.as_deref(), Some("Pet"));
        assert_eq!(api.operations[0].response_type, "Pet[]");
        let SchemaKind::Object {
            fields,
            additional_properties,
        } = &api.schemas[0].value.kind
        else {
            panic!("Pet must be an object")
        };
        assert!(matches!(
            additional_properties,
            AdditionalProperties::Schema { .. }
        ));
        assert!(fields[0].value.nullable);
        assert_eq!(
            fields[0].value.enum_values,
            vec![Value::String("cat".into()), Value::String("dog".into())]
        );
        assert_eq!(
            api.schemas[0].value.extensions["x-target-name"],
            Value::String("animal".into())
        );
        let SchemaKind::OneOf { variants } = &api.schemas[1].value.kind else {
            panic!("Animal must be a union")
        };
        assert_eq!(variants.len(), 2);
        assert!(api.schemas[1].value.nullable);
        assert_eq!(
            api.schemas[1]
                .value
                .discriminator
                .as_ref()
                .unwrap()
                .property_name,
            "kind"
        );
    }

    #[test]
    fn retains_all_of_and_constraints() {
        let value = convert_value(
            &serde_json::json!({"allOf":[{"$ref":"#/components/schemas/Base"},{"type":"object","additionalProperties":false}],"minimum":3}),
        );
        assert!(matches!(value.kind, SchemaKind::AllOf { .. }));
        assert_eq!(value.constraints["minimum"], Value::from(3));
    }

    #[test]
    fn preserves_operation_transport_contracts() {
        let temp = tempfile::tempdir().unwrap();
        let operations = temp.path().join("operations");
        fs::create_dir_all(&operations).unwrap();
        fs::write(
            temp.path().join("operations.json"),
            r#"{"POST /pets/{petId}":"post.json"}"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("operations-order.json"),
            r#"["POST /pets/{petId}"]"#,
        )
        .unwrap();
        fs::write(
            operations.join("post.json"),
            r##"{
              "path":"/pets/{petId}","method":"POST",
              "parameters":[
                {"name":"petId","in":"path","required":true,"style":"matrix","explode":true,"schema":{"type":"string"}},
                {"name":"include","in":"query","style":"pipeDelimited","explode":false,"schema":{"type":"array","items":{"type":"string"}}}
              ],
              "request_body":{"required":true,"description":"New pet","content_type":"application/json","schema_definition":{"$ref":"#/components/schemas/PetInput"},"media_types":[
                {"content_type":"application/json","schema_definition":{"$ref":"#/components/schemas/PetInput"}},
                {"content_type":"application/xml","schema_definition":{"type":"string"}}
              ]},
              "responses":[
                {"code":"201","description":"Created","content_type":"application/json","schema_definition":{"$ref":"#/components/schemas/Pet"}},
                {"code":"201","description":"Created","content_type":"application/xml","schema_definition":{"type":"string"}},
                {"code":"default","description":"Failure"}
              ],
              "security_requirements":[
                {"schemes":{"oauth":["pets:write"],"api_key":[]}},
                {"schemes":{"anonymous":[]}}
              ]
            }"##,
        )
        .unwrap();

        let api = load_operations(temp.path(), "Pets".into(), "1.0.0".into()).unwrap();
        let operation = &api.operations[0];
        assert_eq!(operation.parameters.len(), 2);
        assert_eq!(operation.parameters[0].location, "path");
        assert!(operation.parameters[0].required);
        assert_eq!(operation.parameters[0].annotations["style"], "matrix");
        assert_eq!(operation.parameters[0].annotations["explode"], true);
        assert_eq!(
            operation.parameters[1].annotations["style"],
            "pipeDelimited"
        );
        assert_eq!(operation.parameters[1].annotations["explode"], false);
        assert!(matches!(
            operation.parameters[1].schema.as_ref().unwrap().kind,
            SchemaKind::Array { .. }
        ));
        let request = operation.request_body.as_ref().unwrap();
        assert!(request.required);
        assert_eq!(request.media_types.len(), 2);
        assert_eq!(request.media_types[1].content_type, "application/xml");
        assert_eq!(operation.responses.len(), 2);
        assert_eq!(operation.responses[0].status, "201");
        assert_eq!(operation.responses[0].media_types.len(), 2);
        assert!(operation.responses[1].media_types.is_empty());
        assert_eq!(operation.security.len(), 2);
        assert_eq!(operation.security[0].schemes["oauth"], ["pets:write"]);
        assert!(operation.security[0].schemes.contains_key("api_key"));
    }

    #[test]
    fn preserves_request_examples_for_docs_consumers() {
        let temp = tempfile::tempdir().unwrap();
        let operations = temp.path().join("operations");
        fs::create_dir_all(&operations).unwrap();
        fs::write(
            temp.path().join("operations.json"),
            r#"{"POST /widgets":"post.json"}"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("operations-order.json"),
            r#"["POST /widgets"]"#,
        )
        .unwrap();
        fs::write(
            operations.join("post.json"),
            r##"{
              "path":"/widgets", "method":"POST", "operation_id":"createWidget",
              "request_examples":[
                {"label":"Request application/json: minimal", "content_type":"application/json", "example_json":"{\"name\":\"widget\"}"},
                {"label":"Request application/json: complete", "content_type":"application/json", "example_json":"{\"name\":\"widget\",\"enabled\":true}"}
              ],
              "responses":[{"code":"201"}]
            }"##,
        )
        .unwrap();

        let api = load_operations(temp.path(), "Widgets".into(), "1.0.0".into()).unwrap();
        let examples = api.operations[0]
            .annotations
            .get("kaji.docs.request_examples")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(examples.len(), 2);
        assert_eq!(examples[0]["label"], "Request application/json: minimal");
        assert_eq!(examples[0]["content_type"], "application/json");
        assert_eq!(
            examples[1]["example_json"],
            r#"{"name":"widget","enabled":true}"#
        );
    }

    #[test]
    fn preserves_kaji_mock_contract_for_typed_extraction() {
        let temp = tempfile::tempdir().unwrap();
        let operations = temp.path().join("operations");
        fs::create_dir_all(&operations).unwrap();
        fs::write(
            temp.path().join("operations.json"),
            r#"{"GET /widgets":"get.json"}"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("operations-order.json"),
            r#"["GET /widgets"]"#,
        )
        .unwrap();
        fs::write(
            operations.join("get.json"),
            r##"{
              "path":"/widgets", "method":"GET", "operation_id":"listWidgets",
              "extensions": {
                "x-kaji-mock": {
                  "scenarios": [{
                    "name":"rate-limited",
                    "when":{"headers":{"x-test-scenario":"rate-limited"}},
                    "response":{"status":429,"headers":{"retry-after":"1"},"body":{"message":"Too many requests"}}
                  }]
                }
              },
              "responses":[{"code":"200"}]
            }"##,
        )
        .unwrap();

        let api = load_operations(temp.path(), "Widgets".into(), "1.0.0".into()).unwrap();
        assert!(api.operations[0].annotations.contains_key("x-kaji-mock"));
        let scenarios = crate::extract_mock_scenarios(&api).unwrap();
        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].name, "rate-limited");
        assert_eq!(scenarios[0].response.status, 429);
    }

    #[test]
    fn loads_security_scheme_catalog_without_losing_auth_metadata() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("security-schemes.json"),
            r#"{
              "schemes": [
                {
                  "name": "ApiKey",
                  "type": "apiKey",
                  "description": "Tenant key",
                  "api_key_name": "X-API-Key",
                  "api_key_in": "header"
                },
                {
                  "name": "Bearer",
                  "type": "http",
                  "http_scheme": "bearer",
                  "bearer_format": "JWT"
                },
                {
                  "name": "OAuth",
                  "type": "oauth2",
                  "oauth2_metadata_url": "https://example.test/metadata",
                  "oauth_flows": [{
                    "type": "authorizationCode",
                    "authorization_url": "https://example.test/authorize",
                    "token_url": "https://example.test/token",
                    "refresh_url": "https://example.test/refresh",
                    "scopes": { "pets:read": "Read pets" }
                  }]
                }
              ]
            }"#,
        )
        .unwrap();

        let catalog = load_security_schemes(temp.path()).unwrap();
        assert_eq!(catalog.schemes.len(), 3);
        assert!(matches!(
            &catalog.schemes[0].kind,
            SecuritySchemeKind::ApiKey {
                name: Some(name),
                location: Some(location),
            } if name == "X-API-Key" && location == "header"
        ));
        assert!(matches!(
            &catalog.schemes[1].kind,
            SecuritySchemeKind::Http {
                scheme: Some(scheme),
                bearer_format: Some(format),
            } if scheme == "bearer" && format == "JWT"
        ));
        let SecuritySchemeKind::OAuth2 {
            flows,
            metadata_url: Some(metadata_url),
        } = &catalog.schemes[2].kind
        else {
            panic!("OAuth scheme metadata was not loaded")
        };
        assert_eq!(metadata_url, "https://example.test/metadata");
        assert_eq!(flows[0].flow_type, "authorizationCode");
        assert_eq!(
            flows[0].token_url.as_deref(),
            Some("https://example.test/token")
        );
        assert_eq!(flows[0].scopes["pets:read"], "Read pets");
    }
}
