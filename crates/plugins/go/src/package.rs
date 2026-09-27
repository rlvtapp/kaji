//! Typed package integration for the existing complete Go generator.
use anyhow::Result;
use kaji_core::SdkClientStyle;
use kaji_core::engine::{Language, Meta, Package, Plugin, PluginContext};

pub struct Go;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for Go {
    const NAME: &'static str = "go";
    type Settings = Settings;
    type Workspace = ();
}
pub fn package(dir: impl Into<String>) -> Package<Go> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}
impl PackageExt for Package<Go> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}
pub struct Sdk {
    meta: Meta,
    client_style: Option<SdkClientStyle>,
    jobs: usize,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        client_style: None,
        jobs: 0,
    }
}
impl Sdk {
    /// Maximum rendering workers. Zero uses bounded automatic parallelism.
    pub fn jobs(mut self, jobs: usize) -> Self {
        self.jobs = jobs;
        self
    }
    pub fn flat(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Flat);
        self
    }
    pub fn namespaced(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Namespaced);
        self
    }
}
impl Plugin<Go> for Sdk {
    fn kind(&self) -> &'static str {
        "go-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        cx.files.append(crate::render_sdk(
            cx.api,
            ".",
            cx.settings.package_name.as_deref(),
            self.client_style
                .or(cx.common.client_style)
                .unwrap_or(SdkClientStyle::Namespaced),
            self.jobs,
        )?)
    }
}
