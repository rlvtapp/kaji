use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A normalized, language-neutral description of an API.
///
/// Adapters own the conversion from OpenAPI (or another input) into this
/// structure. Generator plugins never need to know which source format was
/// used. `SchemaValue` intentionally retains OpenAPI's compositional type
/// system instead of reducing it to target-language strings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Api {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub schemas: Vec<Schema>,
    #[serde(default)]
    pub operations: Vec<Operation>,
    /// Adapter-specific source metadata that is safe for generic transforms to
    /// inspect without coupling the generator to OpenAPI.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

/// A named reusable schema, normally an OpenAPI component schema.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub value: SchemaValue,
}

impl Schema {
    pub fn new(name: impl Into<String>, value: SchemaValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

/// A recursively composable API type. This is deliberately target neutral:
/// plugins decide whether (for example) `String { format: uuid }` becomes a
/// branded TypeScript string, a Python UUID, or a Rust `uuid::Uuid`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaValue {
    pub kind: SchemaKind,
    #[serde(default)]
    pub nullable: bool,
    /// A target-neutral schema wrapper for contexts which retain optionality
    /// on the value rather than on an enclosing object property.
    #[serde(default)]
    pub optional: bool,
    /// A target-neutral schema wrapper for values which accept either `null`
    /// or `undefined`. OpenAPI itself normally models this through a nullable,
    /// non-required property, but retaining it makes lossless adapters and
    /// schema generators possible.
    #[serde(default)]
    pub nullish: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub const_value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub write_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Discriminator>,
    /// Validation keywords and vendor-neutral OpenAPI keywords which do not
    /// alter the structural shape. Keeping them here means an adapter does not
    /// throw away constraints that a target plugin may support later.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub constraints: BTreeMap<String, Value>,
    /// Unknown `x-*` and future schema keywords. This is a forward-compatible
    /// lossless escape hatch; core generators must not depend on it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, Value>,
}

impl SchemaValue {
    pub fn new(kind: SchemaKind) -> Self {
        Self {
            kind,
            nullable: false,
            optional: false,
            nullish: false,
            format: None,
            enum_values: Vec::new(),
            const_value: None,
            default: None,
            title: None,
            description: None,
            deprecated: false,
            read_only: false,
            write_only: false,
            discriminator: None,
            constraints: BTreeMap::new(),
            extensions: BTreeMap::new(),
        }
    }

    pub fn unknown() -> Self {
        Self::new(SchemaKind::Any)
    }

    pub fn reference(reference: impl Into<String>) -> Self {
        Self::new(SchemaKind::Reference {
            reference: reference.into(),
        })
    }
}

/// The structural part of a [`SchemaValue`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SchemaKind {
    Any,
    Null,
    Boolean,
    Integer,
    Number,
    String,
    Array {
        items: Box<SchemaValue>,
    },
    Object {
        fields: Vec<Field>,
        additional_properties: AdditionalProperties,
    },
    /// The full OpenAPI/JSON Schema reference is retained to support local,
    /// external, and non-component references. Use `reference_name` when a
    /// target needs the conventional final JSON-pointer segment.
    Reference {
        reference: String,
    },
    OneOf {
        variants: Vec<SchemaValue>,
    },
    AnyOf {
        variants: Vec<SchemaValue>,
    },
    AllOf {
        variants: Vec<SchemaValue>,
    },
    Not {
        schema: Box<SchemaValue>,
    },
}

impl SchemaKind {
    pub fn reference_name(&self) -> Option<&str> {
        let Self::Reference { reference } = self else {
            return None;
        };
        Some(reference.rsplit('/').next().unwrap_or(reference))
    }
}

