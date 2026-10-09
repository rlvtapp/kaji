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
    pub(crate) http: poolster_core::engine::HttpInput,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        http: Default::default(),
    }
}
impl Plugin<Symfony> for Sdk {
    fn kind(&self) -> &'static str {
        "symfony-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Symfony>) -> Result<()> {
        self.http.run(cx, |cx| {
            cx.files.append(crate::render_sdk(
                cx.api,
                ".",
                cx.settings.package_name.as_deref(),
                cx.settings.sdk_package.as_deref(),
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
