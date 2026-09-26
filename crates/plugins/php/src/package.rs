//! Typed package integration for the existing complete Php generator.
use anyhow::Result;
use kaji_core::SdkClientStyle;
use kaji_core::engine::{Language, Meta, Package, Plugin, PluginContext};

pub struct Php;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for Php {
    const NAME: &'static str = "php";
    type Settings = Settings;
    type Workspace = ();
}
pub fn package(dir: impl Into<String>) -> Package<Php> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}
impl PackageExt for Package<Php> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}
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
impl Plugin<Php> for Sdk {
    fn kind(&self) -> &'static str {
        "php-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Php>) -> Result<()> {
        cx.files.append(crate::generate_php_sdk_with_style(
            cx.api,
            ".",
            cx.settings.package_name.as_deref(),
            self.client_style
                .or(cx.common.client_style)
                .unwrap_or(SdkClientStyle::Namespaced),
        )?)
    }
}
