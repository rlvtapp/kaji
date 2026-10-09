//! Source-format adapters.
//!
//! The codegen core only consumes [`crate::Api`]. An [`Adapter`] is the
//! public boundary for bringing another source format into that neutral model;
//! it also carries security definitions which cannot be inferred from an
//! operation's named requirements alone. OpenAPI parsing is supplied by
//! Poolster's embedded Go compiler, whose JSON artifacts are read by
//! the `poolster-input-openapi` package.

use anyhow::Result;

use crate::{Api, SecuritySchemeCatalog};

/// A normalized input contract ready for language plugins.
///
/// Adapters should preserve source-specific information in [`Api::annotations`]
/// where it is useful to a generic transform, rather than exposing their
/// parser's internal types to generators. An empty security catalog is valid
/// for formats without reusable credential definitions.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AdaptedApi {
    pub api: Api,
    pub security_schemes: SecuritySchemeCatalog,
}

impl AdaptedApi {
    pub fn new(api: Api, security_schemes: SecuritySchemeCatalog) -> Self {
        Self {
            api,
            security_schemes,
        }
    }
}

/// Converts a source contract into Poolster's target-neutral AST.
///
/// Implement this trait in an application or integration crate to support
/// formats such as AsyncAPI, GraphQL, or a company-specific contract format.
/// It is deliberately synchronous and object-safe, so adapters can be passed
/// as `&dyn Adapter` when a caller selects an input format at runtime.
///
/// Output customization does not use a separate parser trait: it belongs to
/// Poolster's existing [`crate::engine::Language`] and [`crate::engine::Plugin`]
/// extension points after an adapter has produced an [`Api`].
pub trait Adapter {
    fn adapt(&self) -> Result<AdaptedApi>;
}
