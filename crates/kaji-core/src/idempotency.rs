//! Explicit, package-local idempotency policy. Runtime generation consumes the
//! resolved annotation; preparing an API never changes the caller's API.
use crate::{Api, OperationParameter, SchemaKind, SchemaValue};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const RESOLVED_ANNOTATION: &str = "x-kaji-idempotency-resolved";
fn enabled() -> bool {
    true
}
fn header() -> String {
    "Idempotency-Key".into()
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IdempotencyRule {
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default = "header")]
    pub header: String,
    #[serde(default)]
    pub auto_generate: bool,
}
impl Default for IdempotencyRule {
    fn default() -> Self {
        Self {
            enabled: true,
            header: header(),
            auto_generate: false,
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdempotencyConfig {
    #[serde(default)]
    pub defaults: Option<IdempotencyRule>,
    #[serde(default)]
    pub operations: BTreeMap<String, IdempotencyRule>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResolvedIdempotency {
    pub header: String,
    pub parameter_name: String,
    pub auto_generate: bool,
}
/// Read generator-resolved metadata. Call this on the API supplied by a package.
pub fn resolved(operation: &crate::Operation) -> Option<ResolvedIdempotency> {
    serde_json::from_value(operation.annotations.get(RESOLVED_ANNOTATION)?.clone()).ok()
}

fn valid_header(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}
/// Recipe defaults override operation extensions; recipe operation rules override
/// both. An absent policy is disabled. Explicit disabled rules suppress headers.
pub fn prepare_api(api: &Api, config: &IdempotencyConfig) -> Result<Api> {
    for id in config.operations.keys() {
        let count = api
            .operations
            .iter()
            .filter(|operation| &operation.id == id)
            .count();
        if count != 1 {
            bail!(
                "idempotency operation {id:?} must identify exactly one operation (found {count})"
            );
        }
    }
    let mut prepared = api.clone();
    for operation in &mut prepared.operations {
        operation.annotations.remove(RESOLVED_ANNOTATION);
        let extension = operation
            .annotations
            .get("x-kaji-idempotency")
            .filter(|_| !config.operations.contains_key(&operation.id) && config.defaults.is_none())
            .map(|value| match value {
                serde_json::Value::Bool(value) => Ok(IdempotencyRule {
                    enabled: *value,
                    ..Default::default()
                }),
                _ => serde_json::from_value(value.clone())
                    .context("x-kaji-idempotency must be a boolean or rule object"),
            })
            .transpose()
            .with_context(|| format!("idempotency for {}", operation.id))?;
        let rule = config
            .operations
            .get(&operation.id)
            .or(config.defaults.as_ref())
            .or(extension.as_ref());
        let Some(rule) = rule.filter(|rule| rule.enabled) else {
            continue;
        };
        if !valid_header(&rule.header) {
            bail!("invalid idempotency header name for {}", operation.id);
        }
        if matches!(
            rule.header.to_ascii_lowercase().as_str(),
            "authorization"
                | "proxy-authorization"
                | "host"
                | "content-type"
                | "content-length"
                | "transfer-encoding"
                | "connection"
                | "cookie"
                | "set-cookie"
                | "trailer"
                | "te"
                | "upgrade"
                | "accept"
                | "accept-encoding"
                | "content-encoding"
                | "user-agent"
        ) {
            bail!(
                "idempotency policy for {} cannot use a transport/authentication header",
                operation.id
            );
        }
        let identifier_key = |name: &str| {
            name.bytes()
                .filter(u8::is_ascii_alphanumeric)
                .map(|byte| byte.to_ascii_lowercase())
                .collect::<Vec<_>>()
        };
        let key = identifier_key(&rule.header);
        if key.is_empty() {
            bail!(
                "idempotency header for {} must contain an ASCII alphanumeric identifier",
                operation.id
            );
        }
        if operation.parameters.iter().any(|parameter| {
            identifier_key(&parameter.name) == key
                && (parameter.location != "header"
                    || !parameter.name.eq_ignore_ascii_case(&rule.header))
        }) {
            bail!(
                "idempotency header for {} collides with another parameter identifier",
                operation.id
            );
        }
        let matching: Vec<_> = operation
            .parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| {
                parameter.location == "header" && parameter.name.eq_ignore_ascii_case(&rule.header)
            })
            .map(|(index, _)| index)
            .collect();
        if matching.len() > 1 {
            bail!(
                "duplicate idempotency header parameters for {}",
                operation.id
            );
        }
        let name = if let Some(index) = matching.first() {
            let parameter = &operation.parameters[*index];
            let schema = parameter
                .schema
                .as_ref()
                .context("idempotency header requires a string schema")?;
            if !matches!(schema.kind, SchemaKind::String)
                || schema.nullable
                || schema.nullish
                || schema.optional
                || !schema.enum_values.is_empty()
                || schema.const_value.is_some()
                || schema.format.is_some()
                || !schema.constraints.is_empty()
                || schema.default.is_some()
            {
                bail!(
                    "idempotency header for {} requires a direct unconstrained string schema",
                    operation.id
                );
            }
            if parameter.required {
                bail!("idempotency header for {} must be optional", operation.id);
            }
            parameter.name.clone()
        } else {
            // Preserve the wire header spelling; renderers apply their usual native identifier mapping.
            operation.parameters.push(OperationParameter {
                name: rule.header.clone(),
                location: "header".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: Some("Caller-supplied idempotency key for this operation.".into()),
                annotations: BTreeMap::new(),
            });
            rule.header.clone()
        };
        operation.annotations.insert(
            RESOLVED_ANNOTATION.into(),
            serde_json::to_value(ResolvedIdempotency {
                header: name.clone(),
                parameter_name: name,
                auto_generate: rule.auto_generate,
            })?,
        );
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn api() -> Api {
        let mut api = Api::default();
        api.operations.push(crate::Operation {
            id: "create".into(),
            method: crate::HttpMethod::Post,
            path: "/items".into(),
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security: vec![],
            annotations: BTreeMap::new(),
        });
        api
    }
    #[test]
    fn opt_in_precedence_and_input_are_preserved() {
        let mut source = api();
        assert!(
            prepare_api(&source, &Default::default())
                .unwrap()
                .operations[0]
                .parameters
                .is_empty()
        );
        source.operations[0].annotations.insert(
            "x-kaji-idempotency".into(),
            serde_json::json!({"auto_generate":true}),
        );
        let prepared = prepare_api(&source, &Default::default()).unwrap();
        assert_eq!(prepared.operations[0].parameters[0].name, "Idempotency-Key");
        assert!(
            prepared.operations[0].annotations[RESOLVED_ANNOTATION]["auto_generate"]
                .as_bool()
                .unwrap()
        );
        assert!(source.operations[0].parameters.is_empty());
        let config = IdempotencyConfig {
            defaults: Some(IdempotencyRule {
                enabled: false,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(
            prepare_api(&source, &config).unwrap().operations[0]
                .parameters
                .is_empty()
        );
    }
    #[test]
    fn overrides_ignore_source_policy_and_untrusted_resolution() {
        let mut source = api();
        source.operations[0].annotations.insert(
            "x-kaji-idempotency".into(),
            serde_json::json!({"unknown":true}),
        );
        source.operations[0].annotations.insert(
            RESOLVED_ANNOTATION.into(),
            serde_json::json!({"header":"Fake", "parameter_name":"Fake", "auto_generate":true}),
        );
        assert!(prepare_api(&source, &Default::default()).is_err());
        let config = IdempotencyConfig {
            defaults: Some(IdempotencyRule {
                enabled: false,
                ..Default::default()
            }),
            operations: BTreeMap::from([(
                "create".into(),
                IdempotencyRule {
                    header: "X-Request-Key".into(),
                    ..Default::default()
                },
            )]),
        };
        let prepared = prepare_api(&source, &config).unwrap();
        assert_eq!(
            resolved(&prepared.operations[0]).unwrap().header,
            "X-Request-Key"
        );
        let disabled = IdempotencyConfig {
            defaults: Some(IdempotencyRule {
                enabled: false,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(resolved(&prepare_api(&source, &disabled).unwrap().operations[0]).is_none());
        for header in ["", "X-Key\r\nInjected", "contains space", "ümlaut"] {
            let config = IdempotencyConfig {
                defaults: Some(IdempotencyRule {
                    header: header.into(),
                    ..Default::default()
                }),
                ..Default::default()
            };
            assert!(prepare_api(&api(), &config).is_err());
        }
    }

    #[test]
    fn rejects_reserved_headers_and_parameter_identifier_collisions() {
        for header in [
            "Authorization",
            "content-TYPE",
            "Host",
            "Cookie",
            "Content-Length",
            "---",
        ] {
            let config = IdempotencyConfig {
                defaults: Some(IdempotencyRule {
                    header: header.into(),
                    ..Default::default()
                }),
                ..Default::default()
            };
            assert!(prepare_api(&api(), &config).is_err());
        }
        for name in ["Idempotency-Key", "idempotencyKey", "IDEMPOTENCY_KEY"] {
            let mut source = api();
            source.operations[0].parameters.push(OperationParameter {
                name: name.into(),
                location: "query".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: BTreeMap::new(),
            });
            let config = IdempotencyConfig {
                defaults: Some(Default::default()),
                ..Default::default()
            };
            assert!(prepare_api(&source, &config).is_err());
        }
    }

    #[test]
    fn header_matching_validation_and_unknown_operation() {
        let mut source = api();
        source.operations[0]
            .annotations
            .insert("x-kaji-idempotency".into(), serde_json::json!(true));
        source = prepare_api(&source, &Default::default()).unwrap();
        source.operations[0].parameters[0].name = "idempotency-key".into();
        let prepared = prepare_api(&source, &Default::default()).unwrap();
        assert_eq!(prepared.operations[0].parameters.len(), 1);
        assert_eq!(
            prepared.operations[0].annotations[RESOLVED_ANNOTATION]["header"],
            "idempotency-key"
        );
        source.operations[0].parameters[0].required = true;
        assert!(prepare_api(&source, &Default::default()).is_err());
        source.operations[0].parameters[0].required = false;
        let duplicate = source.operations[0].parameters[0].clone();
        source.operations[0].parameters.push(duplicate);
        assert!(prepare_api(&source, &Default::default()).is_err());
        let config = IdempotencyConfig {
            operations: BTreeMap::from([("missing".into(), Default::default())]),
            ..Default::default()
        };
        assert!(prepare_api(&api(), &config).is_err());
    }
}
