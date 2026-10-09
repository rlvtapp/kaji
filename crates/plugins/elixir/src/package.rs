//! Typed package integration for the existing complete Elixir generator.
use anyhow::Result;
use poolster_core::SdkClientStyle;
use poolster_core::engine::{Language, Meta, Package, Plugin, PluginContext};

pub struct Elixir;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for Elixir {
    const NAME: &'static str = "elixir";
    type Settings = Settings;
    type Workspace = ();
    fn bundle_middleware(
        tree: &mut poolster_core::GeneratedTree,
        middleware: &[poolster_core::customization::BundledMiddleware],
    ) -> Result<()> {
        crate::bundled::bundle(tree, middleware)
    }
}
pub fn package(dir: impl Into<String>) -> Package<Elixir> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}
impl PackageExt for Package<Elixir> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}
pub struct Sdk {
    meta: Meta,
    pub(crate) http: poolster_core::engine::HttpInput,
    client_style: Option<SdkClientStyle>,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        http: Default::default(),
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
impl Plugin<Elixir> for Sdk {
    fn kind(&self) -> &'static str {
        "elixir-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Elixir>) -> Result<()> {
        self.http.run(cx, |cx| {
            cx.files.append(crate::render_sdk(
                cx.api,
                ".",
                cx.settings.package_name.as_deref(),
                self.client_style
                    .or(cx.common.client_style)
                    .unwrap_or(SdkClientStyle::Namespaced),
            )?)
        })
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http.requirements()
    }

    fn supports_native_input(&self) -> bool {
        self.http.is_explicit()
    }
}

impl Sdk {
    /// Select the authoritative HTTP contract produced by an input or transform.
    pub fn input(
        mut self,
        input: poolster_core::engine::Handle<poolster_core::AdaptedApi>,
    ) -> Self {
        self.http = self.http.input(input);
        self
    }
    /// Consume complete model blocks from the selected HTTP contract revision.
    pub fn input_models(
        mut self,
        models: poolster_core::engine::Handle<poolster_core::blocks::Blocks<poolster_core::Schema>>,
    ) -> Self {
        self.http = self.http.input_models(models);
        self
    }
    /// Consume complete endpoint blocks from the selected HTTP contract revision.
    pub fn input_endpoints(
        mut self,
        endpoints: poolster_core::engine::Handle<
            poolster_core::blocks::Blocks<poolster_core::Operation>,
        >,
    ) -> Self {
        self.http = self.http.input_endpoints(endpoints);
        self
    }
}
