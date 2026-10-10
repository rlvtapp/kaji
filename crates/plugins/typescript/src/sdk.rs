//! TypeScript SDK layout, runtime, facade and typed renderer options.
use crate::clients::{ClientRenderOptions, generate_operations, operation_file_identifier};
use crate::models::{
    ModelOptions, ModelRenderOptions, ModelRenderer, operation_model_file_identifier,
};
use anyhow::{Result, bail};
use poolster_core::{
    Api, GeneratedFile, GeneratedTree, Operation, SdkClientStyle, SecuritySchemeCatalog,
};
use serde_json::Value;
use std::collections::BTreeMap;
/// A transport implementation selected within one language profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SdkTransport {
    Fetch,
    Axios,
}

/// Whether a TypeScript SDK exposes only generated exports or also an
/// instantiated product-client facade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SdkSurface {
    /// Models and direct operation functions only.
    Raw,
    /// Models, direct operation functions, and a configured SDK client.
    #[default]
    Client,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SdkConfig {
    pub output_dir: String,
    pub package_name: Option<String>,
    pub client_name: Option<String>,
    pub client_style: SdkClientStyle,
    pub surface: SdkSurface,
    pub transport: SdkTransport,
    pub group_by_tag: bool,
    pub model_options: ModelOptions,
    pub throw_on_error: bool,
}

impl SdkConfig {
    pub(crate) fn new(output_dir: impl Into<String>) -> Self {
        Self {
            output_dir: output_dir.into(),
            package_name: None,
            client_name: None,
            client_style: SdkClientStyle::Namespaced,
            surface: SdkSurface::Client,
            transport: SdkTransport::Fetch,
            group_by_tag: true,
            model_options: ModelOptions::default(),
            throw_on_error: true,
        }
    }
}

pub(crate) fn generate_sdk(
    api: &Api,
    profile: &SdkConfig,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<GeneratedTree> {
    if profile.output_dir.is_empty() {
        bail!("SDK output directory cannot be empty")
    }
    for operation in &api.operations {
        if poolster_core::poolster_extension(&operation.annotations, "pagination")
            .or_else(|| operation.annotations.get("x-speakeasy-pagination"))
            .and_then(|v| v.get("type"))
            .and_then(Value::as_str)
            == Some("page")
        {
            poolster_core::pagination::normalize_pagination(api, operation, None)?;
        }
    }
    generate_typescript_sdk(api, profile, security_schemes)
}

mod assembly;
pub(crate) use assembly::*;

mod barrels;
pub(crate) use barrels::*;

mod facade;
pub(crate) use facade::*;

mod pagination;
pub(crate) use pagination::*;

mod package;
pub(crate) use package::*;

mod runtime;
pub(crate) use runtime::*;

mod names;
pub(crate) use names::*;

#[cfg(test)]
mod tests;
