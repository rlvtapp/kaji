use anyhow::Result;
use poolster_core::engine::{Language, Meta, Package, Plugin, PluginContext};

pub struct Symfony;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
    pub sdk_package: Option<String>,
}
impl Language for Symfony {
    const NAME: &'static str = "symfony";
    type Settings = Settings;
    type Workspace = ();
}
pub fn package(dir: impl Into<String>) -> Package<Symfony> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
    fn sdk_package(self, package: impl Into<String>) -> Self;
}
impl PackageExt for Package<Symfony> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
    fn sdk_package(mut self, package: impl Into<String>) -> Self {
        self.settings_mut().sdk_package = Some(package.into());
        self
    }
}
pub struct Sdk {
    meta: Meta,
}
pub fn sdk() -> Sdk {
    Sdk { meta: Meta::new() }
}
impl Plugin<Symfony> for Sdk {
    fn kind(&self) -> &'static str {
        "symfony-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Symfony>) -> Result<()> {
        cx.files.append(crate::render_sdk(
            cx.api,
            ".",
            cx.settings.package_name.as_deref(),
            cx.settings.sdk_package.as_deref(),
        )?)
    }
}
