//! Rust generators. The package wrapper preserves the existing SDK output;
//! models and Reqwest renderers remain available for low-level composition.
pub mod render;

use anyhow::Result;
use kaji_core::engine::{Language, Meta, Package, Plugin, PluginContext};
use kaji_core::{GeneratedFile, GeneratorConfig, SdkClientStyle};

pub struct Rust;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for Rust {
    const NAME: &'static str = "rust";
    type Settings = Settings;
    type Workspace = ();
}
pub fn package(dir: impl Into<String>) -> Package<Rust> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}
impl PackageExt for Package<Rust> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}

/// Transitional complete SDK plugin. Splitting models/client contracts is a
/// separate migration; this wrapper does not advertise contracts it cannot honor.
pub struct Sdk {
    meta: Meta,
    client_style: Option<SdkClientStyle>,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        client_style: None,
    }
}
impl Sdk {
    pub fn flat(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Flat);
        self
    }
    pub fn namespaced(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Namespaced);
        self
    }
}
impl Plugin<Rust> for Sdk {
    fn kind(&self) -> &'static str {
        "rust-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        let mut config = GeneratorConfig::from([("output_dir".into(), ".".into())]);
        config.insert("sdk_surface".into(), "client".into());
        let style = self
            .client_style
            .or(cx.common.client_style)
            .unwrap_or(SdkClientStyle::Namespaced);
        config.insert(
            "client_style".into(),
            match style {
                SdkClientStyle::Flat => "flat",
                SdkClientStyle::Namespaced => "namespaced",
            }
            .into(),
        );
        if let Some(name) = &cx.settings.package_name {
            config.insert("crate_name".into(), name.clone());
        }
        cx.files.append(kaji_core::generate(
            cx.api,
            &[
                (&render::RustModels, config.clone()),
                (&render::RustReqwest, config.clone()),
                (&render::RustPackage, config),
            ],
        )?)?;
        cx.files
            .emit(GeneratedFile::new("STYLE_GUIDE.md", style_guide(cx.api))?)
    }
}

pub fn style_guide(api: &kaji_core::Api) -> String {
    format!(
        "# {} Rust SDK style guide\n\nThis package provides Kaji's resource-first Rust client: `client.contacts().list().await`. Direct `Client` operation methods remain available for compatibility. Path-parameter operations accept a completed path; typed path and query input structs are the next Rust-surface enhancement.\n",
        api.name
    )
}
