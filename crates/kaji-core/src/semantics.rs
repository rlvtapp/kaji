//! OpenAPI-derived SDK behaviour shared by language targets.
//!
//! Renderers should not each rediscover whether an operation streams, can be
//! retried, needs a credential, or has typed error responses. This module is
//! deliberately free of language and HTTP-client details: it turns the
//! normalized API AST into a small, stable behaviour plan which a selected
//! client generator can render idiomatically.

use crate::{
    Api, HttpMethod, Operation, OperationMediaType, SecuritySchemeCatalog, SecuritySchemeKind,
};

/// The complete target-neutral behaviour plan for one API.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SdkSemantics {
    pub operations: Vec<OperationSemantics>,
}

/// Behaviour that applies to one generated operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationSemantics {
    pub operation_id: String,
    pub auth: Vec<AuthAlternative>,
    pub errors: Vec<DeclaredError>,
    pub retry: RetryClass,
    pub streaming: Option<StreamingKind>,
    pub request_body: Option<RequestBodyKind>,
    pub pagination: Option<PaginationHint>,
}

/// One OpenAPI security alternative. Every item inside an alternative is
/// required; alternatives themselves are OR-ed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthAlternative {
    pub schemes: Vec<AuthScheme>,
}

/// A resolved credential requirement. Unknown schemes are deliberately kept
/// as `Other` so targets can report a useful configuration error rather than
/// quietly omitting security.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthScheme {
    ApiKey { name: String, location: String },
    Http { scheme: String },
    OAuth2 { scopes: Vec<String> },
    OpenIdConnect { discovery_url: Option<String> },
    Other { name: String },
}

/// A non-success response declared by an operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredError {
    pub status: String,
    pub body_type: Option<String>,
    pub content_type: Option<String>,
}

/// Retry safety derived from HTTP semantics and explicit idempotency headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryClass {
    /// GET, PUT, PATCH, and DELETE may use the selected runtime's normal retry
    /// policy. A client still decides which failures/statuses are retryable.
    Idempotent,
    /// POST is only safe to retry when the API declares an idempotency key.
    IdempotencyKey,
    /// Do not retry automatically.
    Unsafe,
}

/// A response whose media type requires a streaming API instead of ordinary
/// JSON deserialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamingKind {
    ServerSentEvents,
    Binary,
}

/// Request encodings that require specialised client-runtime handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestBodyKind {
    Json,
    Multipart,
    FormUrlEncoded,
    Binary,
    Other,
}

/// An opt-in pagination declaration. Pagination is intentionally not guessed
/// from operation names: a false pager is worse than no pager. Adapters retain
/// vendor extensions, so Kaji accepts its own portable extension and the
/// established Speakeasy declaration while keeping their payload opaque until
/// a concrete paginator renderer needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaginationHint {
    pub source: PaginationSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaginationSource {
    Kaji,
    Speakeasy,
}

