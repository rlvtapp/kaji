//! Source-format adapters.
//!
//! The codegen core only consumes [`crate::Api`]. OpenAPI parsing is supplied
//! by Kaji's embedded Go compiler, whose JSON artifacts are read here.

pub mod openapi_sidecar;
