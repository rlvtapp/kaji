//! Structured TypeScript Fetch and Axios operation-client generators.
//!
//! Fetch and Axios operation files share a consistent public source shape;
//! transport-specific setup lives in `.poolster/client`.

use crate::models::operation_model_file_identifier;
use anyhow::{Result, bail};
use serde_json::Value;

use poolster_core::GeneratedFile;
use poolster_core::ast::{Api, Operation, SecuritySchemeCatalog, SecuritySchemeKind};

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

mod operations;
pub(crate) use operations::*;

mod metadata;
pub(crate) use metadata::*;

mod streams;
pub(crate) use streams::*;

mod names;
pub(crate) use names::*;

#[cfg(test)]
mod tests;