/// Analyses every operation once. A caller that has no component security
/// artifact can pass `None`; operation security remains visible as `Other`.
pub fn analyze_sdk_semantics(
    api: &Api,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> SdkSemantics {
    SdkSemantics {
        operations: api
            .operations
            .iter()
            .map(|operation| analyze_operation(operation, security_schemes))
            .collect(),
    }
}

pub fn analyze_operation(
    operation: &Operation,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> OperationSemantics {
    OperationSemantics {
        operation_id: operation.id.clone(),
        auth: operation
            .security
            .iter()
            .map(|requirement| AuthAlternative {
                schemes: requirement
                    .schemes
                    .iter()
                    .map(|(name, scopes)| resolve_auth(name, scopes, security_schemes))
                    .collect(),
            })
            .collect(),
        errors: operation
            .responses
            .iter()
            .filter(|response| is_error_status(&response.status))
            .map(|response| DeclaredError {
                status: response.status.clone(),
                body_type: response
                    .media_types
                    .first()
                    .and_then(|media| media.schema.as_ref())
                    .and_then(|schema| schema.kind.reference_name())
                    .map(str::to_owned),
                content_type: response
                    .media_types
                    .first()
                    .map(|media| media.content_type.clone()),
            })
            .collect(),
        retry: retry_class(operation),
        streaming: streaming_kind(operation),
        request_body: operation
            .request_body
            .as_ref()
            .and_then(|body| body.media_types.first())
            .map(request_body_kind),
        pagination: pagination_hint(operation),
    }
}

fn resolve_auth(
    name: &str,
    scopes: &[String],
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> AuthScheme {
    let scheme = security_schemes
        .and_then(|catalog| catalog.schemes.iter().find(|scheme| scheme.name == name));
    match scheme.map(|scheme| &scheme.kind) {
        Some(SecuritySchemeKind::ApiKey { name, location }) => AuthScheme::ApiKey {
            name: name.clone().unwrap_or_else(|| "Authorization".into()),
            location: location.clone().unwrap_or_else(|| "header".into()),
        },
        Some(SecuritySchemeKind::Http { scheme, .. }) => AuthScheme::Http {
            scheme: scheme.clone().unwrap_or_else(|| "bearer".into()),
        },
        Some(SecuritySchemeKind::OAuth2 { .. }) => AuthScheme::OAuth2 {
            scopes: scopes.to_vec(),
        },
        Some(SecuritySchemeKind::OpenIdConnect { discovery_url }) => AuthScheme::OpenIdConnect {
            discovery_url: discovery_url.clone(),
        },
        Some(SecuritySchemeKind::Other { .. }) | None => AuthScheme::Other { name: name.into() },
    }
}

fn is_error_status(status: &str) -> bool {
    status == "default"
        || status
            .parse::<u16>()
            .is_ok_and(|status| (400..600).contains(&status))
}

fn retry_class(operation: &Operation) -> RetryClass {
    if operation.parameters.iter().any(|parameter| {
        parameter.location == "header" && parameter.name.eq_ignore_ascii_case("idempotency-key")
    }) {
        return RetryClass::IdempotencyKey;
    }
    match operation.method {
        HttpMethod::Get | HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete => {
            RetryClass::Idempotent
        }
        HttpMethod::Post => RetryClass::Unsafe,
    }
}

fn streaming_kind(operation: &Operation) -> Option<StreamingKind> {
    operation
        .responses
        .iter()
        .flat_map(|response| &response.media_types)
        .find_map(|media| match media.content_type.as_str() {
            "text/event-stream" => Some(StreamingKind::ServerSentEvents),
            "application/octet-stream" | "application/pdf" | "image/png" | "image/jpeg" => {
                Some(StreamingKind::Binary)
            }
            _ => None,
        })
}

fn request_body_kind(media: &OperationMediaType) -> RequestBodyKind {
    match media.content_type.as_str() {
        "application/json" | "application/problem+json" => RequestBodyKind::Json,
        "multipart/form-data" => RequestBodyKind::Multipart,
        "application/x-www-form-urlencoded" => RequestBodyKind::FormUrlEncoded,
        "application/octet-stream" => RequestBodyKind::Binary,
        _ => RequestBodyKind::Other,
    }
}

fn pagination_hint(operation: &Operation) -> Option<PaginationHint> {
    if operation.annotations.contains_key("x-kaji-pagination") {
        Some(PaginationHint {
            source: PaginationSource::Kaji,
        })
    } else if operation.annotations.contains_key("x-speakeasy-pagination") {
        Some(PaginationHint {
            source: PaginationSource::Speakeasy,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::{
        OperationMediaType, OperationParameter, OperationResponse, SchemaKind, SchemaValue,
        SecurityRequirement, SecurityScheme,
    };

    use super::*;

    #[test]
    fn resolves_auth_errors_retry_and_media_from_one_operation() {
        let operation = Operation {
            id: "createMessage".into(),
            method: HttpMethod::Post,
            path: "/messages".into(),
            parameters: vec![OperationParameter {
                name: "Idempotency-Key".into(),
                location: "header".into(),
                required: false,
                schema: None,
                description: None,
                annotations: BTreeMap::new(),
            }],
            request_body: Some(crate::OperationRequestBody {
                required: true,
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "multipart/form-data".into(),
                    schema: None,
                }],
            }),
            responses: vec![
                OperationResponse {
                    status: "201".into(),
                    description: None,
                    media_types: vec![],
                },
                OperationResponse {
                    status: "429".into(),
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "application/json".into(),
                        schema: Some(SchemaValue::reference(
                            "#/components/schemas/RateLimitError",
                        )),
                    }],
                },
                OperationResponse {
                    status: "default".into(),
                    description: None,
                    media_types: vec![],
                },
            ],
            security: vec![SecurityRequirement {
                schemes: BTreeMap::from([("apiKey".into(), vec![])]),
            }],
            annotations: BTreeMap::from([("x-kaji-pagination".into(), json!({}))]),
            ..Operation::default()
        };
        let catalog = SecuritySchemeCatalog {
            schemes: vec![SecurityScheme {
                name: "apiKey".into(),
                description: None,
                kind: SecuritySchemeKind::ApiKey {
                    name: Some("X-API-Key".into()),
                    location: Some("header".into()),
                },
            }],
        };

        let semantics = analyze_operation(&operation, Some(&catalog));
        assert_eq!(semantics.retry, RetryClass::IdempotencyKey);
        assert_eq!(semantics.request_body, Some(RequestBodyKind::Multipart));
        assert_eq!(semantics.errors.len(), 2);
        assert_eq!(
            semantics.errors[0].body_type.as_deref(),
            Some("RateLimitError")
        );
        assert_eq!(
            semantics.auth,
            vec![AuthAlternative {
                schemes: vec![AuthScheme::ApiKey {
                    name: "X-API-Key".into(),
                    location: "header".into(),
                }],
            }]
        );
        assert_eq!(
            semantics.pagination,
            Some(PaginationHint {
                source: PaginationSource::Kaji,
            })
        );
    }

    #[test]
    fn detects_server_sent_events_and_does_not_guess_pagination() {
        let operation = Operation {
            id: "watchEvents".into(),
            responses: vec![OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "text/event-stream".into(),
                    schema: Some(SchemaValue::new(SchemaKind::String)),
                }],
            }],
            ..Operation::default()
        };
        let semantics = analyze_operation(&operation, None);
        assert_eq!(semantics.streaming, Some(StreamingKind::ServerSentEvents));
        assert_eq!(semantics.pagination, None);
        assert_eq!(semantics.retry, RetryClass::Idempotent);
    }
}
