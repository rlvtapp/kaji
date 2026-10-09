//! Typed package integration for the existing complete Go generator.
use anyhow::Result;
use poolster_core::SdkClientStyle;
use poolster_core::engine::{Language, Meta, Package, Plugin, PluginContext};

pub struct Go;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for Go {
    const NAME: &'static str = "go";
    type Settings = Settings;
    type Workspace = crate::providers::Workspace;
    fn bundle_middleware(
        tree: &mut poolster_core::GeneratedTree,
        middleware: &[poolster_core::customization::BundledMiddleware],
    ) -> Result<()> {
        crate::bundled_middleware::bundle(tree, middleware)
    }
    fn finalize(cx: &mut poolster_core::engine::FinalizeContext<'_, Self>) -> Result<()> {
        crate::providers::Workspace::finalize(cx)
    }
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
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    client_style: Option<SdkClientStyle>,
    jobs: usize,
}
pub fn sdk() -> Sdk {
    Sdk {
        http_input: Default::default(),
        meta: Meta::new(),
        client_style: None,
        jobs: 0,
    }
}
impl Sdk {
    pub fn client(&self) -> poolster_core::engine::Handle<crate::providers::Client> {
        self.meta.handle()
    }

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
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }

    fn kind(&self) -> &'static str {
        "go-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<poolster_core::engine::Provision> {
        vec![
            poolster_core::engine::Provision::of::<crate::providers::Models>(),
            poolster_core::engine::Provision::of::<crate::providers::Transport>(),
            poolster_core::engine::Provision::of::<crate::providers::Operations>(),
            poolster_core::engine::Provision::of::<crate::providers::Client>(),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
            cx.publish(crate::providers::model_contract(cx.api))?;
            cx.publish(crate::providers::default_transport())?;
            cx.publish(crate::providers::operation_contract(cx.api))?;
            cx.publish(crate::providers::Client {
                symbol: "Client".into(),
            })?;
            cx.files.append(crate::render_sdk(
                cx.api,
                ".",
                cx.settings.package_name.as_deref(),
                self.client_style
                    .or(cx.common.client_style)
                    .unwrap_or(SdkClientStyle::Namespaced),
                self.jobs,
            )?)
        })
    }
}

#[path = "package_input.rs"]
mod http_input;