/// OpenAPI's `additionalProperties` has three semantically distinct states.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdditionalProperties {
    /// The keyword was omitted. A generator can apply the relevant OpenAPI
    /// version's default without confusing it with an explicit `true`.
    #[default]
    Unspecified,
    Any,
    Forbidden,
    Schema {
        value: Box<SchemaValue>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Discriminator {
    pub property_name: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mapping: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub value: SchemaValue,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub method: HttpMethod,
    pub path: String,
    /// Kept for the first-generation renderers. New generators should consume
    /// [`Operation::responses`] so they can select by status code and media
    /// type instead of relying on this lossy convenience value.
    pub response_type: String,
    /// Kept for the first-generation renderers. New generators should consume
    /// [`Operation::request_body`] for the complete media-type set.
    pub request_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<OperationParameter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_body: Option<OperationRequestBody>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub responses: Vec<OperationResponse>,
    /// OpenAPI security requirements, preserving its OR-of-AND structure.
    /// Each entry is an alternative; every named scheme inside an entry is
    /// required together. Values are OAuth/OpenID scopes where applicable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security: Vec<SecurityRequirement>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl Default for Operation {
    fn default() -> Self {
        Self {
            id: String::new(),
            method: HttpMethod::Get,
            path: String::new(),
            response_type: "void".into(),
            request_type: None,
            parameters: Vec::new(),
            request_body: None,
            responses: Vec::new(),
            security: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }
}

/// A path, query, header, or cookie input accepted by an operation. `location`
/// intentionally remains a string so the neutral AST can preserve future or
/// vendor-defined OpenAPI locations instead of rejecting an otherwise valid
/// source document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationParameter {
    pub name: String,
    pub location: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<SchemaValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

/// An operation request body. Its media types are deliberately independent of
/// the legacy `request_type`, since a single request can have JSON, XML, or
/// form encodings with different schemas.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationRequestBody {
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media_types: Vec<OperationMediaType>,
}

/// A response grouped by OpenAPI status code (or `default`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationResponse {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media_types: Vec<OperationMediaType>,
}

/// A named representation carried by a request or response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationMediaType {
    pub content_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<SchemaValue>,
}

/// One OpenAPI Security Requirement Object. Multiple values in
/// `Operation::security` are alternatives; the map entries in one value are
/// conjunctive scheme requirements.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityRequirement {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub schemes: BTreeMap<String, Vec<String>>,
}

/// Reusable security-scheme metadata from OpenAPI components.
///
/// An operation's [`SecurityRequirement`] says which named schemes it needs;
/// this catalog says how an SDK supplies each credential. Keeping it separate
/// lets plugins opt into authentication support without parsing sidecar JSON.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecuritySchemeCatalog {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schemes: Vec<SecurityScheme>,
}

/// A named OpenAPI component security scheme.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityScheme {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub kind: SecuritySchemeKind,
}

/// The credential metadata a generator needs to materialize an OpenAPI
/// security scheme. Unsupported/future scheme kinds are retained as `Other`
/// rather than discarded at the sidecar boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecuritySchemeKind {
    ApiKey {
        name: Option<String>,
        location: Option<String>,
    },
    Http {
        scheme: Option<String>,
        bearer_format: Option<String>,
    },
    OAuth2 {
        flows: Vec<OAuthFlow>,
        metadata_url: Option<String>,
    },
    OpenIdConnect {
        discovery_url: Option<String>,
    },
    Other {
        type_name: String,
    },
}

/// One named OAuth2 flow, including its token endpoints and advertised scopes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthFlow {
    pub flow_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_url: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scopes: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_a_recursive_discriminated_union() {
        let value = SchemaValue {
            nullable: true,
            discriminator: Some(Discriminator {
                property_name: "kind".into(),
                mapping: BTreeMap::from([("cat".into(), "#/components/schemas/Cat".into())]),
            }),
            ..SchemaValue::new(SchemaKind::OneOf {
                variants: vec![SchemaValue::reference("#/components/schemas/Cat")],
            })
        };
        let round_trip: SchemaValue =
            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(round_trip, value);
        assert_eq!(value.kind.reference_name(), None);
    }
}
