//! Source-format adapters.
//!
//! The codegen core only consumes [`crate::Api`]. The native OpenAPI adapter
//! is the standalone production path; the sidecar reader remains available as
//! a migration adapter for existing compiler integrations.

pub mod openapi;
pub mod openapi_sidecar;
