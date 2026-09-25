//! Source-format adapters.
//!
//! The codegen core only consumes [`crate::Api`]. For now, OpenAPI is adapted
//! from the compiler's existing Go sidecar output. Replacing that sidecar with
//! a native Rust OpenAPI parser later will not change generator plugins.

pub mod openapi_sidecar;
